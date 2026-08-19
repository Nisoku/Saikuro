use crate::{impl_native_receiver, impl_native_sender};
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc;
use async_trait::async_trait;
#[cfg(not(target_has_atomic = "ptr"))]
use portable_atomic_util::Arc;
use saikuro_event::{LogLevel, LogRecord};
use saikuro_net::io::{split, ReadHalf, WriteHalf};
use saikuro_net::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use crate::shared::{
    error::Result,
    traits::{Transport, TransportConnector, TransportListener},
};

/// A Unix domain socket transport connection.
pub struct UnixTransport {
    stream: UnixStream,
    path: PathBuf,
    log: Arc<dyn saikuro_event::LogSink>,
}

impl UnixTransport {
    /// Wrap an already-connected [`UnixStream`].
    pub fn new(stream: UnixStream, path: PathBuf, log: Arc<dyn saikuro_event::LogSink>) -> Self {
        Self { stream, path, log }
    }
}

impl Transport for UnixTransport {
    type Sender = UnixSender;
    type Receiver = UnixReceiver;

    fn split(self) -> (Self::Sender, Self::Receiver) {
        let (read, write) = split(self.stream);
        let path = self.path.clone();
        let log = self.log;
        (
            UnixSender {
                inner: write,
                path: path.clone(),
                log: log.clone(),
            },
            UnixReceiver {
                inner: read,
                path,
                log,
            },
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
    log: Arc<dyn saikuro_event::LogSink>,
}

impl_native_sender!(UnixSender, path, "unix");

pub struct UnixReceiver {
    inner: ReadHalf<UnixStream>,
    path: PathBuf,
    log: Arc<dyn saikuro_event::LogSink>,
}

impl_native_receiver!(UnixReceiver, path, "unix");

/// Establishes outgoing Unix socket connections.
pub struct UnixConnector {
    path: PathBuf,
    log: Arc<dyn saikuro_event::LogSink>,
}

impl UnixConnector {
    pub fn new(path: impl AsRef<Path>, log: Arc<dyn saikuro_event::LogSink>) -> Self {
        Self {
            path: path.as_ref().to_owned(),
            log,
        }
    }
}

#[async_trait]
impl TransportConnector for UnixConnector {
    type Output = UnixTransport;

    async fn connect(&self) -> Result<Self::Output> {
        let mut record =
            LogRecord::now(LogLevel::Debug, "saikuro.transport.unix", "unix connecting");
        record.set_context("path", alloc::format!("{:?}", self.path));
        self.log.emit(&record).await;
        let stream = UnixStream::connect(&self.path).await?;
        Ok(UnixTransport::new(
            stream,
            self.path.clone(),
            self.log.clone(),
        ))
    }
}

/// Accepts incoming Unix domain socket connections.
pub struct UnixTransportListener {
    inner: UnixListener,
    path: PathBuf,
    log: Arc<dyn saikuro_event::LogSink>,
}

impl UnixTransportListener {
    /// Bind a listener on the given socket path.
    ///
    /// If a stale socket file already exists at the path it is removed first.
    pub async fn bind(
        path: impl AsRef<Path>,
        log: Arc<dyn saikuro_event::LogSink>,
    ) -> Result<Self> {
        let path = path.as_ref().to_owned();
        // Remove any stale socket from a previous run.
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        let inner = UnixListener::bind(&path)?;
        let mut record = LogRecord::now(
            LogLevel::Debug,
            "saikuro.transport.unix",
            "unix listener bound",
        );
        record.set_context("path", alloc::format!("{:?}", path));
        log.emit(&record).await;
        Ok(Self { inner, path, log })
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
                let mut record = LogRecord::now(
                    LogLevel::Debug,
                    "saikuro.transport.unix",
                    "unix accepted connection",
                );
                record.set_context("path", alloc::format!("{:?}", self.path));
                self.log.emit(&record).await;
                Ok(Some(UnixTransport::new(
                    stream,
                    self.path.clone(),
                    self.log.clone(),
                )))
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn close(&mut self) -> Result<()> {
        let mut record = LogRecord::now(
            LogLevel::Debug,
            "saikuro.transport.unix",
            "unix listener closing",
        );
        record.set_context("path", alloc::format!("{:?}", self.path));
        self.log.emit(&record).await;
        Ok(())
    }
}
