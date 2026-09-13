use crate::{impl_native_receiver, impl_native_sender};
use async_trait::async_trait;
#[cfg(not(target_has_atomic = "ptr"))]
use portable_atomic_util::Arc;
#[cfg(target_has_atomic = "ptr")]
use saikuro_core::Arc;
use saikuro_event::{LogLevel, LogRecord};
use saikuro_net::io::{split, ReadHalf, WriteHalf};
use saikuro_net::net::{TcpListener, TcpStream};
use std::net::SocketAddr;

use crate::shared::{
    error::{Result, TransportError},
    traits::{Transport, TransportConnector, TransportListener},
};

/// A TCP transport connection.
pub struct TcpTransport {
    stream: TcpStream,
    peer_addr: SocketAddr,
    log: Arc<dyn saikuro_event::LogSink>,
    max_frame_size: usize,
}

impl TcpTransport {
    /// Wrap an already-connected [`TcpStream`].
    pub fn new(stream: TcpStream, log: Arc<dyn saikuro_event::LogSink>) -> Result<Self> {
        let peer_addr = stream.peer_addr()?;
        // Disable Nagle's algorithm: Saikuro sends complete frames and latency
        // matters more than segment coalescing.
        stream.set_nodelay(true)?;
        Ok(Self {
            stream,
            peer_addr,
            log,
            max_frame_size: crate::shared::framing::DEFAULT_MAX_FRAME_LEN,
        })
    }

    /// Raise the inbound frame limit above the transport default.
    pub fn max_frame_size(mut self, limit: usize) -> Result<Self> {
        if limit > crate::MAX_FRAME_SIZE {
            return Err(TransportError::MessageTooLarge {
                size: limit,
                limit: crate::MAX_FRAME_SIZE,
            });
        }
        self.max_frame_size = limit;
        Ok(self)
    }
}

impl Transport for TcpTransport {
    type Sender = TcpSender;
    type Receiver = TcpReceiver;

    fn split(self) -> (Self::Sender, Self::Receiver) {
        let (read, write) = split(self.stream);
        let peer = self.peer_addr;
        let log = self.log;
        let max_frame_size = self.max_frame_size;
        (
            TcpSender {
                inner: write,
                peer_addr: peer,
                log: log.clone(),
            },
            TcpReceiver {
                inner: read,
                peer_addr: peer,
                log,
                max_frame_size,
            },
        )
    }

    fn description(&self) -> &str {
        "tcp"
    }
}

// Sender / Receiver
pub struct TcpSender {
    inner: WriteHalf<TcpStream>,
    peer_addr: SocketAddr,
    log: Arc<dyn saikuro_event::LogSink>,
}

impl_native_sender!(TcpSender, peer_addr, "tcp");

pub struct TcpReceiver {
    inner: ReadHalf<TcpStream>,
    peer_addr: SocketAddr,
    log: Arc<dyn saikuro_event::LogSink>,
    max_frame_size: usize,
}

impl_native_receiver!(TcpReceiver, peer_addr, "tcp");

/// Establishes outgoing TCP connections.
pub struct TcpConnector {
    addr: SocketAddr,
    log: Arc<dyn saikuro_event::LogSink>,
}

impl TcpConnector {
    pub fn new(addr: SocketAddr, log: Arc<dyn saikuro_event::LogSink>) -> Self {
        Self { addr, log }
    }
}

#[async_trait]
impl TransportConnector for TcpConnector {
    type Output = TcpTransport;

    async fn connect(&self) -> Result<Self::Output> {
        let mut record = LogRecord::now(LogLevel::Debug, "saikuro.transport.tcp", "tcp connecting");
        record.set_context("addr", alloc::format!("{}", self.addr));
        self.log.emit(&record).await;
        let stream = TcpStream::connect(self.addr).await?;
        TcpTransport::new(stream, self.log.clone())
    }
}

/// Accepts incoming TCP connections.
pub struct TcpTransportListener {
    inner: TcpListener,
    local_addr: SocketAddr,
    log: Arc<dyn saikuro_event::LogSink>,
}

impl TcpTransportListener {
    /// Bind a listener on the given address.
    pub async fn bind(addr: SocketAddr, log: Arc<dyn saikuro_event::LogSink>) -> Result<Self> {
        let inner = TcpListener::bind(addr).await?;
        let local_addr = inner.local_addr()?;
        let mut record = LogRecord::now(
            LogLevel::Debug,
            "saikuro.transport.tcp",
            "tcp listener bound",
        );
        record.set_context("local_addr", alloc::format!("{}", local_addr));
        log.emit(&record).await;
        Ok(Self {
            inner,
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
impl TransportListener for TcpTransportListener {
    type Output = TcpTransport;

    async fn accept(&mut self) -> Result<Option<Self::Output>> {
        match self.inner.accept().await {
            Ok((stream, peer)) => {
                let mut record = LogRecord::now(
                    LogLevel::Debug,
                    "saikuro.transport.tcp",
                    "tcp accepted connection",
                );
                record.set_context("peer", alloc::format!("{}", peer));
                self.log.emit(&record).await;
                Ok(Some(TcpTransport::new(stream, self.log.clone())?))
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn close(&mut self) -> Result<()> {
        let mut record = LogRecord::now(
            LogLevel::Debug,
            "saikuro.transport.tcp",
            "tcp listener closing",
        );
        record.set_context("local_addr", alloc::format!("{}", self.local_addr));
        self.log.emit(&record).await;
        // TcpListener closes on drop.
        Ok(())
    }
}
