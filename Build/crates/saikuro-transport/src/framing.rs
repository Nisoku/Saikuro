//! Length-prefixed framing for byte-stream transports.
//!
//! Raw stream transports (TCP, Unix sockets) deliver an unbroken river of
//! bytes with no inherent message boundaries.  We impose message framing with
//! a simple 4-byte big-endian length prefix before every frame:

use bytes::{Buf, BufMut, Bytes, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

use crate::error::{Result, TransportError};

/// Maximum allowed frame size (16 MiB).  Frames larger than this are rejected
/// to prevent memory exhaustion from malformed or malicious peers.
pub const MAX_FRAME_SIZE: usize = 16 * 1024 * 1024;

/// Tokio codec that frames a byte stream into discrete length-prefixed messages.
#[derive(Debug, Clone, Default)]
pub struct LengthPrefixedCodec {
    /// Once we've read the length header we cache it here to avoid re-parsing.
    pending_len: Option<u32>,
}

impl LengthPrefixedCodec {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Decoder for LengthPrefixedCodec {
    type Item = Bytes;
    type Error = TransportError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>> {
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

        let frame_len = frame_len as usize;

        if frame_len > MAX_FRAME_SIZE {
            return Err(TransportError::MessageTooLarge {
                size: frame_len,
                limit: MAX_FRAME_SIZE,
            });
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
}

impl Encoder<Bytes> for LengthPrefixedCodec {
    type Error = TransportError;

    fn encode(&mut self, item: Bytes, dst: &mut BytesMut) -> Result<()> {
        let len = item.len();
        if len > MAX_FRAME_SIZE {
            return Err(TransportError::MessageTooLarge {
                size: len,
                limit: MAX_FRAME_SIZE,
            });
        }

        dst.reserve(4 + len);
        dst.put_u32(len as u32);
        dst.put(item);
        Ok(())
    }
}
