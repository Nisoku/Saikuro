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
//! | [`websocket`]      | `ws-transport`      | native + wasm32   |
//! | [`wasm_host`]      | always on (wasm32)  | wasm32 only       |

/// Maximum allowed frame size (16 MiB). Frames larger than this are rejected
/// to prevent memory exhaustion from malformed or malicious peers.
pub const MAX_FRAME_SIZE: usize = 16 * 1024 * 1024;

/// Default channel capacity for in-memory transports.
///
/// This bounds memory usage and provides backpressure: if the receiver is
/// slow the sender's `send` call will yield until space frees up.
pub const DEFAULT_CHANNEL_CAPACITY: usize = 256;

pub mod error;
#[cfg(feature = "native-transport")]
pub mod framing;
pub mod memory;
pub mod selector;
pub mod traits;

#[cfg(all(feature = "native-transport", not(target_arch = "wasm32")))]
pub mod tcp;

#[cfg(all(
    feature = "native-transport",
    not(target_arch = "wasm32"),
    target_family = "unix"
))]
pub mod unix;

#[cfg(feature = "ws-transport")]
pub mod websocket;

#[cfg(target_arch = "wasm32")]
pub mod wasm_host;

pub use error::TransportError;
pub use memory::MemoryTransport;
pub use selector::{TransportConfig, TransportKind, TransportSelector};
pub use traits::{Transport, TransportReceiver, TransportSender};

#[cfg(all(feature = "native-transport", not(target_arch = "wasm32")))]
pub use tcp::TcpTransport;

#[cfg(all(
    feature = "native-transport",
    not(target_arch = "wasm32"),
    target_family = "unix"
))]
pub use unix::UnixTransport;

#[cfg(feature = "ws-transport")]
pub use websocket::WebSocketTransport;

#[cfg(all(feature = "ws-transport", not(target_arch = "wasm32")))]
pub use websocket::WsTransportListener;

#[cfg(target_arch = "wasm32")]
pub use wasm_host::WasmHostTransport;
