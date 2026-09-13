//! Native (tokio) adapter for the shared framing core.
use saikuro_net::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::shared::error::TransportError;
use crate::shared::framing::{AsyncByteRead, AsyncByteWrite};

impl<R: AsyncRead + Unpin> AsyncByteRead for R {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, TransportError> {
        AsyncReadExt::read(self, buf)
            .await
            .map_err(|e| TransportError::ConnectionLost(e.to_string()))
    }
}

impl<W: AsyncWrite + Unpin> AsyncByteWrite for W {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, TransportError> {
        AsyncWriteExt::write(self, buf)
            .await
            .map_err(|e| TransportError::ConnectionLost(e.to_string()))
    }

    async fn flush(&mut self) -> Result<(), TransportError> {
        AsyncWriteExt::flush(self)
            .await
            .map_err(|e| TransportError::ConnectionLost(e.to_string()))
    }
}

pub use crate::shared::framing::{read_frame, write_frame};
