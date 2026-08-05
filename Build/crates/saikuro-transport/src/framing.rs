//! Length-prefixed framing for byte-stream transports.
//!
//! Raw stream transports (TCP, Unix sockets) deliver an unbroken river of
//! bytes with no inherent message boundaries.  We impose message framing with
//! a simple 4-byte big-endian length prefix before every frame:
//!
//! +--------+------------------------------+
//! |  u32   |         payload              |
//! |  len   |   len bytes                  |
//! +--------+------------------------------+
//!
//! The [`LengthPrefixedCodec`] is pure byte-slicing over `BytesMut` and
//! compiles without `std` or any tokio dependency, so the same framing logic
//! is reused verbatim by native transports and the future embedded-io
//! backend.  [`FramedStream`] wraps the codec around an async byte stream and
//! is the native (`tokio`) adapter used by [`crate::tcp::TcpTransport`] and
//! [`crate::unix::UnixTransport`].

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::error::{Result, TransportError};

pub use crate::MAX_FRAME_SIZE;

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

    /// Decode the next complete frame from `src`, returning `Ok(None)` until a
    /// full frame is buffered.  Consumes the header and payload from the front
    /// of `src` when a frame is returned.  A header over the size limit yields
    /// `MessageTooLarge` and the codec then discards the declared payload on
    /// subsequent calls so it resynchronizes at the next real header.
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

/// Async byte stream framed into discrete messages.
///
/// Drop-in replacement for `tokio_util::codec::Framed` that stays within this
/// crate's own codec so we don't depend on tokio-util for framing.  Implements
/// `Stream<Item = Result<Bytes>>` for reads and `Sink<Bytes>` for writes; use
/// [`FramedStream::split`] to obtain independent halves.
#[cfg(feature = "native-transport")]
pub mod framed {
    use core::pin::Pin;
    use core::task::{Context, Poll};

    use bytes::{Buf, BufMut};
    use futures::{ready, Sink, Stream};
    use pin_project_lite::pin_project;
    use saikuro_exec::io::{AsyncRead, AsyncWrite};

    use super::LengthPrefixedCodec;
    use crate::error::{Result, TransportError};

    /// Minimum capacity to make available for each read when no frame is
    /// pending.  Large enough to amortize syscalls without over-committing
    /// memory on small frames; when a frame is pending, decode reserves the
    /// exact remaining frame bytes so the read spans the whole frame.
    const READ_CHUNK: usize = 4096;

    pin_project! {
        pub struct FramedStream<S> {
            #[pin]
            inner: S,
            codec: LengthPrefixedCodec,
            read_buf: bytes::BytesMut,
            write_buf: bytes::BytesMut,
            // Set once a framing, I/O, or truncation error is surfaced so the
            // stream stays terminal and later polls report the end instead of
            // resuming on an unaligned byte stream.
            failed: bool,
        }
    }

    impl<S: AsyncRead + AsyncWrite> FramedStream<S> {
        pub fn new(inner: S) -> Self {
            Self {
                inner,
                codec: LengthPrefixedCodec::new(),
                read_buf: bytes::BytesMut::new(),
                write_buf: bytes::BytesMut::new(),
                failed: false,
            }
        }

        /// Split into a sink (write half) and a stream (read half).
        ///
        /// `StreamExt::split` produces both halves from a single underlying
        /// `BiLock` so they stay safely paired.
        pub fn split(
            self,
        ) -> (
            futures::stream::SplitSink<Self, bytes::Bytes>,
            futures::stream::SplitStream<Self>,
        ) {
            futures::StreamExt::split::<bytes::Bytes>(self)
        }
    }

    impl<S: AsyncRead + AsyncWrite> Stream for FramedStream<S> {
        type Item = Result<bytes::Bytes>;

        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            let mut this = self.project();

            if *this.failed {
                return Poll::Ready(None);
            }

            loop {
                // decode any complete frames already buffered.
                match this.codec.decode(this.read_buf) {
                    Ok(Some(frame)) => return Poll::Ready(Some(Ok(frame))),
                    Ok(None) => {}
                    Err(e) => {
                        // Corrupt or oversized frame; the byte stream is no
                        // longer aligned, so surface the error and terminate.
                        *this.failed = true;
                        return Poll::Ready(Some(Err(e)));
                    }
                }

                // Read directly into the uninitialized tail of read_buf.  When
                // a frame is pending, decode already reserved the remaining
                // frame bytes so chunk_mut spans the whole frame; otherwise
                // reserve the chunk size so the read still has a writable
                // target.  advance_mut only appends the filled bytes, so a
                // Pending read leaves no phantom bytes behind.
                this.read_buf.reserve(READ_CHUNK);
                let filled = {
                    let dst = this.read_buf.chunk_mut();
                    // SAFETY: chunk_mut borrows the uninitialized tail of the
                    // buffer; the slice is only filled by poll_read below
                    // before we advance_mut by the filled length.
                    let dst = unsafe { dst.as_uninit_slice_mut() };
                    let mut read_buf = saikuro_exec::io::ReadBuf::uninit(dst);
                    match ready!(this.inner.as_mut().poll_read(cx, &mut read_buf)) {
                        Ok(()) => read_buf.filled().len(),
                        Err(e) => {
                            *this.failed = true;
                            return Poll::Ready(Some(Err(TransportError::from(e))));
                        }
                    }
                };
                // SAFETY: poll_read initialized the first `filled` bytes.
                unsafe { this.read_buf.advance_mut(filled) };

                if filled == 0 {
                    // EOF from the peer.  A clean close happens only at a
                    // frame boundary; leftover bytes mean a truncated frame.
                    if this.read_buf.is_empty() {
                        return Poll::Ready(None);
                    }
                    *this.failed = true;
                    return Poll::Ready(Some(Err(TransportError::FramingError(
                        "connection closed mid-frame".into(),
                    ))));
                }

                // More bytes arrived; loop back to decode them.
            }
        }
    }

    impl<S: AsyncRead + AsyncWrite> Sink<bytes::Bytes> for FramedStream<S> {
        type Error = TransportError;

        fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
            if self.as_ref().project_ref().write_buf.is_empty() {
                return Poll::Ready(Ok(()));
            }
            self.poll_flush(cx)
        }

        fn start_send(self: Pin<&mut Self>, item: bytes::Bytes) -> Result<()> {
            let this = self.project();
            this.codec.encode(item, this.write_buf)?;
            Ok(())
        }

        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
            ready!(flush_write_buf(self.as_mut(), cx))?;
            self.project()
                .inner
                .poll_flush(cx)
                .map_err(TransportError::from)
        }

        fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
            ready!(flush_write_buf(self.as_mut(), cx))?;
            let flushed = ready!(self.as_mut().project().inner.poll_flush(cx));
            match flushed {
                Err(e) => Poll::Ready(Err(TransportError::from(e))),
                Ok(()) => self
                    .project()
                    .inner
                    .poll_shutdown(cx)
                    .map_err(TransportError::from),
            }
        }
    }

    /// Drain `write_buf` into the underlying stream until it is empty.
    fn flush_write_buf<S: AsyncWrite>(
        mut stream: Pin<&mut FramedStream<S>>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<()>> {
        while !stream.as_ref().project_ref().write_buf.is_empty() {
            let this = stream.as_mut().project();
            let n = match ready!(this.inner.poll_write(cx, this.write_buf)) {
                Ok(n) => n,
                Err(e) => return Poll::Ready(Err(TransportError::from(e))),
            };
            if n == 0 {
                // The stream refuses to take bytes; treat as a write failure
                // rather than spinning forever.
                return Poll::Ready(Err(TransportError::FramingError(
                    "write made no progress".into(),
                )));
            }
            this.write_buf.advance(n);
        }
        Poll::Ready(Ok(()))
    }
}

#[cfg(feature = "native-transport")]
pub use framed::FramedStream;
