//! Per-stream and per-channel lifecycle state.
//!
//! When a `Stream` or `Channel` invocation is opened the router creates an
//! entry in the [`StreamStateStore`].  Subsequent messages that carry the
//! same invocation ID are correlated back to that entry for sequence checking
//! and backpressure enforcement.

use alloc::{collections::BTreeMap, sync::Arc};
use core::sync::atomic::Ordering;
use portable_atomic::{AtomicBool, AtomicU64};
use saikuro_core::invocation::InvocationId;
use saikuro_core::sync::RwLock;
use saikuro_core::ResponseEnvelope;
use saikuro_exec::mpsc;

/// Extension trait for atomic sequence-number advancement.
///
/// Replaces three identical load/compare/store patterns in `StreamState`
/// and `ChannelState`.
trait TryAdvanceSeq {
    fn try_advance(&self, seq: u64) -> bool;
}

impl TryAdvanceSeq for AtomicU64 {
    fn try_advance(&self, seq: u64) -> bool {
        let Some(next) = seq.checked_add(1) else {
            return false;
        };
        self.compare_exchange(seq, next, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
}

// Stream state

/// Lifecycle state for an open server-to-client stream.
pub struct StreamState {
    /// Next expected inbound sequence number (for in-order delivery enforcement).
    next_seq: AtomicU64,
    /// Whether the stream has been closed (end-of-stream sentinel received).
    closed: AtomicBool,
    /// Channel to deliver stream items to the waiting client receiver.
    item_tx: mpsc::Sender<ResponseEnvelope>,
}

impl StreamState {
    pub fn new(item_tx: mpsc::Sender<ResponseEnvelope>) -> Arc<Self> {
        Arc::new(Self {
            next_seq: AtomicU64::new(0),
            closed: AtomicBool::new(false),
            item_tx,
        })
    }

    /// Record receipt of the next item.  Returns `false` if the sequence
    /// number is out of order (caller should produce an `OutOfOrder` error).
    pub fn advance_seq(&self, seq: u64) -> bool {
        self.next_seq.try_advance(seq)
    }

    pub fn mark_closed(&self) {
        self.closed.store(true, Ordering::Release);
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    pub fn item_tx(&self) -> &mpsc::Sender<ResponseEnvelope> {
        &self.item_tx
    }
}

// Channel state

/// Lifecycle state for an open bidirectional channel.
pub struct ChannelState {
    /// Sequence counter for inbound messages (client -> server).
    inbound_seq: AtomicU64,
    /// Sequence counter for outbound messages (server -> client).
    outbound_seq: AtomicU64,
    /// Whether the channel has been fully closed.
    closed: AtomicBool,
    /// Channel to deliver inbound messages to the provider.
    inbound_tx: mpsc::Sender<ResponseEnvelope>,
    /// Channel to deliver outbound messages back to the client.
    outbound_tx: mpsc::Sender<ResponseEnvelope>,
}

impl ChannelState {
    pub fn new(
        inbound_tx: mpsc::Sender<ResponseEnvelope>,
        outbound_tx: mpsc::Sender<ResponseEnvelope>,
    ) -> Arc<Self> {
        Arc::new(Self {
            inbound_seq: AtomicU64::new(0),
            outbound_seq: AtomicU64::new(0),
            closed: AtomicBool::new(false),
            inbound_tx,
            outbound_tx,
        })
    }

    pub fn advance_inbound(&self, seq: u64) -> bool {
        self.inbound_seq.try_advance(seq)
    }

    pub fn advance_outbound(&self, seq: u64) -> bool {
        self.outbound_seq.try_advance(seq)
    }

    pub fn mark_closed(&self) {
        self.closed.store(true, Ordering::Release);
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    pub fn inbound_tx(&self) -> &mpsc::Sender<ResponseEnvelope> {
        &self.inbound_tx
    }

    pub fn outbound_tx(&self) -> &mpsc::Sender<ResponseEnvelope> {
        &self.outbound_tx
    }
}

// Store

/// Thread-safe store for all open stream and channel states.
///
/// Each map has its own [`RwLock`]; every access is a single-statement guard
/// so no two locks are ever held simultaneously.  `InvocationId` is
/// `Ord`, so `BTreeMap` keys keep iteration deterministic.
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

    // Stream

    /// Insert a stream state together with the corresponding receiver.
    ///
    /// Keeping the receiver here ensures the mpsc channel stays open so that
    /// `item_tx.send()` succeeds until someone takes the receiver.
    pub fn insert_stream(
        &self,
        id: InvocationId,
        state: Arc<StreamState>,
        receiver: mpsc::Receiver<ResponseEnvelope>,
    ) {
        self.streams.write().insert(
            id,
            StreamEntry {
                state,
                receiver: Some(receiver),
            },
        );
    }

    pub fn get_stream(&self, id: &InvocationId) -> Option<Arc<StreamState>> {
        self.streams.read().get(id).map(|entry| entry.state.clone())
    }

    pub fn remove_stream(&self, id: &InvocationId) -> Option<Arc<StreamState>> {
        self.streams.write().remove(id).map(|entry| entry.state)
    }

    /// Take the receiver half of the stream item channel.
    ///
    /// After this call the router no longer holds the receiver; the caller is
    /// responsible for consuming it.  The channel remains live because `item_tx`
    /// is still held inside `StreamState`.
    pub fn take_stream_receiver(
        &self,
        id: &InvocationId,
    ) -> Option<mpsc::Receiver<ResponseEnvelope>> {
        self.streams
            .write()
            .get_mut(id)
            .and_then(|entry| entry.receiver.take())
    }

    // Channel

    pub fn insert_channel(
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

    pub fn get_channel(&self, id: &InvocationId) -> Option<Arc<ChannelState>> {
        self.channels
            .read()
            .get(id)
            .map(|entry| entry.state.clone())
    }

    pub fn remove_channel(&self, id: &InvocationId) -> Option<Arc<ChannelState>> {
        self.channels.write().remove(id).map(|entry| entry.state)
    }

    /// Take the inbound receiver (client -> provider) for a channel.
    pub fn take_channel_inbound_receiver(
        &self,
        id: &InvocationId,
    ) -> Option<mpsc::Receiver<ResponseEnvelope>> {
        self.channels
            .write()
            .get_mut(id)
            .and_then(|entry| entry.inbound_receiver.take())
    }

    /// Take the outbound receiver (provider -> client) for a channel.
    pub fn take_channel_outbound_receiver(
        &self,
        id: &InvocationId,
    ) -> Option<mpsc::Receiver<ResponseEnvelope>> {
        self.channels
            .write()
            .get_mut(id)
            .and_then(|entry| entry.outbound_receiver.take())
    }
}
