use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::MAX_FRAME_SIZE;
use crate::shared::error::{Result, TransportError};

/// Codec that frames a byte stream into discrete length-prefixed messages.
#[derive(Debug, Clone, Default)]
pub struct LengthPrefixedCodec {
    /// Once we've read the length header we cache it here to avoid re-parsing.
    pending_len: Option<u32>,
    /// Payload bytes still owed for a frame whose length header exceeded
    /// [`MAX_FRAME_SIZE`].  The header is consumed but the declared payload
    /// must be swallowed so it is not misinterpreted as a fresh header.
    discard_remaining: u64,
}

impl LengthPrefixedCodec {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return whether decoding has consumed a header and still expects bytes.
    #[cfg(any(feature = "tcp", feature = "unix"))]
    pub(crate) fn has_pending_frame(&self) -> bool {
        self.pending_len.is_some() || self.discard_remaining != 0
    }

    /// Decode the next complete frame from `src`, returning `Ok(None)` until a
    /// full frame is buffered.
    pub fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Bytes>> {
        // Swallow any payload owed by a rejected oversized frame before
        // touching normal framing state.
        if self.discard_remaining > 0 {
            let take = core::cmp::min(self.discard_remaining, src.len() as u64);
            src.advance(take as usize);
            self.discard_remaining -= take;
            if self.discard_remaining > 0 {
                return Ok(None);
            }
        }

        // Phase 1: read the 4-byte length header if we don't have it yet.
        let frame_len = match self.pending_len {
            Some(len) => len,
            None => {
                if src.len() < 4 {
                    // Not enough bytes yet; ask for more.
                    return Ok(None);
                }
                let len = u32::from_be_bytes([src[0], src[1], src[2], src[3]]);
                src.advance(4);
                self.pending_len = Some(len);
                len
            }
        };

        let frame_len =
            usize::try_from(frame_len).map_err(|_| message_too_large(frame_len as usize))?;

        if frame_len > MAX_FRAME_SIZE {
            // The declared payload will never be decoded, so count it against
            // the discard budget instead of resetting and letting the next
            // call misread payload bytes as a length header.
            self.pending_len = None;
            self.discard_remaining = frame_len as u64;
            return Err(message_too_large(frame_len));
        }

        // Phase 2: wait until the full payload has arrived.
        if src.len() < frame_len {
            // Reserve exactly the bytes we still need to avoid churn.
            src.reserve(frame_len - src.len());
            return Ok(None);
        }

        // We have a complete frame.
        self.pending_len = None;
        let payload = src.split_to(frame_len).freeze();
        Ok(Some(payload))
    }

    /// Encode `item` as a length-prefixed frame appended to `dst`.
    pub fn encode(&mut self, item: Bytes, dst: &mut BytesMut) -> Result<()> {
        let len = item.len();
        if len > MAX_FRAME_SIZE {
            return Err(message_too_large(len));
        }

        dst.reserve(4 + len);
        dst.put_u32(u32::try_from(len).map_err(|_| message_too_large(len))?);
        dst.put(item);
        Ok(())
    }
}

fn message_too_large(size: usize) -> TransportError {
    TransportError::MessageTooLarge {
        size,
        limit: MAX_FRAME_SIZE,
    }
}
