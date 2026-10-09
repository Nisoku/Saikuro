//! Unified transport handle for client/provider.
//!
//! Provides [`AdapterTransport`] which is a single
//! trait-object-compatible interface that bundles send + receive + close
//!
//! The [`connect`] function constructs a connected [`AdapterTransport`] from a
//! string address, dispatching through [`TransportSelector`] to the best
//! available backend.

use bytes::Bytes;
use saikuro_event::{LogSink, NullSink};

use super::error::{Result, TransportError};
use super::memory::{MemoryReceiver, MemorySender, MemoryTransport};
use super::selector::{TransportKind, TransportSelector};
#[cfg(any(
    feature = "tcp",
    all(feature = "quic", feature = "native", not(target_arch = "wasm32"))
))]
use super::traits::TransportConnector;
#[cfg(any(
    all(feature = "wasm-host", target_arch = "wasm32"),
    all(feature = "wasi-tcp", feature = "no_std", not(feature = "native"))
))]
use super::traits::{LocalTransport, LocalTransportConnector};
use super::traits::{Transport, TransportReceiver, TransportSender};

use alloc::boxed::Box;
#[cfg(all(feature = "wasm-host", target_arch = "wasm32"))]
use alloc::string::String;
#[cfg(not(target_has_atomic = "ptr"))]
use portable_atomic_util::Arc;
#[cfg(target_has_atomic = "ptr")]
use saikuro_core::Arc;

/// A trait-object-compatible transport providing both send and receive.
#[cfg(feature = "native")]
#[async_trait::async_trait]
pub trait AdapterTransport: Send + 'static {
    /// Send a single binary frame to the remote peer.
    async fn send(&mut self, frame: Bytes) -> Result<()>;

    /// Receive the next binary frame from the remote peer.
    ///
    /// Returns `Ok(None)` when the peer has closed the connection.
    async fn recv(&mut self) -> Result<Option<Bytes>>;

    /// Close the connection, flushing any pending data where applicable.
    async fn close(&mut self) -> Result<()>;
}

/// A trait-object-compatible transport providing both send and receive.
#[cfg(not(feature = "native"))]
#[async_trait::async_trait(?Send)]
pub trait AdapterTransport: 'static {
    /// Send a single binary frame to the remote peer.
    async fn send(&mut self, frame: Bytes) -> Result<()>;

    /// Receive the next binary frame from the remote peer.
    ///
    /// Returns `Ok(None)` when the peer has closed the connection.
    async fn recv(&mut self) -> Result<Option<Bytes>>;

    /// Close the connection, flushing any pending data where applicable.
    async fn close(&mut self) -> Result<()>;
}

passive_engine_items! {
    struct CombinedAdapter<S, R> {
        sender: S,
        receiver: R,
    }
}

passive_engine_items! { for native;
    #[async_trait::async_trait]
    impl<S, R> AdapterTransport for CombinedAdapter<S, R>
    where
        S: TransportSender,
        R: TransportReceiver,
    {
        async fn send(&mut self, frame: Bytes) -> Result<()> {
            self.sender.send(frame).await
        }

        async fn recv(&mut self) -> Result<Option<Bytes>> {
            self.receiver.recv().await
        }

        async fn close(&mut self) -> Result<()> {
            self.sender.close().await
        }
    }
}

passive_engine_items! { for non_native;
    #[async_trait::async_trait(?Send)]
    impl<S, R> AdapterTransport for CombinedAdapter<S, R>
    where
        S: TransportSender,
        R: TransportReceiver,
    {
        async fn send(&mut self, frame: Bytes) -> Result<()> {
            self.sender.send(frame).await
        }

        async fn recv(&mut self) -> Result<Option<Bytes>> {
            self.receiver.recv().await
        }

        async fn close(&mut self) -> Result<()> {
            self.sender.close().await
        }
    }
}

passive_engine_items! {
    /// Combine a sender half and a receiver half into a boxed [`AdapterTransport`].
    pub fn from_halves(
        sender: impl TransportSender + 'static,
        receiver: impl TransportReceiver + 'static,
    ) -> Box<dyn AdapterTransport> {
        Box::new(CombinedAdapter { sender, receiver })
    }
}

/// A connected in-memory [`AdapterTransport`].
///
/// Construct via [`MemoryAdapterTransport::pair`] or
/// [`MemoryAdapterTransport::from_transport`]. Bytes sent on one side are
/// received on the other.
pub struct MemoryAdapterTransport {
    sender: MemorySender,
    receiver: MemoryReceiver,
    /// Keeps the remote side's halves alive for the duration of this
    /// transport. When the caller drops this transport the peer channel
    /// closes naturally.
    _peer: Option<(MemorySender, MemoryReceiver)>,
}

impl MemoryAdapterTransport {
    /// Create a connected pair of in-memory transports.
    ///
    /// Bytes sent on side A are received on side B, and vice-versa.
    pub fn pair() -> (Self, Self) {
        let log: Arc<dyn LogSink> = Arc::from(Box::new(NullSink) as Box<dyn LogSink>);
        let (a, b) = MemoryTransport::connected_pair(log);
        let (a_tx, a_rx) = a.split();
        let (b_tx, b_rx) = b.split();
        (
            Self {
                sender: a_tx,
                receiver: a_rx,
                _peer: None,
            },
            Self {
                sender: b_tx,
                receiver: b_rx,
                _peer: None,
            },
        )
    }

    /// Wrap a [`MemoryTransport`] into a single [`AdapterTransport`] handle.
    pub fn from_transport(transport: MemoryTransport) -> Self {
        let (sender, receiver) = transport.split();
        Self {
            sender,
            receiver,
            _peer: None,
        }
    }

    /// Wrap a [`MemoryTransport`] pair, returning one side as an
    /// [`AdapterTransport`] and storing the other side as a keepalive.
    pub fn from_pair(a: MemoryTransport, b: MemoryTransport) -> Self {
        let (a_tx, a_rx) = a.split();
        let (b_tx, b_rx) = b.split();
        Self {
            sender: a_tx,
            receiver: a_rx,
            _peer: Some((b_tx, b_rx)),
        }
    }
}

#[cfg(feature = "native")]
#[async_trait::async_trait]
impl AdapterTransport for MemoryAdapterTransport {
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        self.sender.send(frame).await
    }

    async fn recv(&mut self) -> Result<Option<Bytes>> {
        self.receiver.recv().await
    }

    async fn close(&mut self) -> Result<()> {
        self.sender.close().await
    }
}

#[cfg(not(feature = "native"))]
#[async_trait::async_trait(?Send)]
impl AdapterTransport for MemoryAdapterTransport {
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        self.sender.send(frame).await
    }

    async fn recv(&mut self) -> Result<Option<Bytes>> {
        self.receiver.recv().await
    }

    async fn close(&mut self) -> Result<()> {
        self.sender.close().await
    }
}

/// Options for establishing an adapter connection.
#[derive(Debug, Clone, Copy, Default)]
pub struct ConnectOptions<'a> {
    /// PEM-encoded self-signed server certificate or CA chain trusted for QUIC.
    /// The server certificate must be valid for the destination IP address.
    /// `None` uses the default TLS trust configuration. Ignored by other transports.
    pub quic_server_cert_pem: Option<&'a [u8]>,
}

/// Parse an address string and return a connected boxed transport.
///
/// The best transport backend is selected automatically based on the address
/// format and the features enabled at compile time.
///
/// # Supported address formats
///
/// - `memory` - in-process MPSC
/// - `tcp://host:port` - TCP
/// - `unix:///path/to/socket` - Unix domain socket
/// - `ws://host:port/path` or `wss://...` - WebSocket
/// - `quic://host:port` - QUIC (native targets with `quic` and `native` features)
///
/// Uses default TLS trust. Use [`connect_with_options`] to configure custom QUIC trust.
pub async fn connect(address: &str) -> Result<Box<dyn AdapterTransport>> {
    connect_with_options(address, ConnectOptions::default()).await
}

/// Connect using the address formats supported by [`connect`] and explicit options.
#[allow(unused_variables)]
pub async fn connect_with_options(
    address: &str,
    options: ConnectOptions<'_>,
) -> Result<Box<dyn AdapterTransport>> {
    let log: Arc<dyn LogSink> = Arc::from(Box::new(NullSink) as Box<dyn LogSink>);
    let (kind, addr) = TransportSelector::select(Some(address), None);

    #[allow(unreachable_patterns)]
    match kind {
        TransportKind::Memory => {
            let pair =
                MemoryTransport::connected_pair(Arc::from(Box::new(NullSink) as Box<dyn LogSink>));
            let transport = MemoryAdapterTransport::from_pair(pair.0, pair.1);
            Ok(Box::new(transport))
        }

        #[cfg(feature = "tcp")]
        TransportKind::Tcp => {
            let addr_str = addr.as_deref().ok_or(TransportError::ConnectionRefused(
                "tcp requires a host:port address".into(),
            ))?;
            let sock_addr: std::net::SocketAddr =
                addr_str.parse().map_err(|e: std::net::AddrParseError| {
                    TransportError::ConnectionRefused(e.to_string())
                })?;
            let connector = crate::native::tcp::TcpConnector::new(sock_addr, log);
            let transport = connector.connect().await?;
            let (sender, receiver) = transport.split();
            Ok(Box::new(CombinedAdapter { sender, receiver }))
        }

        #[cfg(all(feature = "quic", feature = "native", not(target_arch = "wasm32")))]
        TransportKind::Quic => {
            let addr_str = addr.as_deref().ok_or(TransportError::ConnectionRefused(
                "quic requires a host:port address".into(),
            ))?;
            let sock_addr: std::net::SocketAddr =
                addr_str.parse().map_err(|e: std::net::AddrParseError| {
                    TransportError::ConnectionRefused(e.to_string())
                })?;
            let connector = match options.quic_server_cert_pem {
                Some(cert) => crate::native::quic::QuicConnector::new(sock_addr, cert, log).await?,
                None => crate::native::quic::QuicConnector::new_addr(sock_addr, log).await?,
            };
            let transport = connector.connect().await?;
            let (sender, receiver) = transport.split();
            Ok(Box::new(CombinedAdapter { sender, receiver }))
        }

        #[cfg(all(feature = "wasi-tcp", feature = "no_std", not(feature = "native")))]
        TransportKind::Tcp => {
            let addr_str = addr.as_deref().ok_or(TransportError::ConnectionRefused(
                "tcp requires a host:port address".into(),
            ))?;
            let connector = crate::wasi::tcp::WasiTcpConnector::new(addr_str);
            let transport = LocalTransportConnector::connect(&connector).await?;
            let (sender, receiver) = transport.split();
            Ok(Box::new(CombinedAdapter { sender, receiver }))
        }

        #[cfg(all(feature = "unix", not(target_arch = "wasm32"), target_family = "unix"))]
        TransportKind::Unix => {
            let path = addr.as_deref().ok_or(TransportError::ConnectionRefused(
                "unix socket requires a path".into(),
            ))?;
            let connector = crate::native::unix::UnixConnector::new(path, log);
            let transport = connector.connect().await?;
            let (sender, receiver) = transport.split();
            Ok(Box::new(CombinedAdapter { sender, receiver }))
        }

        #[cfg(all(feature = "ws", feature = "native"))]
        TransportKind::WebSocket => {
            let url = addr.as_deref().ok_or(TransportError::ConnectionRefused(
                "websocket requires a ws:// or wss:// URL".into(),
            ))?;
            let transport = crate::native::websocket::WebSocketTransport::connect(url, log).await?;
            let (sender, receiver) = transport.split();
            Ok(Box::new(CombinedAdapter { sender, receiver }))
        }

        #[cfg(all(
            feature = "ws",
            feature = "wasm",
            not(feature = "native"),
            not(feature = "no_std")
        ))]
        TransportKind::WebSocket => {
            let url = addr.as_deref().ok_or(TransportError::ConnectionRefused(
                "websocket requires a ws:// or wss:// URL".into(),
            ))?;
            let transport = crate::wasm::WebSocketTransport::connect(url).await?;
            let (sender, receiver) = transport.split();
            Ok(Box::new(CombinedAdapter { sender, receiver }))
        }

        #[cfg(all(feature = "ws", feature = "no_std", not(feature = "native")))]
        TransportKind::WebSocket => {
            let url = addr.as_deref().ok_or(TransportError::ConnectionRefused(
                "websocket requires a ws:// or wss:// URL".into(),
            ))?;
            let transport = crate::wasi::websocket::WebSocketTransport::connect(url).await?;
            let (sender, receiver) = transport.split();
            Ok(Box::new(CombinedAdapter { sender, receiver }))
        }

        #[cfg(all(feature = "wasm-host", target_arch = "wasm32"))]
        TransportKind::WasmHost => {
            let address = addr.as_deref().ok_or(TransportError::ConnectionRefused(
                "wasm-host requires a channel name".into(),
            ))?;
            let channel = String::from(address.strip_prefix("wasm-host://").unwrap_or(address));
            let connector =
                crate::WasmHostConnector::<crate::wasm::BroadcastChannelPipe>::new(channel);
            let transport = LocalTransportConnector::connect(&connector).await?;
            let (sender, receiver) = transport.split();
            Ok(Box::new(CombinedAdapter { sender, receiver }))
        }

        _ => Err(TransportError::NotSupported),
    }
}
