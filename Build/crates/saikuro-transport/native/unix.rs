use crate::{impl_native_receiver, impl_native_sender};
use async_trait::async_trait;
use saikuro_net::io::{split, ReadHalf, WriteHalf};
use saikuro_net::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use tracing::debug;

use crate::shared::{
    error::Result,
    traits::{Transport, TransportConnector, TransportListener},
};

/// A Unix domain socket transport connection.
pub struct UnixTransport {
    stream: UnixStream,
    path: PathBuf,
}

impl UnixTransport {
    /// Wrap an already-connected [`UnixStream`].
    pub fn new(stream: UnixStream, path: PathBuf) -> Self {
        Self { stream, path }
    }
}

impl Transport for UnixTransport {
    type Sender = UnixSender;
    type Receiver = UnixReceiver;

    fn split(self) -> (Self::Sender, Self::Receiver) {
        let (read, write) = split(self.stream);
        let path = self.path.clone();
        (
            UnixSender {
                inner: write,
                path: path.clone(),
            },
            UnixReceiver { inner: read, path },
        )
    }

    fn description(&self) -> &str {
        "unix"
    }
}

// Sender / Receiver
pub struct UnixSender {
    inner: WriteHalf<UnixStream>,
    path: PathBuf,
}

impl_native_sender!(UnixSender, path, "unix");

pub struct UnixReceiver {
    inner: ReadHalf<UnixStream>,
    path: PathBuf,
}

impl_native_receiver!(UnixReceiver, path, "unix");

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

/// Accepts incoming Unix domain socket connections.
pub struct UnixTransportListener {
    inner: UnixListener,
    path: PathBuf,
}

impl UnixTransportListener {
    /// Bind a listener on the given socket path.
    ///
    /// If a stale socket file already exists at the path it is removed first.
    pub async fn bind(path: impl AsRef<Path>) -> Result<Self> {
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
