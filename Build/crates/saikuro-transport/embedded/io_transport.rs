use alloc::boxed::Box;
use async_trait::async_trait;
use bytes::Bytes;
use embedded_io_async::{Read, Write};

use crate::shared::error::{Result, TransportError};
use crate::shared::framing::{read_frame, write_frame};
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
            },
        )
    }
}

#[async_trait(?Send)]
impl<W: Write + 'static> LocalTransportSender for EmbeddedIoSender<W> {
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        if frame.len() > self.max_frame_size {
            return Err(TransportError::MessageTooLarge {
                size: frame.len(),
                limit: self.max_frame_size,
            });
        }
        write_frame(&mut self.writer, &frame).await
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}

#[async_trait(?Send)]
impl<R: Read + 'static> LocalTransportReceiver for EmbeddedIoReceiver<R> {
    async fn recv(&mut self) -> Result<Option<Bytes>> {
        read_frame(&mut self.reader).await
    }
}
