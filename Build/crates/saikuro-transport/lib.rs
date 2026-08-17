//! Pluggable, backend-agnostic transports for Saikuro.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(async_fn_in_trait)]

#[macro_use]
extern crate alloc;

#[cfg(not(any(
    feature = "native",
    feature = "no_std",
    feature = "wasm",
    feature = "embedded"
)))]
compile_error!(
    "saikuro-transport: enable exactly one engine feature: native, no_std, wasm, or embedded"
);

#[cfg(any(
    all(feature = "native", feature = "no_std"),
    all(feature = "native", feature = "wasm"),
    all(feature = "native", feature = "embedded"),
    all(feature = "no_std", feature = "wasm"),
    all(feature = "no_std", feature = "embedded"),
    all(feature = "wasm", feature = "embedded")
))]
compile_error!(
    "saikuro-transport: enable exactly one engine feature (native, no_std, wasm, embedded), not more"
);

#[cfg(all(feature = "no_std", feature = "std"))]
compile_error!(
    "saikuro-transport: the no_std engine cannot be combined with the std toolchain feature"
);

#[cfg(all(feature = "wasm", feature = "ws", not(feature = "std")))]
compile_error!(
    "saikuro-transport: browser wasm (no_std) has no WebSocket socket API; use WASI \
     (wasm32-wasip1/wasip2) with the `ws-wasi` feature for wasm-no_std WebSocket clients, \
     or enable `std` on the wasm engine."
);

pub mod shared;

#[cfg(feature = "native")]
pub mod native;
#[cfg(all(feature = "native", feature = "tcp"))]
pub use native::tcp;
#[cfg(all(feature = "native", feature = "unix"))]
pub use native::unix;
#[cfg(all(feature = "native", feature = "ws"))]
pub use native::websocket;

/// Transport selection and configuration types.
#[cfg(any(
    feature = "native",
    feature = "no_std",
    feature = "wasm",
    feature = "embedded"
))]
pub use shared::selector;

#[cfg(feature = "embedded")]
pub mod embedded;

#[cfg(feature = "wasm")]
pub mod wasm;

#[cfg(feature = "no_std")]
pub mod wasi;

pub use shared::error::TransportError;
pub use shared::host::{
    HostPipeFactory, HostPipeRecv, HostPipeSend, Role, WasmHostConnector, WasmHostListener,
    WasmHostTransport,
};
pub use shared::memory::MemoryTransport;
pub use shared::selector::{TransportConfig, TransportKind, TransportSelector};
pub use shared::traits::{
    LocalTransport, LocalTransportConnector, LocalTransportListener, LocalTransportReceiver,
    LocalTransportSender, Transport, TransportConnector, TransportListener, TransportReceiver,
    TransportSender,
};

#[cfg(all(feature = "native", feature = "tcp"))]
pub use native::tcp::TcpTransport;
#[cfg(all(feature = "native", feature = "unix", target_family = "unix"))]
pub use native::unix::UnixTransport;
#[cfg(all(feature = "native", feature = "ws"))]
pub use native::websocket::{WebSocketTransport, WsTransportListener};

#[cfg(feature = "embedded")]
pub use embedded::io_transport::{EmbeddedIoReceiver, EmbeddedIoSender, EmbeddedIoTransport};
#[cfg(all(feature = "embedded", feature = "tcp"))]
pub use embedded::tcp::TcpTransport;

#[cfg(all(feature = "wasm", feature = "wasm-host"))]
pub use wasm::host_browser::{BroadcastChannelPipe, WasmHost};
#[cfg(all(feature = "wasm", feature = "ws", feature = "std"))]
pub use wasm::websocket::WebSocketTransport;

#[cfg(all(feature = "no_std", feature = "ws-wasi"))]
pub use wasi::websocket;

#[cfg(all(feature = "no_std", feature = "wasi-host"))]
pub use wasi::host::{WasiHostRecv, WasiHostSend, WasiPipe};
#[cfg(all(feature = "no_std", feature = "wasi-tcp"))]
pub use wasi::tcp::{WasiTcpConnector, WasiTcpListener, WasiTcpTransport};

/// Maximum allowed frame size (16 MiB).  Frames larger than this are rejected
/// to prevent memory exhaustion from malformed or malicious peers.
pub const MAX_FRAME_SIZE: usize = 16 * 1024 * 1024;

/// Default capacity of internal transport channels.
pub const DEFAULT_CHANNEL_CAPACITY: saikuro_exec::ChannelCapacity =
    saikuro_exec::ChannelCapacity::MAX;

/// Implements [`TransportSender`] for a native transport's sending half.
#[macro_export]
macro_rules! impl_native_sender {
    ($ty:ty, $addr:ident, $desc:literal) => {
        #[async_trait::async_trait]
        impl $crate::shared::traits::TransportSender for $ty {
            async fn send(&mut self, frame: ::bytes::Bytes) -> $crate::shared::error::Result<()> {
                tracing::trace!($addr = ?self.$addr, bytes = frame.len(), concat!($desc, " send"));
                $crate::shared::framing::write_frame(&mut self.inner, &frame).await
            }

            async fn close(&mut self) -> $crate::shared::error::Result<()> {
                tracing::debug!($addr = ?self.$addr, concat!($desc, " sender closing"));
                $crate::shared::framing::AsyncByteWrite::flush(&mut self.inner).await
            }
        }
    };
}

/// Implements [`TransportReceiver`] for a native transport's receiving half.
#[macro_export]
macro_rules! impl_native_receiver {
    ($ty:ty, $addr:ident, $desc:literal) => {
        #[async_trait::async_trait]
        impl $crate::shared::traits::TransportReceiver for $ty {
            async fn recv(
                &mut self,
            ) -> $crate::shared::error::Result<Option<::bytes::Bytes>> {
                match $crate::shared::framing::read_frame(&mut self.inner).await {
                    Ok(bytes) => {
                        match &bytes {
                            Some(b) => tracing::trace!(
                                $addr = ?self.$addr,
                                bytes = b.len(),
                                concat!($desc, " recv")
                            ),
                            None => tracing::debug!(
                                $addr = ?self.$addr,
                                concat!($desc, " connection closed by peer")
                            ),
                        }
                        Ok(bytes)
                    }
                    Err(e) => Err(e),
                }
            }
        }
    };
}
