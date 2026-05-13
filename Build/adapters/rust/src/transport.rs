//! Transport connection helpers.
//!
//! Parses address strings like `"tcp://127.0.0.1:7700"`, `"ws://..."`, or
//! `"unix:///tmp/saikuro.sock"` and returns a connected boxed transport.

use bytes::Bytes;

use crate::error::{Error, Result};

/// A URL-style address string understood by the Saikuro adapter.
///
/// Supported schemes:
/// - `tcp://host:port`
/// - `ws://host:port`  (requires `ws` feature)
/// - `unix:///path/to/socket`  (requires `unix` feature, Unix only)
/// - `wasm-host://channel-name` (WASM only, feature `wasm`)
/// - `wasm-host` (uses default channel "saikuro")
pub struct Address(pub String);

impl<S: Into<String>> From<S> for Address {
    fn from(s: S) -> Self {
        Self(s.into())
    }
}

/// A trait-object-compatible trait for sending and receiving framed byte buffers.
///
/// This is a thin adapter over the underlying saikuro-transport types so that
/// the Provider and Client don't need to be generic over the concrete transport.
#[async_trait::async_trait]
pub trait AdapterTransport: Send + 'static {
    async fn send(&mut self, frame: Bytes) -> Result<()>;
    async fn recv(&mut self) -> Result<Option<Bytes>>;
    async fn close(&mut self) -> Result<()>;
}

// Concrete implementations for each transport backend.

// TCP

#[cfg(all(feature = "tcp", not(target_arch = "wasm32")))]
mod tcp_impl {
    use super::*;
    use saikuro_transport::tcp::{TcpReceiver, TcpSender};
    use saikuro_transport::{
        traits::{TransportReceiver, TransportSender},
        TcpTransport,
    };

    pub struct TcpAdapter {
        sender: TcpSender,
        receiver: TcpReceiver,
    }

    impl TcpAdapter {
        pub async fn connect(addr: std::net::SocketAddr) -> Result<Self> {
            use saikuro_transport::traits::Transport;
            let transport = TcpTransport::new(
                saikuro_exec::net::TcpStream::connect(addr)
                    .await
                    .map_err(|e| Error::Transport(e.to_string()))?,
            )
            .map_err(|e| Error::Transport(e.to_string()))?;
            let (sender, receiver) = transport.split();
            Ok(Self { sender, receiver })
        }
    }

    #[async_trait::async_trait]
    impl AdapterTransport for TcpAdapter {
        async fn send(&mut self, frame: Bytes) -> Result<()> {
            self.sender.send(frame).await.map_err(Into::into)
        }

        async fn recv(&mut self) -> Result<Option<Bytes>> {
            self.receiver.recv().await.map_err(Into::into)
        }

        async fn close(&mut self) -> Result<()> {
            self.sender.close().await.map_err(Into::into)
        }
    }

    pub async fn connect_tcp(host: &str, port: u16) -> Result<Box<dyn AdapterTransport>> {
        let addr: std::net::SocketAddr = format!("{host}:{port}")
            .parse()
            .map_err(|e: std::net::AddrParseError| Error::Transport(e.to_string()))?;
        Ok(Box::new(TcpAdapter::connect(addr).await?))
    }
}

// Unix socket

#[cfg(all(feature = "unix", not(target_arch = "wasm32"), target_family = "unix"))]
mod unix_impl {
    use super::*;
    use saikuro_transport::traits::TransportConnector;
    use saikuro_transport::unix::UnixConnector;
    use saikuro_transport::{
        traits::{Transport, TransportReceiver, TransportSender},
        unix::{UnixReceiver, UnixSender},
    };

    pub struct UnixAdapter {
        sender: UnixSender,
        receiver: UnixReceiver,
    }

    impl UnixAdapter {
        pub async fn connect(path: &str) -> Result<Self> {
            let connector = UnixConnector::new(path);
            let transport = connector
                .connect()
                .await
                .map_err(|e| Error::Transport(e.to_string()))?;
            let (sender, receiver) = transport.split();
            Ok(Self { sender, receiver })
        }
    }

    #[async_trait::async_trait]
    impl AdapterTransport for UnixAdapter {
        async fn send(&mut self, frame: Bytes) -> Result<()> {
            self.sender.send(frame).await.map_err(Into::into)
        }

        async fn recv(&mut self) -> Result<Option<Bytes>> {
            self.receiver.recv().await.map_err(Into::into)
        }

        async fn close(&mut self) -> Result<()> {
            self.sender.close().await.map_err(Into::into)
        }
    }

    pub async fn connect_unix(path: &str) -> Result<Box<dyn AdapterTransport>> {
        Ok(Box::new(UnixAdapter::connect(path).await?))
    }
}

// WebSocket

#[cfg(feature = "ws")]
mod ws_impl {
    use super::*;
    use saikuro_transport::{
        traits::{Transport, TransportReceiver, TransportSender},
        websocket::{WebSocketReceiver, WebSocketSender},
        WebSocketTransport,
    };

    pub struct WsAdapter {
        sender: WebSocketSender,
        receiver: WebSocketReceiver,
    }

    impl WsAdapter {
        pub async fn connect(url: &str) -> Result<Self> {
            let transport = WebSocketTransport::connect(url)
                .await
                .map_err(|e| Error::Transport(e.to_string()))?;
            let (sender, receiver) = transport.split();
            Ok(Self { sender, receiver })
        }
    }

    #[async_trait::async_trait]
    impl AdapterTransport for WsAdapter {
        async fn send(&mut self, frame: Bytes) -> Result<()> {
            self.sender.send(frame).await.map_err(Into::into)
        }

        async fn recv(&mut self) -> Result<Option<Bytes>> {
            self.receiver.recv().await.map_err(Into::into)
        }

        async fn close(&mut self) -> Result<()> {
            self.sender.close().await.map_err(Into::into)
        }
    }

    pub async fn connect_ws(url: &str) -> Result<Box<dyn AdapterTransport>> {
        Ok(Box::new(WsAdapter::connect(url).await?))
    }
}

// WasmHost (BroadcastChannel for WASM in-browser communication)

#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
mod wasm_host_impl {
    use super::*;
    use saikuro_transport::{
        traits::{Transport, TransportReceiver, TransportSender},
        wasm_host::{WasmHostConnector, WasmHostReceiver, WasmHostSender},
    };

    const DEFAULT_WASM_HOST_CHANNEL: &str = "saikuro";

    pub struct WasmHostAdapter {
        sender: WasmHostSender,
        receiver: WasmHostReceiver,
    }

    impl WasmHostAdapter {
        pub async fn connect(channel_name: &str) -> Result<Self> {
            use saikuro_transport::traits::TransportConnector;
            let connector = WasmHostConnector::new(channel_name);
            let transport = connector
                .connect()
                .await
                .map_err(|e| Error::Transport(e.to_string()))?;
            let (sender, receiver) = transport.split();
            Ok(Self { sender, receiver })
        }
    }

    #[async_trait::async_trait]
    impl AdapterTransport for WasmHostAdapter {
        async fn send(&mut self, frame: Bytes) -> Result<()> {
            self.sender.send(frame).await.map_err(Into::into)
        }

        async fn recv(&mut self) -> Result<Option<Bytes>> {
            self.receiver.recv().await.map_err(Into::into)
        }

        async fn close(&mut self) -> Result<()> {
            self.sender.close().await.map_err(Into::into)
        }
    }

    pub async fn connect_wasm_host(
        channel_name: Option<&str>,
    ) -> Result<Box<dyn AdapterTransport>> {
        let channel = channel_name.unwrap_or(DEFAULT_WASM_HOST_CHANNEL);
        Ok(Box::new(WasmHostAdapter::connect(channel).await?))
    }
}

/// Parse an address string and return a connected boxed transport.
///
/// Supported formats:
/// - `tcp://host:port`
/// - `ws://host:port` or `wss://host:port`
/// - `unix:///absolute/path`
/// - `wasm-host://channel-name` (WASM only, feature `wasm`)
/// - `wasm-host` (uses default channel "saikuro")
pub async fn connect(address: &str) -> Result<Box<dyn AdapterTransport>> {
    if let Some(_rest) = address.strip_prefix("tcp://") {
        #[cfg(all(feature = "tcp", not(target_arch = "wasm32")))]
        {
            let (host, port_str) = parse_host_port(_rest)?;
            let port: u16 = port_str
                .parse()
                .map_err(|_| Error::Transport(format!("invalid port in address: {address}")))?;
            return tcp_impl::connect_tcp(&host, port).await;
        }
        #[cfg(not(all(feature = "tcp", not(target_arch = "wasm32"))))]
        return Err(Error::Transport(
            "TCP transport is not available (feature 'tcp' disabled or wasm32 target)".into(),
        ));
    }

    if address.starts_with("ws://") || address.starts_with("wss://") {
        #[cfg(feature = "ws")]
        return ws_impl::connect_ws(address).await;
        #[cfg(not(feature = "ws"))]
        return Err(Error::Transport(
            "WebSocket transport is not available (feature 'ws' disabled)".into(),
        ));
    }

    if let Some(_path) = address.strip_prefix("unix://") {
        #[cfg(all(feature = "unix", not(target_arch = "wasm32"), target_family = "unix"))]
        return unix_impl::connect_unix(_path).await;
        #[cfg(not(all(feature = "unix", not(target_arch = "wasm32"), target_family = "unix")))]
        return Err(Error::Transport(
            "Unix socket transport is not available on this platform".into(),
        ));
    }

    if address == "wasm-host" || address.starts_with("wasm-host://") {
        #[cfg(all(feature = "wasm", target_arch = "wasm32"))]
        {
            let channel_name = address
                .strip_prefix("wasm-host://")
                .filter(|s| !s.is_empty());
            return wasm_host_impl::connect_wasm_host(channel_name).await;
        }
        #[cfg(not(all(feature = "wasm", target_arch = "wasm32")))]
        return Err(Error::Transport(
            "WasmHost transport is only available with feature 'wasm' on wasm32 target".into(),
        ));
    }

    Err(Error::Transport(format!(
        "unrecognised address scheme: {address}; expected tcp://, ws://, unix://, or wasm-host://"
    )))
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_host_port(s: &str) -> Result<(String, &str)> {
    // Handle IPv6 like [::1]:7700
    if let Some(bracket_end) = s.find(']') {
        let host = &s[1..bracket_end];
        let rest = &s[bracket_end + 1..];
        let port = rest
            .strip_prefix(':')
            .ok_or_else(|| Error::Transport(format!("missing port in address: {s}")))?;
        return Ok((host.to_owned(), port));
    }
    // IPv4 / hostname
    if let Some(colon) = s.rfind(':') {
        let host = &s[..colon];
        let port = &s[colon + 1..];
        return Ok((host.to_owned(), port));
    }
    Err(Error::Transport(format!("missing port in address: {s}")))
}

// In-memory transport

/// A connected in-memory transport pair.
///
/// Use [`InMemoryTransport::pair`] to create two transports that communicate
/// via in-process channels, with no network I/O.  Useful for unit-testing
/// providers and clients without a live runtime.
///
/// # Example
///
/// ```no_run
/// use saikuro::transport::InMemoryTransport;
/// use saikuro::{Client, Provider};
///
/// let (provider_t, client_t) = InMemoryTransport::pair();
///
/// // Spawn the provider on one side, connect the client on the other.
/// saikuro_exec::spawn(async move {
///     let mut provider = Provider::new("math");
///     provider.register("add", |args: Vec<serde_json::Value>| async move {
///         Ok(serde_json::json!(args[0].as_i64().unwrap_or(0) + args[1].as_i64().unwrap_or(0)))
///     });
///     provider.serve_on(Box::new(provider_t)).await.unwrap();
/// });
/// ```
pub struct InMemoryTransport {
    sender: saikuro_exec::mpsc::Sender<Bytes>,
    receiver: saikuro_exec::mpsc::Receiver<Bytes>,
}

impl InMemoryTransport {
    /// Create a connected pair of in-memory transports.
    ///
    /// Bytes sent on one side are received on the other.
    pub fn pair() -> (Self, Self) {
        let (a_tx, b_rx) = saikuro_exec::mpsc::channel(256);
        let (b_tx, a_rx) = saikuro_exec::mpsc::channel(256);
        (
            Self {
                sender: a_tx,
                receiver: a_rx,
            },
            Self {
                sender: b_tx,
                receiver: b_rx,
            },
        )
    }
}

#[async_trait::async_trait]
impl AdapterTransport for InMemoryTransport {
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        self.sender
            .send(frame)
            .await
            .map_err(|_| Error::Transport("in-memory channel closed".into()))
    }

    async fn recv(&mut self) -> Result<Option<Bytes>> {
        Ok(self.receiver.recv().await)
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}
