//! Transport selector:  automatic best-transport choice plus manual overrides.
//!
//! Rather than forcing callers to know which transport to use, the
//! [`TransportSelector`] inspects the target address and the current platform
//! and picks the most efficient backend automatically.
//!
//! | Condition                                     | Chosen transport  |
//! ---------------------------------------------------------------------
//! | Target is the same process                    | In-memory         |
//! | Target is on the same machine (Unix)          | Unix socket       |
//! | Target is on the same machine (non-Unix)      | TCP loopback      |
//! | Target is remote, WASM context                | BroadcastChannel  |
//! | Target is remote, native context              | TCP               |
//!
//! The user can override any of these choices by supplying an explicit
//! [`TransportConfig`].

use serde::{Deserialize, Serialize};

/// The set of transport backends Saikuro knows about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportKind {
    /// In-process MPSC channels:  zero-copy, zero-latency.
    Memory,
    /// Unix domain sockets:  intra-machine on Unix.
    #[cfg(all(not(target_arch = "wasm32"), target_family = "unix"))]
    Unix,
    /// Raw TCP stream:  cross-machine native.
    #[cfg(not(target_arch = "wasm32"))]
    Tcp,
    /// WebSocket:  cross-machine and WASM-compatible.
    WebSocket,
    /// BroadcastChannel host transport:  WASM in-browser communication.
    #[cfg(target_arch = "wasm32")]
    WasmHost,
}

/// User-supplied transport configuration.
///
/// When `None` is passed where a `TransportConfig` is expected, the selector
/// falls back to automatic detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    /// Force a specific transport kind regardless of the target address.
    pub kind: TransportKind,

    /// Address string whose format depends on `kind`:
    /// - `Memory`: ignored
    /// - `Unix`: filesystem path to the socket
    /// - `Tcp`: `"host:port"`
    /// - `WebSocket`: full URL `"ws://host:port/path"`
    pub address: Option<String>,

    /// Maximum message size in bytes. Defaults to 16 MiB.
    #[serde(default = "default_max_message_size")]
    pub max_message_size: usize,

    /// Send-buffer capacity in frames (for memory) or bytes (for stream transports).
    #[serde(default = "default_send_buffer")]
    pub send_buffer: usize,
}

fn default_max_message_size() -> usize {
    crate::MAX_FRAME_SIZE
}

fn default_send_buffer() -> usize {
    256
}

/// Automatic transport selector.
pub struct TransportSelector;

impl TransportSelector {
    /// Choose the best transport for a given target address, respecting any
    /// user override in `config`.
    ///
    /// Returns a [`TransportKind`] describing which backend to construct and
    /// the resolved address string (if applicable).
    pub fn select(
        target_address: Option<&str>,
        override_config: Option<&TransportConfig>,
    ) -> (TransportKind, Option<String>) {
        // Explicit override wins unconditionally.
        if let Some(cfg) = override_config {
            return (cfg.kind.clone(), cfg.address.clone());
        }

        let addr = match target_address {
            None => return (TransportKind::Memory, None),
            Some(a) => a,
        };

        // In-process: no address needed.
        if addr == "memory" || addr.is_empty() {
            return (TransportKind::Memory, None);
        }

        // WASM: prefer BroadcastChannel host transport for in-realm communication.
        #[cfg(target_arch = "wasm32")]
        {
            if addr == "wasm-host" || addr.starts_with("wasm-host://") {
                return (TransportKind::WasmHost, Some(addr.to_owned()));
            }
            let ws_url = if addr.starts_with("ws://") || addr.starts_with("wss://") {
                addr.to_owned()
            } else {
                format!("ws://{}", addr)
            };
            return (TransportKind::WebSocket, Some(ws_url));
        }

        // Unix socket: path-like address on a Unix host.
        #[cfg(all(not(target_arch = "wasm32"), target_family = "unix"))]
        if addr.starts_with('/') || addr.starts_with('.') {
            return (TransportKind::Unix, Some(addr.to_owned()));
        }

        // WebSocket URL.
        #[cfg(not(target_arch = "wasm32"))]
        if addr.starts_with("ws://") || addr.starts_with("wss://") {
            return (TransportKind::WebSocket, Some(addr.to_owned()));
        }

        // TCP (non-WASM) or memory (WASM fallback).
        if cfg!(target_arch = "wasm32") {
            (TransportKind::Memory, None)
        } else {
            (TransportKind::Tcp, Some(addr.to_owned()))
        }
    }
}
