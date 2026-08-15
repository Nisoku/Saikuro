use alloc::{collections::BTreeMap, sync::Arc};
use saikuro_core::invocation::InvocationId;
use saikuro_core::ResponseEnvelope;
use saikuro_exec::{
    mpsc,
    sync::{Mutex, RwLock},
};

/// Result of attempting to deliver one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryOutcome {
    Delivered,
    Terminal,
    Closed,
    OutOfOrder,
}

#[derive(Default)]
struct Lifecycle {
    inbound_seq: u64,
    outbound_seq: u64,
    closed: bool,
}

/// Lifecycle state for an open server-to-client stream.
pub struct StreamState {
    lifecycle: Mutex<Lifecycle>,
    item_tx: mpsc::Sender<ResponseEnvelope>,
}

impl StreamState {
    pub fn new(item_tx: mpsc::Sender<ResponseEnvelope>) -> Arc<Self> {
        Arc::new(Self {
            lifecycle: Mutex::new(Lifecycle::default()),
            item_tx,
        })
    }

    pub async fn deliver(&self, response: ResponseEnvelope) -> DeliveryOutcome {
        let mut lifecycle = self.lifecycle.lock().await;
        let expected_seq = lifecycle.inbound_seq;
        let has_seq = response.seq.is_some();
        let outcome =
            deliver_locked(&mut lifecycle.closed, expected_seq, &self.item_tx, response).await;
        if matches!(
            outcome,
            DeliveryOutcome::Delivered | DeliveryOutcome::Terminal
        ) && has_seq
            && expected_seq < u64::MAX
        {
            lifecycle.inbound_seq = expected_seq + 1;
        }
        outcome
    }
}

/// Lifecycle state for an open bidirectional channel.
pub struct ChannelState {
    lifecycle: Mutex<Lifecycle>,
    inbound_tx: mpsc::Sender<ResponseEnvelope>,
    outbound_tx: mpsc::Sender<ResponseEnvelope>,
}

impl ChannelState {
    pub fn new(
        inbound_tx: mpsc::Sender<ResponseEnvelope>,
        outbound_tx: mpsc::Sender<ResponseEnvelope>,
    ) -> Arc<Self> {
        Arc::new(Self {
            lifecycle: Mutex::new(Lifecycle::default()),
            inbound_tx,
            outbound_tx,
        })
    }

    pub async fn deliver(&self, response: ResponseEnvelope, inbound: bool) -> DeliveryOutcome {
        let mut lifecycle = self.lifecycle.lock().await;
        let (expected_seq, tx) = if inbound {
            (lifecycle.inbound_seq, &self.inbound_tx)
        } else {
            (lifecycle.outbound_seq, &self.outbound_tx)
        };
        let has_seq = response.seq.is_some();
        let outcome = deliver_locked(&mut lifecycle.closed, expected_seq, tx, response).await;
        if matches!(
            outcome,
            DeliveryOutcome::Delivered | DeliveryOutcome::Terminal
        ) && has_seq
            && expected_seq < u64::MAX
        {
            if inbound {
                lifecycle.inbound_seq = expected_seq + 1;
            } else {
                lifecycle.outbound_seq = expected_seq + 1;
            }
        }
        outcome
    }
}

async fn deliver_locked(
    closed: &mut bool,
    expected_seq: u64,
    tx: &mpsc::Sender<ResponseEnvelope>,
    response: ResponseEnvelope,
) -> DeliveryOutcome {
    if *closed {
        return DeliveryOutcome::Closed;
    }
    if let Some(seq) = response.seq {
        if seq.checked_add(1).is_none() || seq != expected_seq {
            return DeliveryOutcome::OutOfOrder;
        }
    }
    let terminal = matches!(
        response.stream_control,
        Some(saikuro_core::envelope::StreamControl::End)
            | Some(saikuro_core::envelope::StreamControl::Abort)
    );
    if tx.send(response).await.is_err() {
        *closed = true;
        return DeliveryOutcome::Closed;
    }
    if terminal {
        *closed = true;
        DeliveryOutcome::Terminal
    } else {
        DeliveryOutcome::Delivered
    }
}

/// Thread-safe store for all open stream and channel states.
#[derive(Clone, Default)]
pub struct StreamStateStore {
    streams: Arc<RwLock<BTreeMap<InvocationId, StreamEntry>>>,
    channels: Arc<RwLock<BTreeMap<InvocationId, ChannelEntry>>>,
}

struct StreamEntry {
    state: Arc<StreamState>,
    receiver: Option<mpsc::Receiver<ResponseEnvelope>>,
}

struct ChannelEntry {
    state: Arc<ChannelState>,
    inbound_receiver: Option<mpsc::Receiver<ResponseEnvelope>>,
    outbound_receiver: Option<mpsc::Receiver<ResponseEnvelope>>,
}

impl StreamStateStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn insert_stream(
        &self,
        id: InvocationId,
        state: Arc<StreamState>,
        receiver: mpsc::Receiver<ResponseEnvelope>,
    ) {
        self.streams.write().await.insert(
            id,
            StreamEntry {
                state,
                receiver: Some(receiver),
            },
        );
    }

    pub async fn get_stream(&self, id: &InvocationId) -> Option<Arc<StreamState>> {
        self.streams
            .read()
            .await
            .get(id)
            .map(|entry| entry.state.clone())
    }

    pub async fn remove_stream(&self, id: &InvocationId) -> Option<Arc<StreamState>> {
        self.streams
            .write()
            .await
            .remove(id)
            .map(|entry| entry.state)
    }

    pub async fn remove_stream_if(&self, id: &InvocationId, state: &Arc<StreamState>) -> bool {
        let mut streams = self.streams.write().await;
        if streams
            .get(id)
            .is_some_and(|entry| Arc::ptr_eq(&entry.state, state))
        {
            streams.remove(id);
            true
        } else {
            false
        }
    }

    pub async fn take_stream_receiver(
        &self,
        id: &InvocationId,
    ) -> Option<mpsc::Receiver<ResponseEnvelope>> {
        self.streams
            .write()
            .get_mut(id)
            .and_then(|entry| entry.receiver.take())
    }

    pub async fn insert_channel(
        &self,
        id: InvocationId,
        state: Arc<ChannelState>,
        inbound_rx: mpsc::Receiver<ResponseEnvelope>,
        outbound_rx: mpsc::Receiver<ResponseEnvelope>,
    ) {
        self.channels.write().insert(
            id,
            ChannelEntry {
                state,
                inbound_receiver: Some(inbound_rx),
                outbound_receiver: Some(outbound_rx),
            },
        );
    }

    pub async fn get_channel(&self, id: &InvocationId) -> Option<Arc<ChannelState>> {
        self.channels
            .read()
            .get(id)
            .map(|entry| entry.state.clone())
    }

    pub async fn remove_channel(&self, id: &InvocationId) -> Option<Arc<ChannelState>> {
        self.channels.write().remove(id).map(|entry| entry.state)
    }

    pub async fn remove_channel_if(&self, id: &InvocationId, state: &Arc<ChannelState>) -> bool {
        let mut channels = self.channels.write();
        if channels
            .get(id)
            .is_some_and(|entry| Arc::ptr_eq(&entry.state, state))
        {
            channels.remove(id);
            true
        } else {
            false
        }
    }

    pub async fn take_channel_inbound_receiver(
        &self,
        id: &InvocationId,
    ) -> Option<mpsc::Receiver<ResponseEnvelope>> {
        self.channels
            .write()
            .get_mut(id)
            .and_then(|entry| entry.inbound_receiver.take())
    }

    pub async fn take_channel_outbound_receiver(
        &self,
        id: &InvocationId,
    ) -> Option<mpsc::Receiver<ResponseEnvelope>> {
        self.channels
            .write()
            .get_mut(id)
            .and_then(|entry| entry.outbound_receiver.take())
    }
}
