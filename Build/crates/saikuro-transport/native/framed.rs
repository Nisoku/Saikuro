use core::pin::Pin;
use core::task::{Context, Poll};

use bytes::{Buf, BufMut};
use futures::{ready, Sink, Stream};
use pin_project_lite::pin_project;
use saikuro_net::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::shared::error::{Result, TransportError};
use crate::shared::framing::LengthPrefixedCodec;

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
                let mut read_buf = ReadBuf::uninit(dst);
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
                if this.read_buf.is_empty() && !this.codec.has_pending_frame() {
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
