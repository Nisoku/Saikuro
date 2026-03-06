//! Unix domain socket transport (Unix + native only).
//!
//! On the same physical machine a Unix domain socket is faster than TCP
//! because it skips the TCP stack entirely.  It uses the same
//! length-prefixed framing as the TCP transport.
//! This only works when the target OS is a Unix family OS. (yes, not you Windows >:( )

use async_trait::async_trait;
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use std::path::{Path, PathBuf};
use tokio::net::{UnixListener, UnixStream};
use tokio_util::codec::Framed;
use tracing::{debug, trace};

use crate::{
    error::Result,
    framing::LengthPrefixedCodec,
    traits::{
        Transport, TransportConnector, TransportListener, TransportReceiver, TransportSender,
    },
};

// Transport

/// A Unix domain socket transport connection.
pub struct UnixTransport {
    framed: Framed<UnixStream, LengthPrefixedCodec>,
    path: PathBuf,
}

impl UnixTransport {
    /// Wrap an already-connected [`UnixStream`].
    pub fn new(stream: UnixStream, path: PathBuf) -> Self {
        Self {
            framed: Framed::new(stream, LengthPrefixedCodec::new()),
            path,
        }
    }
}

impl Transport for UnixTransport {
    type Sender = UnixSender;
    type Receiver = UnixReceiver;

    fn split(self) -> (Self::Sender, Self::Receiver) {
        let path = self.path.clone();
        let (sink, stream) = self.framed.split();
        (
            UnixSender {
                inner: sink,
                path: path.clone(),
            },
            UnixReceiver {
                inner: stream,
                path,
            },
        )
    }

    fn description(&self) -> &str {
        "unix"
    }
}

// Sender / Receiver

pub struct UnixSender {
    inner: futures::stream::SplitSink<Framed<UnixStream, LengthPrefixedCodec>, Bytes>,
    path: PathBuf,
}

#[async_trait]
impl TransportSender for UnixSender {
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        trace!(path = ?self.path, bytes = frame.len(), "unix send");
        self.inner.send(frame).await
    }

    async fn close(&mut self) -> Result<()> {
        debug!(path = ?self.path, "unix sender closing");
        self.inner.close().await
    }
}

pub struct UnixReceiver {
    inner: futures::stream::SplitStream<Framed<UnixStream, LengthPrefixedCodec>>,
    path: PathBuf,
}

#[async_trait]
impl TransportReceiver for UnixReceiver {
    async fn recv(&mut self) -> Result<Option<Bytes>> {
        match self.inner.next().await {
            Some(Ok(bytes)) => {
                trace!(path = ?self.path, bytes = bytes.len(), "unix recv");
                Ok(Some(bytes))
            }
            Some(Err(e)) => Err(e),
            None => {
                debug!(path = ?self.path, "unix connection closed by peer");
                Ok(None)
            }
        }
    }
}

// Connector

/// Establishes outgoing Unix socket connections.
pub struct UnixConnector {
    path: PathBuf,
}

impl UnixConnector {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_owned(),
        }
    }
}

#[async_trait]
impl TransportConnector for UnixConnector {
    type Output = UnixTransport;

    async fn connect(&self) -> Result<Self::Output> {
        debug!(path = ?self.path, "unix connecting");
        let stream = UnixStream::connect(&self.path).await?;
        Ok(UnixTransport::new(stream, self.path.clone()))
    }
}

// Listener

/// Accepts incoming Unix domain socket connections.
pub struct UnixTransportListener {
    inner: UnixListener,
    path: PathBuf,
}

impl UnixTransportListener {
    /// Bind a listener on the given socket path.
    ///
    /// If a stale socket file already exists at the path it is removed first.
    pub fn bind(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_owned();
        // Remove any stale socket from a previous run.
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        let inner = UnixListener::bind(&path)?;
        debug!(?path, "unix listener bound");
        Ok(Self { inner, path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for UnixTransportListener {
    fn drop(&mut self) {
        // Best-effort cleanup of the socket file.
        let _ = std::fs::remove_file(&self.path);
    }
}

#[async_trait]
impl TransportListener for UnixTransportListener {
    type Output = UnixTransport;

    async fn accept(&mut self) -> Result<Option<Self::Output>> {
        match self.inner.accept().await {
            Ok((stream, _addr)) => {
                debug!(path = ?self.path, "unix accepted connection");
                Ok(Some(UnixTransport::new(stream, self.path.clone())))
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn close(&mut self) -> Result<()> {
        debug!(path = ?self.path, "unix listener closing");
        Ok(())
    }
}
