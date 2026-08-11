//! Saikuro Transport
//!
//! This crate defines the [`Transport`] trait and provides concrete
//! implementations:
//!
//! | Backend            | Feature flag         | Platforms         |
//! |--------------------|---------------------|-------------------|
//! | [`memory`]         | always on           | native + wasm32   |
//! | [`unix`]           | `native-transport`  | Unix only         |
//! | [`tcp`]            | `native-transport`  | native only       |
//! | [`websocket`]      | `native-ws`/wasm32  | native + wasm32   |
//! | [`wasm_host`]      | always on (wasm32)  | wasm32 only       |
//!
//! The crate is `no_std` + `alloc` without the `std` feature; the in-memory
//! transport, selector, traits, and error types compile for bare-metal MCU
//! targets.  Native backends (Unix/TCP/WebSocket) require `std`.

#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;

pub mod error;
pub mod framing;
pub mod memory;
pub mod selector;
pub mod traits;

#[cfg(feature = "embedded-io")]
pub mod embedded_io;

#[cfg(all(feature = "native-transport", not(target_arch = "wasm32")))]
pub mod tcp;

#[cfg(all(
    feature = "native-transport",
    not(target_arch = "wasm32"),
    target_family = "unix"
))]
pub mod unix;

#[cfg(all(
    feature = "ws-transport",
    any(feature = "native-ws", target_arch = "wasm32")
))]
pub mod websocket;

#[cfg(target_arch = "wasm32")]
pub mod wasm_host;

pub use error::TransportError;
pub use memory::MemoryTransport;
pub use selector::{TransportConfig, TransportKind, TransportSelector};
pub use traits::{Transport, TransportReceiver, TransportSender};

#[cfg(feature = "embedded-io")]
pub use embedded_io::{
    EmbeddedIoReceiver, EmbeddedIoSender, EmbeddedIoTransport, LocalTransportReceiver,
    LocalTransportSender,
};

#[cfg(all(feature = "native-transport", not(target_arch = "wasm32")))]
pub use tcp::TcpTransport;

#[cfg(all(
    feature = "native-transport",
    not(target_arch = "wasm32"),
    target_family = "unix"
))]
pub use unix::UnixTransport;

#[cfg(all(
    feature = "ws-transport",
    any(feature = "native-ws", target_arch = "wasm32")
))]
pub use websocket::WebSocketTransport;

#[cfg(all(feature = "native-ws", not(target_arch = "wasm32")))]
pub use websocket::WsTransportListener;

#[cfg(target_arch = "wasm32")]
pub use wasm_host::WasmHostTransport;

/// Maximum allowed frame size (16 MiB). Frames larger than this are rejected
/// to prevent memory exhaustion from malformed or malicious peers.
pub const MAX_FRAME_SIZE: usize = 16 * 1024 * 1024;

/// Implements [`TransportSender`] for a sender type whose `inner` field
/// implements `Sink<Bytes>`.  `$addr_field` is the struct field (logged with
/// `Debug` on every send/close).
#[macro_export]
macro_rules! impl_native_sender {
    ($sender:ty, $addr_field:ident, $transport:literal) => {
        #[async_trait::async_trait]
        impl $crate::traits::TransportSender for $sender {
            async fn send(&mut self, frame: bytes::Bytes) -> $crate::error::Result<()> {
                tracing::trace!($addr_field = ?self.$addr_field, bytes = frame.len(), concat!($transport, " send"));
                futures::SinkExt::send(&mut self.inner, frame).await
            }

            async fn close(&mut self) -> $crate::error::Result<()> {
                tracing::debug!($addr_field = ?self.$addr_field, concat!($transport, " sender closing"));
                futures::SinkExt::close(&mut self.inner).await
            }
        }
    };
}

/// Implements [`TransportReceiver`] for a receiver type whose `inner` field
/// implements `Stream<Item = io::Result<Bytes>>`.
#[macro_export]
macro_rules! impl_native_receiver {
    ($receiver:ty, $addr_field:ident, $transport:literal) => {
        #[async_trait::async_trait]
        impl $crate::traits::TransportReceiver for $receiver {
            async fn recv(&mut self) -> $crate::error::Result<Option<bytes::Bytes>> {
                match futures::StreamExt::next(&mut self.inner).await {
                    Some(Ok(bytes)) => {
                        tracing::trace!($addr_field = ?self.$addr_field, bytes = bytes.len(), concat!($transport, " recv"));
                        Ok(Some(bytes))
                    }
                    Some(Err(e)) => Err($crate::error::TransportError::from(e)),
                    None => {
                        tracing::debug!($addr_field = ?self.$addr_field, concat!($transport, " connection closed by peer"));
                        Ok(None)
                    }
                }
            }
        }
    };
}

/// Default channel capacity for in-memory transports.
///
/// This bounds memory usage and provides backpressure: if the receiver is
/// slow the sender's `send` call will yield until space frees up.
pub const DEFAULT_CHANNEL_CAPACITY: saikuro_exec::ChannelCapacity =
    saikuro_exec::ChannelCapacity::MAX;
