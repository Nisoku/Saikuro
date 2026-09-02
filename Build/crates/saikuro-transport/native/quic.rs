//! QUIC (RFC 9000) transport

use crate::{impl_native_receiver, impl_native_sender};
use async_trait::async_trait;
#[cfg(not(target_has_atomic = "ptr"))]
use portable_atomic_util::Arc;
use s2n_quic::stream::{ReceiveStream, SendStream};
use s2n_quic::{Client, Server};
#[cfg(target_has_atomic = "ptr")]
use saikuro_core::Arc;
use saikuro_event::{LogLevel, LogRecord};
use std::net::SocketAddr;

use crate::shared::{
    error::{Result, TransportError},
    traits::{Transport, TransportConnector, TransportListener},
};

/// Wrap a provider error in an [`io::Error`] so it conforms to [`TransportError`].
fn io_err(e: impl core::fmt::Display) -> TransportError {
    TransportError::Io(std::io::Error::other(alloc::format!("{e}")))
}

/// A QUIC connection carrying one bidirectional stream.
pub struct QuicTransport {
    receiver: ReceiveStream,
    sender: SendStream,
    peer_addr: SocketAddr,
    log: Arc<dyn saikuro_event::LogSink>,
}

impl Transport for QuicTransport {
    type Sender = QuicSender;
    type Receiver = QuicReceiver;

    fn split(self) -> (Self::Sender, Self::Receiver) {
        let peer = self.peer_addr;
        let log = self.log;
        (
            QuicSender {
                inner: self.sender,
                peer_addr: peer,
                log: log.clone(),
            },
            QuicReceiver {
                inner: self.receiver,
                peer_addr: peer,
                log,
            },
        )
    }

    fn description(&self) -> &str {
        "quic"
    }
}

// Sender / Receiver
pub struct QuicSender {
    inner: SendStream,
    peer_addr: SocketAddr,
    log: Arc<dyn saikuro_event::LogSink>,
}

impl_native_sender!(QuicSender, peer_addr, "quic");

pub struct QuicReceiver {
    inner: ReceiveStream,
    peer_addr: SocketAddr,
    log: Arc<dyn saikuro_event::LogSink>,
}

impl_native_receiver!(QuicReceiver, peer_addr, "quic");

/// Establishes outgoing QUIC connections.
pub struct QuicConnector {
    client: Client,
    addr: SocketAddr,
    log: Arc<dyn saikuro_event::LogSink>,
}

impl QuicConnector {
    /// Build a client endpoint that trusts `server_cert_pem` and connect to
    /// `addr`.  The PEM is the server's own self-signed certificate or the CA
    /// chain that signed it.
    pub async fn new(
        addr: SocketAddr,
        server_cert_pem: &[u8],
        log: Arc<dyn saikuro_event::LogSink>,
    ) -> Result<Self> {
        let client = Client::builder()
            .with_tls(server_cert_pem)
            .map_err(io_err)?
            .with_io("0.0.0.0:0")
            .map_err(io_err)?
            .start()
            .map_err(io_err)?;
        Ok(Self { client, addr, log })
    }
}

#[async_trait]
impl TransportConnector for QuicConnector {
    type Output = QuicTransport;

    async fn connect(&self) -> Result<Self::Output> {
        let mut record =
            LogRecord::now(LogLevel::Debug, "saikuro.transport.quic", "quic connecting");
        record.set_context("addr", alloc::format!("{}", self.addr));
        self.log.emit(&record).await;
        let mut connection = self.client.connect(self.addr.into()).await.map_err(|e| {
            TransportError::ConnectionRefused(format!("quic connect to {} failed: {e}", self.addr))
        })?;
        let stream = connection.open_bidirectional_stream().await.map_err(|e| {
            TransportError::ConnectionLost(format!("quic open stream to {} failed: {e}", self.addr))
        })?;
        let (receiver, sender) = stream.split();
        Ok(QuicTransport {
            receiver,
            sender,
            peer_addr: self.addr,
            log: self.log.clone(),
        })
    }
}

/// Accepts inbound QUIC connections.
pub struct QuicTransportListener {
    server: Server,
    local_addr: SocketAddr,
    log: Arc<dyn saikuro_event::LogSink>,
}

impl QuicTransportListener {
    /// Bind a QUIC server on `addr` presenting `cert_pem`/`key_pem`.
    pub async fn bind(
        addr: SocketAddr,
        cert_pem: &[u8],
        key_pem: &[u8],
        log: Arc<dyn saikuro_event::LogSink>,
    ) -> Result<Self> {
        let server = Server::builder()
            .with_tls((cert_pem, key_pem))
            .map_err(io_err)?
            .with_io(addr)
            .map_err(io_err)?
            .start()
            .map_err(io_err)?;
        let local_addr = server.local_addr().map_err(io_err)?;
        let mut record = LogRecord::now(
            LogLevel::Debug,
            "saikuro.transport.quic",
            "quic listener bound",
        );
        record.set_context("local_addr", alloc::format!("{}", local_addr));
        log.emit(&record).await;
        Ok(Self {
            server,
            local_addr,
            log,
        })
    }

    /// Return the address this listener is bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

#[async_trait]
impl TransportListener for QuicTransportListener {
    type Output = QuicTransport;

    async fn accept(&mut self) -> Result<Option<Self::Output>> {
        while let Some(connection) = self.server.accept().await {
            let peer_addr = match connection.remote_addr() {
                Ok(addr) => addr,
                Err(e) => {
                    // The connection is unusable; skip it and accept the next one.
                    let mut record = LogRecord::now(
                        LogLevel::Warn,
                        "saikuro.transport.quic",
                        "quic connection has no remote address",
                    );
                    record.set_context("error", alloc::format!("{}", e));
                    self.log.emit(&record).await;
                    continue;
                }
            };
            let (_handle, mut acceptor) = connection.split();
            match acceptor.accept_bidirectional_stream().await {
                Ok(Some(stream)) => {
                    let mut record = LogRecord::now(
                        LogLevel::Debug,
                        "saikuro.transport.quic",
                        "quic accepted connection",
                    );
                    record.set_context("peer", alloc::format!("{}", peer_addr));
                    self.log.emit(&record).await;
                    let (receiver, sender) = stream.split();
                    return Ok(Some(QuicTransport {
                        receiver,
                        sender,
                        peer_addr,
                        log: self.log.clone(),
                    }));
                }
                Ok(None) => {
                    // Peer closed without opening a stream; accept the next connection.
                    let mut record = LogRecord::now(
                        LogLevel::Debug,
                        "saikuro.transport.quic",
                        "quic connection closed before opening a stream",
                    );
                    record.set_context("peer", alloc::format!("{}", peer_addr));
                    self.log.emit(&record).await;
                }
                Err(e) => {
                    return Err(TransportError::ConnectionLost(format!(
                        "quic accept stream from {peer_addr} failed: {e}"
                    )))
                }
            }
        }
        Ok(None)
    }

    async fn close(&mut self) -> Result<()> {
        let mut record = LogRecord::now(
            LogLevel::Debug,
            "saikuro.transport.quic",
            "quic listener closing",
        );
        record.set_context("local_addr", alloc::format!("{}", self.local_addr));
        self.log.emit(&record).await;
        // The s2n-quic Server acceptor stops when the endpoint is dropped.
        Ok(())
    }
}
