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

#[cfg(all(feature = "no_std", feature = "std", not(target_os = "wasi")))]
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
#[cfg(feature = "native")]
#[allow(unused_imports)]
pub use native::*;

#[cfg(feature = "embedded")]
pub mod embedded;
#[cfg(feature = "embedded")]
#[allow(unused_imports)]
pub use embedded::*;

#[cfg(feature = "wasm")]
pub mod wasm;
#[cfg(feature = "wasm")]
#[allow(unused_imports)]
pub use wasm::*;

#[cfg(feature = "no_std")]
pub mod wasi;
#[cfg(feature = "no_std")]
#[allow(unused_imports)]
pub use wasi::*;

pub use shared::adapter::{AdapterTransport, MemoryAdapterTransport, connect};
pub use shared::error::TransportError;
pub use shared::host::{
    HostPipeFactory, HostPipeRecv, HostPipeSend, Role, WasmHostConnector, WasmHostListener,
    WasmHostTransport,
};
pub use shared::memory::MemoryTransport;
pub use shared::selector::{self, TransportConfig, TransportKind, TransportSelector};
pub use shared::traits::{
    LocalTransport, LocalTransportConnector, LocalTransportListener, LocalTransportReceiver,
    LocalTransportSender, Transport, TransportConnector, TransportListener, TransportReceiver,
    TransportSender,
};

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
                use ::saikuro_event::{LogLevel, LogRecord};
                let mut record = LogRecord::now(
                    LogLevel::Trace,
                    concat!("saikuro.transport.", $desc),
                    concat!($desc, " send"),
                );
                record.set_context("_addr", ::alloc::format!("{:?}", self.$addr));
                record.set_context("bytes", frame.len() as u64);
                self.log.emit(&record).await;
                $crate::shared::framing::write_frame(&mut self.inner, &frame).await
            }

            async fn close(&mut self) -> $crate::shared::error::Result<()> {
                use ::saikuro_event::{LogLevel, LogRecord};
                let mut record = LogRecord::now(
                    LogLevel::Debug,
                    concat!("saikuro.transport.", $desc),
                    concat!($desc, " sender closing"),
                );
                record.set_context("_addr", ::alloc::format!("{:?}", self.$addr));
                self.log.emit(&record).await;
                $crate::shared::framing::AsyncByteWrite::flush(&mut self.inner).await
            }
        }
    };
}

/// Implements [`TransportReceiver`] for a native transport's receiving half.
///
/// The concrete type must have a `log: Arc<dyn ::saikuro_event::LogSink>` field.
#[macro_export]
macro_rules! impl_native_receiver {
    ($ty:ty, $addr:ident, $desc:literal) => {
        #[async_trait::async_trait]
        impl $crate::shared::traits::TransportReceiver for $ty {
            async fn recv(&mut self) -> $crate::shared::error::Result<Option<::bytes::Bytes>> {
                match $crate::shared::framing::read_frame(&mut self.inner).await {
                    Ok(bytes) => {
                        match &bytes {
                            Some(b) => {
                                use ::saikuro_event::{LogLevel, LogRecord};
                                let mut record = LogRecord::now(
                                    LogLevel::Trace,
                                    concat!("saikuro.transport.", $desc),
                                    concat!($desc, " recv"),
                                );
                                record.set_context("_addr", ::alloc::format!("{:?}", self.$addr));
                                record.set_context("bytes", b.len() as u64);
                                self.log.emit(&record).await;
                            }
                            None => {
                                use ::saikuro_event::{LogLevel, LogRecord};
                                let mut record = LogRecord::now(
                                    LogLevel::Debug,
                                    concat!("saikuro.transport.", $desc),
                                    concat!($desc, " connection closed by peer"),
                                );
                                record.set_context("_addr", ::alloc::format!("{:?}", self.$addr));
                                self.log.emit(&record).await;
                            }
                        }
                        Ok(bytes)
                    }
                    Err(e) => Err(e),
                }
            }
        }
    };
}
