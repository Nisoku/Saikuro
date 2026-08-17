//! Embedded (embedded-io-async) adapter for the shared framing core
use crate::shared::error::TransportError;
use crate::shared::framing::{AsyncByteRead, AsyncByteWrite};

impl<R: embedded_io_async::Read> AsyncByteRead for R {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, TransportError> {
        self.read(buf)
            .await
            .map_err(|e| TransportError::ConnectionLost(alloc::format!("{:?}", e)))
    }
}

impl<W: embedded_io_async::Write> AsyncByteWrite for W {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, TransportError> {
        self.write(buf)
            .await
            .map_err(|e| TransportError::ConnectionLost(alloc::format!("{:?}", e)))
    }

    async fn flush(&mut self) -> Result<(), TransportError> {
        embedded_io_async::Write::flush(self)
            .await
            .map_err(|e| TransportError::ConnectionLost(alloc::format!("{:?}", e)))
    }
}
