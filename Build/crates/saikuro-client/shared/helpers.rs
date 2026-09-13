//! Shared helper functions for client and provider.

use alloc::borrow::ToOwned;
use alloc::string::ToString;
use alloc::vec::Vec;

use bytes::Bytes;
use saikuro_core::envelope::{Envelope, InvocationType, ResponseEnvelope, StreamControl};
use saikuro_core::invocation::InvocationId;
use saikuro_core::PROTOCOL_VERSION;
use saikuro_event::Result;
use saikuro_event::{core_to_json, json_to_core, ErrorCode, ErrorDetail, SaikuroError};
use saikuro_transport::AdapterTransport;

use super::map::{ChannelSenderMap, PendingMap};
use super::types::PendingSlot;
use crate::Value;

/// Drain announce envelopes that arrived before the client's I/O task starts.
pub(crate) async fn drain_announces(transport: &mut dyn AdapterTransport) {
    const POLL_TIMEOUT: core::time::Duration = core::time::Duration::from_millis(20);

    while let Ok(Ok(Some(frame))) = saikuro_exec::timeout(POLL_TIMEOUT, transport.recv()).await {
        if let Ok(env) = Envelope::from_msgpack(&frame) {
            if env.invocation_type == InvocationType::Announce {
                let ack = ResponseEnvelope::ok_empty(env.id);
                if let Ok(ack_bytes) = ack.to_msgpack() {
                    let _ = transport.send(Bytes::from(ack_bytes)).await;
                }
                continue;
            }
        }
    }
}

/// Process a single inbound frame from the transport.
pub(crate) async fn handle_inbound(
    frame: Bytes,
    transport: &mut dyn AdapterTransport,
    pending: &PendingMap,
    channel_senders: &ChannelSenderMap,
) {
    if let Ok(resp) = ResponseEnvelope::from_msgpack(&frame) {
        route_response(resp, pending, channel_senders).await;
        return;
    }

    // Fall back to Envelope: this catches late/duplicate announces.
    if let Ok(env) = Envelope::from_msgpack(&frame) {
        if env.invocation_type == InvocationType::Announce {
            let ack = ResponseEnvelope::ok_empty(env.id);
            if let Ok(ack_bytes) = ack.to_msgpack() {
                let _ = transport.send(Bytes::from(ack_bytes)).await;
            }
        }
    }
}

/// Route a decoded response to its pending slot.
///
/// Uses `remove` instead of `get` to avoid holding the lock across `.await`
/// points. For streams/channels that need to stay open, the slot is
/// re-inserted after a successful send.
pub(crate) async fn route_response(
    resp: ResponseEnvelope,
    pending: &PendingMap,
    channel_senders: &ChannelSenderMap,
) {
    let id = resp.id;
    let is_stream_end = resp
        .stream_control
        .as_ref()
        .is_some_and(|c| matches!(c, StreamControl::End | StreamControl::Abort));
    let is_error = !resp.ok;

    // Remove the slot atomically, never hold the map lock across an await.
    let slot = match pending.remove(&id) {
        Some((_, slot)) => slot,
        None => return,
    };

    match slot {
        PendingSlot::Call(tx) => {
            let _ = tx.send(resp);
        }
        PendingSlot::Stream(tx) => {
            if is_stream_end {
                // Stream finished so slot already removed.
            } else if is_error {
                let detail = resp
                    .error
                    .unwrap_or_else(|| ErrorDetail::new(ErrorCode::Internal, "stream error"));
                let _ = tx
                    .send(Err(SaikuroError::remote(
                        detail.code.to_string(),
                        detail.message,
                        None,
                    )))
                    .await;
            } else {
                let value = resp.result.map(core_to_json).unwrap_or(Value::Null);
                if tx.send(Ok(value)).await.is_err() {
                    // Receiver dropped, stream is dead, slot already removed.
                } else {
                    // Stream still alive, re-insert so future frames route here.
                    pending.insert(id, PendingSlot::Stream(tx));
                }
            }
        }
        PendingSlot::Channel(tx) => {
            if is_stream_end {
                // Channel finished so slot already removed.
                cleanup_channel_sender(channel_senders, &id).await;
            } else if is_error {
                let detail = resp
                    .error
                    .unwrap_or_else(|| ErrorDetail::new(ErrorCode::Internal, "channel error"));
                let _ = tx.try_send(Err(SaikuroError::remote(
                    detail.code.to_string(),
                    detail.message,
                    None,
                )));
                cleanup_channel_sender(channel_senders, &id).await;
            } else {
                let value = resp.result.map(core_to_json).unwrap_or(Value::Null);
                if tx.try_send(Ok(value)).is_err() {
                    // Sender full or disconnected, channel is dead.
                    cleanup_channel_sender(channel_senders, &id).await;
                } else {
                    // Channel still alive, re-insert.
                    pending.insert(id, PendingSlot::Channel(tx));
                }
            }
        }
    }
}

/// Clean up a channel's outbound sender handle.
async fn cleanup_channel_sender(channel_senders: &ChannelSenderMap, id: &InvocationId) {
    if let Some((_, sender)) = channel_senders.remove(id) {
        let _ = sender.lock().await.take();
    }
}

/// Tear down all pending slots, sending connection-lost errors to streams
/// and channels.
pub(crate) fn teardown_pending(pending: &PendingMap) {
    let keys = pending.keys();
    for key in keys {
        if let Some((_, slot)) = pending.remove(&key) {
            match slot {
                PendingSlot::Call(tx) => drop(tx),
                PendingSlot::Stream(tx) => {
                    let _ =
                        tx.try_send(Err(SaikuroError::ConnectionLost("connection lost".into())));
                }
                PendingSlot::Channel(tx) => {
                    let _ =
                        tx.try_send(Err(SaikuroError::ConnectionLost("connection lost".into())));
                }
            }
        }
    }
}

// Envelope helpers

/// Build an envelope with an auto-generated invocation ID.
pub(crate) fn make_envelope(
    inv_type: InvocationType,
    target: &str,
    args: Vec<Value>,
) -> Result<Envelope> {
    Ok(make_envelope_with_id(
        InvocationId::new()?,
        inv_type,
        target,
        args,
        None,
    ))
}

/// Build an envelope with a caller-supplied invocation ID.
pub(crate) fn make_envelope_with_id(
    id: InvocationId,
    inv_type: InvocationType,
    target: &str,
    args: Vec<Value>,
    seq: Option<u64>,
) -> Envelope {
    let core_args: Vec<saikuro_event::Value> = args.into_iter().map(json_to_core).collect();
    Envelope {
        version: PROTOCOL_VERSION,
        invocation_type: inv_type,
        id,
        target: target.to_owned(),
        args: core_args,
        meta: Default::default(),
        capability: None,
        batch_items: None,
        stream_control: None,
        seq,
    }
}

/// Convert a [`ResponseEnvelope`] into a `Result<Value>`.
pub(crate) fn response_to_result(resp: ResponseEnvelope) -> Result<Value> {
    if resp.ok {
        Ok(resp.result.map(core_to_json).unwrap_or(Value::Null))
    } else {
        let detail = resp
            .error
            .unwrap_or_else(|| ErrorDetail::new(ErrorCode::Internal, "call failed with no detail"));
        Err(SaikuroError::remote(
            detail.code.to_string(),
            detail.message,
            None,
        ))
    }
}
