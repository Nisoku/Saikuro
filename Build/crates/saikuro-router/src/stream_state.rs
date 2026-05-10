//! Per-stream and per-channel lifecycle state.
//!
//! When a `Stream` or `Channel` invocation is opened the router creates an
//! entry in the [`StreamStateStore`].  Subsequent messages that carry the
//! same invocation ID are correlated back to that entry for sequence checking
//! and backpressure enforcement.

use dashmap::DashMap;
use saikuro_core::invocation::InvocationId;
use saikuro_core::ResponseEnvelope;
use saikuro_exec::mpsc;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

/// Extension trait for atomic sequence-number advancement.
///
/// Replaces three identical load/compare/store patterns in `StreamState`
/// and `ChannelState`.
trait TryAdvanceSeq {
    fn try_advance(&self, seq: u64) -> bool;
}

impl TryAdvanceSeq for AtomicU64 {
    fn try_advance(&self, seq: u64) -> bool {
        let expected = self.load(Ordering::Acquire);
        if seq != expected {
            return false;
        }
        self.store(expected + 1, Ordering::Release);
        true
    }
}

// Stream state

/// Lifecycle state for an open server-to-client stream.
pub struct StreamState {
    /// Next expected inbound sequence number (for in-order delivery enforcement).
    pub next_seq: AtomicU64,
    /// Whether the stream has been closed (end-of-stream sentinel received).
    pub closed: AtomicBool,
    /// Channel to deliver stream items to the waiting client receiver.
    pub item_tx: mpsc::Sender<ResponseEnvelope>,
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
}

// Channel state

/// Lifecycle state for an open bidirectional channel.
pub struct ChannelState {
    /// Sequence counter for inbound messages (client -> server).
    pub inbound_seq: AtomicU64,
    /// Sequence counter for outbound messages (server -> client).
    pub outbound_seq: AtomicU64,
    /// Whether the channel has been fully closed.
    pub closed: AtomicBool,
    /// Channel to deliver inbound messages to the provider.
    pub inbound_tx: mpsc::Sender<ResponseEnvelope>,
    /// Channel to deliver outbound messages back to the client.
    pub outbound_tx: mpsc::Sender<ResponseEnvelope>,
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
}

// Store

/// Thread-safe store for all open stream and channel states.
#[derive(Clone, Default)]
pub struct StreamStateStore {
    streams: Arc<DashMap<InvocationId, Arc<StreamState>>>,
    /// Receivers for stream item channels.  Stored here so the channel stays
    /// live (i.e. `item_tx.send()` does not fail with "channel closed") until
    /// a caller explicitly takes and consumes the receiver.
    stream_receivers: Arc<DashMap<InvocationId, mpsc::Receiver<ResponseEnvelope>>>,
    channels: Arc<DashMap<InvocationId, Arc<ChannelState>>>,
    /// Receivers for channel inbound messages.
    channel_inbound_receivers: Arc<DashMap<InvocationId, mpsc::Receiver<ResponseEnvelope>>>,
    /// Receivers for channel outbound messages.
    channel_outbound_receivers: Arc<DashMap<InvocationId, mpsc::Receiver<ResponseEnvelope>>>,
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
        self.streams.insert(id, state);
        self.stream_receivers.insert(id, receiver);
    }

    pub fn get_stream(&self, id: &InvocationId) -> Option<Arc<StreamState>> {
        self.streams.get(id).map(|r| r.clone())
    }

    pub fn remove_stream(&self, id: &InvocationId) -> Option<Arc<StreamState>> {
        self.stream_receivers.remove(id);
        self.streams.remove(id).map(|(_, v)| v)
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
        self.stream_receivers.remove(id).map(|(_, v)| v)
    }

    // Channel

    pub fn insert_channel(
        &self,
        id: InvocationId,
        state: Arc<ChannelState>,
        inbound_rx: mpsc::Receiver<ResponseEnvelope>,
        outbound_rx: mpsc::Receiver<ResponseEnvelope>,
    ) {
        self.channels.insert(id, state);
        self.channel_inbound_receivers.insert(id, inbound_rx);
        self.channel_outbound_receivers.insert(id, outbound_rx);
    }

    pub fn get_channel(&self, id: &InvocationId) -> Option<Arc<ChannelState>> {
        self.channels.get(id).map(|r| r.clone())
    }

    pub fn remove_channel(&self, id: &InvocationId) -> Option<Arc<ChannelState>> {
        self.channel_inbound_receivers.remove(id);
        self.channel_outbound_receivers.remove(id);
        self.channels.remove(id).map(|(_, v)| v)
    }

    /// Take the inbound receiver (client -> provider) for a channel.
    pub fn take_channel_inbound_receiver(
        &self,
        id: &InvocationId,
    ) -> Option<mpsc::Receiver<ResponseEnvelope>> {
        self.channel_inbound_receivers.remove(id).map(|(_, v)| v)
    }

    /// Take the outbound receiver (provider -> client) for a channel.
    pub fn take_channel_outbound_receiver(
        &self,
        id: &InvocationId,
    ) -> Option<mpsc::Receiver<ResponseEnvelope>> {
        self.channel_outbound_receivers.remove(id).map(|(_, v)| v)
    }
}
