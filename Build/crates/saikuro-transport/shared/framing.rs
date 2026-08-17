//! Length-prefixed framing for every Saikuro byte-stream transport.
//!
//! Wire format: a 4-byte big-endian length prefix followed by the payload, with
//! a 16 MiB maximum.

use bytes::{Bytes, BytesMut};

use crate::shared::error::{Result, TransportError};
use crate::MAX_FRAME_SIZE;

/// Number of big-endian length bytes that prefix every frame.
pub(crate) const HEADER_LEN: usize = 4;

fn message_too_large(size: usize) -> TransportError {
    TransportError::MessageTooLarge {
        size,
        limit: MAX_FRAME_SIZE,
    }
}

/// Backend-agnostic async source of bytes.  Every transport adapts its own
/// I/O to this trait so the framing codec is implemented only once.
pub trait AsyncByteRead {
    /// Read into `buf`, returning the number of bytes read.  `0` signals a
    /// clean end-of-stream (the peer closed the connection).
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize>;
}

/// Backend-agnostic async sink of bytes.
pub trait AsyncByteWrite {
    /// Write `buf`, returning the number of bytes accepted.
    async fn write(&mut self, buf: &[u8]) -> Result<usize>;
    /// Flush any buffered bytes to the underlying transport.
    async fn flush(&mut self) -> Result<()>;
}

#[cfg(feature = "embedded")]
impl<R: embedded_io_async::Read> AsyncByteRead for R {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.read(buf)
            .await
            .map_err(|e| TransportError::ConnectionLost(alloc::format!("{:?}", e)))
    }
}

#[cfg(feature = "embedded")]
impl<W: embedded_io_async::Write> AsyncByteWrite for W {
    async fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.write(buf)
            .await
            .map_err(|e| TransportError::ConnectionLost(alloc::format!("{:?}", e)))
    }

    async fn flush(&mut self) -> Result<()> {
        embedded_io_async::Write::flush(self)
            .await
            .map_err(|e| TransportError::ConnectionLost(alloc::format!("{:?}", e)))
    }
}

/// Read exactly `buf.len()` bytes, or fail if the peer closes first.
async fn read_exact<R: AsyncByteRead>(reader: &mut R, buf: &mut [u8]) -> Result<()> {
    let mut filled = 0;
    while filled < buf.len() {
        let n = reader.read(&mut buf[filled..]).await?;
        if n == 0 {
            return Err(TransportError::ConnectionLost(
                "connection closed mid-frame".into(),
            ));
        }
        filled += n;
    }
    Ok(())
}

/// Read the 4-byte header.  Returns `Ok(true)` if the peer closed cleanly at a
/// frame boundary (no bytes were read), `Ok(false)` once the header is full.
async fn read_header_or_eof<R: AsyncByteRead>(
    reader: &mut R,
    header: &mut [u8; HEADER_LEN],
) -> Result<bool> {
    let mut filled = 0;
    while filled < HEADER_LEN {
        let n = reader.read(&mut header[filled..]).await?;
        if n == 0 {
            if filled == 0 {
                return Ok(true);
            }
            return Err(TransportError::ConnectionLost(
                "connection closed mid-frame header".into(),
            ));
        }
        filled += n;
    }
    Ok(false)
}

/// Receive one length-prefixed frame, or `None` on a clean peer close.
pub async fn read_frame<R: AsyncByteRead>(reader: &mut R) -> Result<Option<Bytes>> {
    let mut header = [0u8; HEADER_LEN];
    if read_header_or_eof(reader, &mut header).await? {
        return Ok(None);
    }
    let frame_len = decode_length_prefix(&header);
    if frame_len > MAX_FRAME_SIZE {
        return Err(message_too_large(frame_len));
    }
    let mut payload = BytesMut::zeroed(frame_len);
    read_exact(reader, &mut payload).await?;
    Ok(Some(payload.freeze()))
}

/// Send one length-prefixed frame.
pub async fn write_frame<W: AsyncByteWrite>(writer: &mut W, frame: &[u8]) -> Result<()> {
    if frame.len() > MAX_FRAME_SIZE {
        return Err(message_too_large(frame.len()));
    }
    let header = encode_length_prefix(frame.len());
    writer.write(&header).await?;
    writer.write(frame).await?;
    writer.flush().await?;
    Ok(())
}

/// Encode a frame length as the 4-byte big-endian wire header.
pub const fn encode_length_prefix(len: usize) -> [u8; HEADER_LEN] {
    (len as u32).to_be_bytes()
}

/// Decode a 4-byte big-endian wire header into a frame length.
pub fn decode_length_prefix(header: &[u8; HEADER_LEN]) -> usize {
    u32::from_be_bytes(*header) as usize
}
