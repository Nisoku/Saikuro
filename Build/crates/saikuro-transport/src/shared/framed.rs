#![cfg(any(feature = "embedded", feature = "wasi-tcp"))]

use alloc::string::ToString;
use embedded_io_async::{Read, Write};

use crate::shared::error::{Result, TransportError};

/// Number of big-endian length bytes that prefix every frame.
pub(crate) const HEADER_LEN: usize = 4;

/// Read a single byte from `reader`, writing it to `first` and returning it.
pub(crate) async fn read_first_byte<R: Read>(reader: &mut R, first: &mut u8) -> Result<u8> {
    let mut byte = [0u8; 1];
    reader
        .read_exact(&mut byte)
        .await
        .map_err(|e| TransportError::ConnectionLost(e.to_string()))?;
    *first = byte[0];
    Ok(byte[0])
}

/// Read exactly `buf.len()` bytes from `reader`, or fail with `msg`.
pub(crate) async fn read_exact<R: Read>(reader: &mut R, buf: &mut [u8], msg: &str) -> Result<()> {
    reader
        .read_exact(buf)
        .await
        .map_err(|_| TransportError::ConnectionLost(msg.into()))
}

/// Write every byte of `buf` to `writer`.
pub(crate) async fn write_all<W: Write>(writer: &mut W, buf: &[u8]) -> Result<()> {
    writer
        .write_all(buf)
        .await
        .map_err(|e| TransportError::ConnectionLost(e.to_string()))
}
