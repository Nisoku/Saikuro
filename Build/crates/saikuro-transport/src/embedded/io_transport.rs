use alloc::string::ToString;
use bytes::{Bytes, BytesMut};
use embedded_io_async::{Read, Write};

use crate::shared::framed::{read_exact, read_first_byte, write_all, HEADER_LEN};
use crate::shared::error::{Result, TransportError};
use crate::shared::traits::{LocalTransportReceiver, LocalTransportSender};

/// A framed transport composed from independently owned reader and writer halves.
pub struct EmbeddedIoTransport<R, W> {
    reader: R,
    writer: W,
    max_frame_size: usize,
}

/// The writer half of [`EmbeddedIoTransport`].
pub struct EmbeddedIoSender<W> {
    writer: W,
    max_frame_size: usize,
}

/// The reader half of [`EmbeddedIoTransport`].
pub struct EmbeddedIoReceiver<R> {
    reader: R,
    max_frame_size: usize,
}

impl<R, W> EmbeddedIoTransport<R, W> {
    /// Construct a transport from `reader` and `writer`.
    ///
    /// Errors if `max_frame_size` exceeds the crate-wide [`MAX_FRAME_SIZE`](crate::MAX_FRAME_SIZE)
    /// limit.
    pub fn new(reader: R, writer: W, max_frame_size: usize) -> Result<Self> {
        if max_frame_size > crate::MAX_FRAME_SIZE {
            return Err(TransportError::MessageTooLarge {
                size: max_frame_size,
                limit: crate::MAX_FRAME_SIZE,
            });
        }
        Ok(Self {
            reader,
            writer,
            max_frame_size,
        })
    }

    /// Split into independent sender and receiver halves.
    pub fn split(self) -> (EmbeddedIoSender<W>, EmbeddedIoReceiver<R>) {
        (
            EmbeddedIoSender {
                writer: self.writer,
                max_frame_size: self.max_frame_size,
            },
            EmbeddedIoReceiver {
                reader: self.reader,
                max_frame_size: self.max_frame_size,
            },
        )
    }
}

impl<W: Write> LocalTransportSender for EmbeddedIoSender<W> {
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        if frame.len() > self.max_frame_size {
            return Err(TransportError::MessageTooLarge {
                size: frame.len(),
                limit: self.max_frame_size,
            });
        }
        let mut header = [0u8; HEADER_LEN];
        header.copy_from_slice(&(frame.len() as u32).to_be_bytes());
        write_all(&mut self.writer, &header).await?;
        write_all(&mut self.writer, &frame).await?;
        self.writer
            .flush()
            .await
            .map_err(|error| TransportError::SendFailed(error.kind().to_string()))
    }

    async fn close(&mut self) -> Result<()> {
        self.writer
            .flush()
            .await
            .map_err(|error| TransportError::SendFailed(error.kind().to_string()))
    }
}

impl<R: Read> LocalTransportReceiver for EmbeddedIoReceiver<R> {
    async fn recv(&mut self) -> Result<Option<Bytes>> {
        let mut header = [0u8; HEADER_LEN];
        if read_first_byte(&mut self.reader, &mut header[0]).await? == 0 {
            return Ok(None);
        }
        read_exact(
            &mut self.reader,
            &mut header[1..],
            "connection closed during frame header",
        )
        .await?;
        let frame_len = u32::from_be_bytes(header) as usize;
        if frame_len > self.max_frame_size {
            return Err(TransportError::MessageTooLarge {
                size: frame_len,
                limit: self.max_frame_size,
            });
        }
        let mut payload = BytesMut::zeroed(frame_len);
        read_exact(
            &mut self.reader,
            &mut payload,
            "connection closed during frame payload",
        )
        .await?;
        Ok(Some(payload.freeze()))
    }
}
