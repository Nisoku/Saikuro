//! Saikuro Transport
//!
//! This crate defines the [`Transport`] trait and provides concrete
//! implementations:
//!
//! | Backend            | Feature flag        | Platforms         |
//! |--------------------|---------------------|-------------------|
//! | [`memory`]         | always on           | native + wasm32   |
//! | [`unix`]           | `native-transport`  | Unix only         |
//! | [`tcp`]            | `native-transport`  | native only       |
//! | [`websocket`]      | `ws-transport`      | native + wasm32   |
//! (idk, tables are cool-looking don't ask)

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
