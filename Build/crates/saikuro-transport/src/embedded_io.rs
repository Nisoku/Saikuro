use alloc::string::ToString;
use bytes::{Bytes, BytesMut};
use core::future::Future;
use embedded_io_async::{Error, Read, Write};

use crate::error::{Result, TransportError};

const HEADER_LEN: usize = 4;

/// A local, statically-dispatched sender for a transport.
pub trait LocalTransportSender {
    /// Send one length-prefixed binary frame and flush it to the writer.
    fn send(&mut self, frame: Bytes) -> impl Future<Output = Result<()>> + '_;

    /// Flush and close the writer if its implementation supports shutdown.
    fn close(&mut self) -> impl Future<Output = Result<()>> + '_;
}

/// A local, statically-dispatched receiver for a transport.
pub trait LocalTransportReceiver {
    /// Receive the next frame. `Ok(None)` is a clean EOF at a frame boundary.
    fn recv(&mut self) -> impl Future<Output = Result<Option<Bytes>>> + '_;
}

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
    /// Creates a transport and rejects limits above [`crate::MAX_FRAME_SIZE`].
    ///
    /// No payload allocation occurs during construction or while inspecting a
    /// hostile header. A zero limit is valid and permits only empty frames.
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

    /// Splits the transport into its separately-owned local halves.
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

        let mut header = [0; HEADER_LEN];
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
        let mut header = [0; HEADER_LEN];
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

async fn read_first_byte<R: Read>(reader: &mut R, byte: &mut u8) -> Result<usize> {
    reader
        .read(core::slice::from_mut(byte))
        .await
        .map_err(|error| TransportError::ReceiveFailed(error.kind().to_string()))
}

async fn read_exact<R: Read>(
    reader: &mut R,
    mut target: &mut [u8],
    eof_message: &'static str,
) -> Result<()> {
    while !target.is_empty() {
        let count = reader
            .read(target)
            .await
            .map_err(|error| TransportError::ReceiveFailed(error.kind().to_string()))?;
        if count == 0 {
            return Err(TransportError::FramingError(eof_message.into()));
        }
        target = &mut target[count..];
    }
    Ok(())
}

async fn write_all<W: Write>(writer: &mut W, mut source: &[u8]) -> Result<()> {
    while !source.is_empty() {
        let count = writer
            .write(source)
            .await
            .map_err(|error| TransportError::SendFailed(error.kind().to_string()))?;
        if count == 0 {
            return Err(TransportError::FramingError(
                "write made no progress".into(),
            ));
        }
        source = &source[count..];
    }
    Ok(())
}
