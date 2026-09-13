//! Saikuro provider: register Rust functions and serve them to the runtime.

mod dispatch;
mod handler;
mod send;

#[cfg(any(feature = "wasm", feature = "embedded", feature = "no_std"))]
mod base;
#[cfg(feature = "native")]
mod native;

pub use handler::{HandlerArgs, RegisterOptions};

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::string::{String, ToString};

use bytes::Bytes;
use saikuro_core::envelope::{Envelope, InvocationType, ResponseEnvelope};
use saikuro_core::schema::Schema;
use saikuro_event::{json_to_core, LogLevel, LogRecord, LogSink, Result, SaikuroError};
use saikuro_schema::builder::{build_schema, NamespaceSchema};
use saikuro_transport::{connect, AdapterTransport};

#[cfg(not(feature = "std"))]
use alloc::collections::BTreeMap as HashMap;
#[cfg(feature = "std")]
use std::collections::HashMap;

use crate::shared::types::Arc;

/// A Saikuro provider that exposes Rust functions as invokable functions.
///
/// One `Provider` maps to one namespace.  It connects to the runtime, announces
/// its schema, then enters a serve loop dispatching inbound invocations.
pub struct Provider {
    namespace: String,
    handlers: HashMap<String, handler::HandlerEntry>,
    log: Arc<dyn LogSink>,
}

impl Provider {
    /// Create a new provider for the given namespace.
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            handlers: HashMap::new(),
            log: Arc::from(Box::new(saikuro_event::NullSink) as Box<dyn LogSink>),
        }
    }

    /// Set the log sink for this provider.
    pub fn with_log_sink(mut self, log: Arc<dyn LogSink>) -> Self {
        self.log = log;
        self
    }

    /// Returns the namespace this provider operates under.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    // Schema

    /// Build the schema announcement for this provider.
    fn build_schema(&self) -> Result<Schema> {
        let mut ns_schema = NamespaceSchema::new();
        for (name, entry) in &self.handlers {
            if let Some(schema) = &entry.schema {
                ns_schema.insert(name.clone(), schema.clone());
            }
        }

        let mut all_ns = HashMap::new();
        all_ns.insert(self.namespace.clone(), ns_schema);

        build_schema(&all_ns)
    }

    // Serving

    /// Connect to the runtime at `address` and serve until the connection
    /// closes or an unrecoverable error occurs.
    pub async fn serve(self, address: impl AsRef<str>) -> Result<()> {
        let addr = address.as_ref();
        {
            let mut record = LogRecord::now(
                LogLevel::Info,
                "saikuro.rust.provider",
                "connecting to runtime",
            );
            record.set_context("namespace", self.namespace.clone());
            record.set_context("address", addr.to_owned());
            self.log.emit(&record).await;
        }
        let transport = connect(addr).await?;
        self.serve_on(transport).await
    }

    /// Serve on an already-connected transport.
    pub async fn serve_on(self, mut transport: Box<dyn AdapterTransport>) -> Result<()> {
        self.announce(&mut *transport).await?;

        {
            let mut record = LogRecord::now(
                LogLevel::Info,
                "saikuro.rust.provider",
                "provider ready, entering serve loop",
            );
            record.set_context("namespace", self.namespace.clone());
            self.log.emit(&record).await;
        }

        let handlers = Arc::new(self.handlers);
        let namespace = Arc::new(self.namespace);
        let log = self.log.clone();

        loop {
            let frame = match transport.recv().await {
                Ok(Some(f)) => f,
                Ok(None) => {
                    let mut record = LogRecord::now(
                        LogLevel::Info,
                        "saikuro.rust.provider",
                        "runtime closed connection",
                    );
                    record.set_context("namespace", namespace.to_string());
                    log.emit(&record).await;
                    break;
                }
                Err(e) => {
                    let mut record =
                        LogRecord::now(LogLevel::Error, "saikuro.rust.provider", "recv error");
                    record.set_context("namespace", namespace.to_string());
                    record.set_context("error", alloc::format!("{e}"));
                    log.emit(&record).await;
                    break;
                }
            };

            let envelope = match Envelope::from_msgpack(&frame) {
                Ok(e) => e,
                Err(e) => {
                    let mut record = LogRecord::now(
                        LogLevel::Warn,
                        "saikuro.rust.provider",
                        "malformed inbound envelope, skipping",
                    );
                    record.set_context("error", alloc::format!("{e}"));
                    log.emit(&record).await;
                    continue;
                }
            };

            match envelope.invocation_type {
                InvocationType::Call => {
                    dispatch::dispatch_call(envelope, &handlers, &mut *transport, &*log).await;
                }
                InvocationType::Cast => {
                    dispatch::dispatch_cast(envelope, &handlers, &*log).await;
                }
                InvocationType::Batch => {
                    dispatch::dispatch_batch(envelope, &handlers, &mut *transport, &*log).await;
                }
                other => {
                    let mut record = LogRecord::now(
                        LogLevel::Warn,
                        "saikuro.rust.provider",
                        "unsupported invocation type",
                    );
                    record.set_context("invocation_type", alloc::format!("{other}"));
                    record.set_context("target", envelope.target.clone());
                    log.emit(&record).await;
                }
            }
        }

        let _ = transport.close().await;
        Ok(())
    }

    // Announce

    async fn announce(&self, transport: &mut dyn AdapterTransport) -> Result<()> {
        let schema = match self.build_schema() {
            Ok(schema) => schema,
            Err(e) => {
                let mut record = LogRecord::now(
                    LogLevel::Warn,
                    "saikuro.rust.provider",
                    "failed to build schema announcement",
                );
                record.set_context("error", alloc::format!("{e}"));
                self.log.emit(&record).await;
                return Err(e);
            }
        };
        let schema_value = match serde_json::to_value(&schema) {
            Ok(v) => json_to_core(v),
            Err(e) => {
                let mut record = LogRecord::now(
                    LogLevel::Warn,
                    "saikuro.rust.provider",
                    "failed to serialize schema for announcement",
                );
                record.set_context("error", alloc::format!("{e}"));
                self.log.emit(&record).await;
                return Err(SaikuroError::Serialization(e.to_string()));
            }
        };

        let announce_env = Envelope::announce(schema_value)?;
        let frame = match announce_env.to_msgpack() {
            Ok(b) => Bytes::from(b),
            Err(e) => {
                let mut record = LogRecord::now(
                    LogLevel::Warn,
                    "saikuro.rust.provider",
                    "failed to encode announce envelope",
                );
                record.set_context("error", alloc::format!("{e}"));
                self.log.emit(&record).await;
                return Err(SaikuroError::Serialization(e.to_string()));
            }
        };

        if let Err(e) = transport.send(frame).await {
            let mut record = LogRecord::now(
                LogLevel::Warn,
                "saikuro.rust.provider",
                "failed to send schema announce",
            );
            record.set_context("error", alloc::format!("{e}"));
            self.log.emit(&record).await;
            return Err(SaikuroError::SendFailed(e.to_string()));
        }

        match saikuro_exec::timeout(core::time::Duration::from_millis(500), transport.recv()).await
        {
            Ok(Ok(Some(ack_frame))) => match ResponseEnvelope::from_msgpack(&ack_frame) {
                Ok(ack) if ack.ok => {
                    let mut record = LogRecord::now(
                        LogLevel::Debug,
                        "saikuro.rust.provider",
                        "schema announce acknowledged",
                    );
                    record.set_context("namespace", self.namespace.clone());
                    self.log.emit(&record).await;
                }
                Ok(_) => {
                    let mut record = LogRecord::now(
                        LogLevel::Warn,
                        "saikuro.rust.provider",
                        "schema announce rejected by runtime",
                    );
                    record.set_context("namespace", self.namespace.clone());
                    self.log.emit(&record).await;
                }
                Err(e) => {
                    let mut record = LogRecord::now(
                        LogLevel::Warn,
                        "saikuro.rust.provider",
                        "could not decode schema announce ack",
                    );
                    record.set_context("error", alloc::format!("{e}"));
                    self.log.emit(&record).await;
                }
            },
            Ok(Ok(None)) => {
                let mut record = LogRecord::now(
                    LogLevel::Warn,
                    "saikuro.rust.provider",
                    "transport closed after schema announce",
                );
                record.set_context("namespace", self.namespace.clone());
                self.log.emit(&record).await;
            }
            Ok(Err(e)) => {
                let mut record = LogRecord::now(
                    LogLevel::Warn,
                    "saikuro.rust.provider",
                    "error receiving schema announce ack",
                );
                record.set_context("error", alloc::format!("{e}"));
                self.log.emit(&record).await;
            }
            Err(_) => {
                let mut record = LogRecord::now(
                    LogLevel::Debug,
                    "saikuro.rust.provider",
                    "schema announce ack timed out, continuing",
                );
                record.set_context("namespace", self.namespace.clone());
                self.log.emit(&record).await;
            }
        }

        Ok(())
    }
}
