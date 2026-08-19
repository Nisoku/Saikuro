//! Saikuro provider: register Rust functions and serve them to the runtime.
//!

#[cfg(not(feature = "std"))]
use alloc::collections::BTreeMap as HashMap;
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc;
use alloc::{
    borrow::ToOwned,
    boxed::Box,
    string::{String, ToString},
    vec::Vec,
};
use core::{future::Future, pin::Pin};
#[cfg(not(target_has_atomic = "ptr"))]
use portable_atomic_util::Arc;
#[cfg(feature = "std")]
use std::collections::HashMap;

use bytes::Bytes;
use saikuro_core::{
    envelope::{Envelope, InvocationType, ResponseEnvelope},
    invocation::InvocationId,
    schema::Schema,
};
use saikuro_event::{ErrorCode, ErrorDetail, LogLevel, LogRecord, LogSink};

use crate::{
    error::{Error, Result},
    schema::{build_schema, FunctionSchema, NamespaceSchema},
    transport::{connect, AdapterTransport},
    value::{core_to_json, json_to_core},
    Value,
};

/// Arguments passed to a registered handler function.
pub type HandlerArgs = Vec<Value>;

/// A boxed future returned by handler closures.
#[cfg(not(feature = "wasm"))]
type HandlerFuture = Pin<Box<dyn Future<Output = Result<Value>> + Send>>;
#[cfg(feature = "wasm")]
type HandlerFuture = Pin<Box<dyn Future<Output = Result<Value>>>>;

/// A boxed handler that accepts args and returns a result.
#[cfg(not(feature = "wasm"))]
type BoxedHandler = Arc<dyn Fn(HandlerArgs) -> HandlerFuture + Send + Sync>;
#[cfg(feature = "wasm")]
type BoxedHandler = Arc<dyn Fn(HandlerArgs) -> HandlerFuture>;

/// Options that can be supplied when registering a function.
#[derive(Debug, Clone, Default)]
pub struct RegisterOptions {
    pub schema: Option<FunctionSchema>,
}

/// Internal handler entry.
struct HandlerEntry {
    handler: BoxedHandler,
    schema: Option<FunctionSchema>,
}

/// A Saikuro provider that exposes Rust functions as invokable functions.
///
/// One `Provider` maps to one namespace.  It connects to the runtime, announces
/// its schema, then enters a serve loop dispatching inbound invocations.
pub struct Provider {
    namespace: String,
    handlers: HashMap<String, HandlerEntry>,
    extra_namespaces: HashMap<String, NamespaceSchema>,
    log: Arc<dyn LogSink>,
}

impl Provider {
    /// Create a new provider for the given namespace.
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            handlers: HashMap::new(),
            extra_namespaces: HashMap::new(),
            log: Arc::from(Box::new(saikuro_event::NullSink) as Box<dyn LogSink>),
        }
    }

    /// Set the log sink for this provider.
    pub fn with_log_sink(mut self, log: Arc<dyn LogSink>) -> Self {
        self.log = log;
        self
    }

    /// The namespace this provider publishes under.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    // Registration

    /// Register a function handler.
    #[cfg(not(feature = "wasm"))]
    pub fn register<F, Fut>(&mut self, name: impl Into<String>, handler: F)
    where
        F: Fn(HandlerArgs) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value>> + Send + 'static,
    {
        self.register_with_options(name, handler, RegisterOptions::default());
    }

    #[cfg(feature = "wasm")]
    pub fn register<F, Fut>(&mut self, name: impl Into<String>, handler: F)
    where
        F: Fn(HandlerArgs) -> Fut + 'static,
        Fut: Future<Output = Result<Value>> + 'static,
    {
        self.register_with_options(name, handler, RegisterOptions::default());
    }

    /// Register a function handler with schema metadata.
    #[cfg(not(feature = "wasm"))]
    pub fn register_with_options<F, Fut>(
        &mut self,
        name: impl Into<String>,
        handler: F,
        options: RegisterOptions,
    ) where
        F: Fn(HandlerArgs) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value>> + Send + 'static,
    {
        let name = name.into();
        let handler = move |args| Box::pin(handler(args)) as HandlerFuture;
        let boxed: BoxedHandler =
            Arc::from(Box::new(handler) as Box<dyn Fn(HandlerArgs) -> HandlerFuture + Send + Sync>);
        self.handlers.insert(
            name,
            HandlerEntry {
                handler: boxed,
                schema: options.schema,
            },
        );
    }

    #[cfg(feature = "wasm")]
    pub fn register_with_options<F, Fut>(
        &mut self,
        name: impl Into<String>,
        handler: F,
        options: RegisterOptions,
    ) where
        F: Fn(HandlerArgs) -> Fut + 'static,
        Fut: Future<Output = Result<Value>> + 'static,
    {
        let name = name.into();
        let handler = move |args| Box::pin(handler(args)) as HandlerFuture;
        let boxed: BoxedHandler =
            Arc::from(Box::new(handler) as Box<dyn Fn(HandlerArgs) -> HandlerFuture>);
        self.handlers.insert(
            name,
            HandlerEntry {
                handler: boxed,
                schema: options.schema,
            },
        );
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
        for (name, ns) in &self.extra_namespaces {
            all_ns.insert(name.clone(), ns.clone());
        }

        build_schema(&all_ns)
    }

    // Serving

    /// Connect to the runtime at `address` and serve invocations until the
    /// connection is closed or an unrecoverable error occurs.
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
                    dispatch_call(envelope, &handlers, &mut *transport, &*log).await;
                }
                InvocationType::Cast => {
                    dispatch_cast(envelope, &handlers, &*log).await;
                }
                InvocationType::Batch => {
                    dispatch_batch(envelope, &handlers, &mut *transport, &*log).await;
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
                return Err(Error::Codec(e.to_string()));
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
                return Err(Error::Codec(e.to_string()));
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
            return Err(Error::Transport(e.to_string()));
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

async fn dispatch_call(
    envelope: Envelope,
    handlers: &HashMap<String, HandlerEntry>,
    transport: &mut dyn AdapterTransport,
    log: &dyn LogSink,
) {
    let id = envelope.id;
    let target = envelope.target.clone();

    let fn_name = local_name(&target);
    let entry = match handlers.get(fn_name) {
        Some(e) => e,
        None => {
            send_error(
                transport,
                id,
                ErrorCode::FunctionNotFound,
                format!("no handler registered for '{target}'"),
            )
            .await;
            return;
        }
    };

    let args: Vec<Value> = envelope.args.into_iter().map(core_to_json).collect();
    let handler = entry.handler.clone();

    match handler(args).await {
        Ok(result) => {
            let response = ResponseEnvelope::ok(id, json_to_core(result));
            send_response(transport, &response, log).await;
        }
        Err(Error::Remote { code, message, .. }) => {
            let error_code = parse_error_code(&code);
            send_error(transport, id, error_code, message).await;
        }
        Err(e) => {
            send_error(transport, id, ErrorCode::ProviderError, e.to_string()).await;
        }
    }
}

async fn dispatch_cast(
    envelope: Envelope,
    handlers: &HashMap<String, HandlerEntry>,
    log: &dyn LogSink,
) {
    let fn_name = local_name(&envelope.target);
    let entry = match handlers.get(fn_name) {
        Some(e) => e,
        None => return,
    };

    let args: Vec<Value> = envelope.args.into_iter().map(core_to_json).collect();
    let handler = entry.handler.clone();

    if let Err(e) = handler(args).await {
        let mut record = LogRecord::now(
            LogLevel::Warn,
            "saikuro.rust.provider",
            "cast handler returned error",
        );
        record.set_context("target", envelope.target.clone());
        record.set_context("error", alloc::format!("{e}"));
        log.emit(&record).await;
    }
}

async fn dispatch_batch(
    envelope: Envelope,
    handlers: &HashMap<String, HandlerEntry>,
    transport: &mut dyn AdapterTransport,
    log: &dyn LogSink,
) {
    use saikuro_event::Value as CoreValue;

    let id = envelope.id;
    let items = match envelope.batch_items {
        Some(items) => items,
        None => {
            send_error(
                transport,
                id,
                ErrorCode::MalformedEnvelope,
                "batch envelope missing batch_items field",
            )
            .await;
            return;
        }
    };

    let mut results: Vec<CoreValue> = Vec::with_capacity(items.len());

    for item in items {
        match item.invocation_type {
            InvocationType::Call => {
                let fn_name = local_name(&item.target).to_owned();
                match handlers.get(&fn_name) {
                    Some(entry) => {
                        let args: Vec<Value> = item.args.into_iter().map(core_to_json).collect();
                        let handler = entry.handler.clone();
                        match handler(args).await {
                            Ok(v) => results.push(json_to_core(v)),
                            Err(e) => {
                                let mut record = LogRecord::now(
                                    LogLevel::Warn,
                                    "saikuro.rust.provider",
                                    "batch item handler error",
                                );
                                record.set_context("target", item.target.clone());
                                record.set_context("error", alloc::format!("{e}"));
                                log.emit(&record).await;
                                results.push(CoreValue::Null);
                            }
                        }
                    }
                    None => {
                        results.push(CoreValue::Null);
                    }
                }
            }
            InvocationType::Cast => {
                dispatch_cast(item, handlers, log).await;
                results.push(CoreValue::Null);
            }
            _other => {
                results.push(CoreValue::Null);
            }
        }
    }

    let response = ResponseEnvelope::ok(id, CoreValue::Array(results));
    send_response(transport, &response, log).await;
}

fn local_name(target: &str) -> &str {
    match target.rsplit_once('.') {
        Some((_, name)) => name,
        None => target,
    }
}

fn parse_error_code(s: &str) -> ErrorCode {
    serde_json::from_value(serde_json::Value::String(s.to_owned())).unwrap_or(ErrorCode::Internal)
}

async fn send_response(
    transport: &mut dyn AdapterTransport,
    response: &ResponseEnvelope,
    log: &dyn LogSink,
) {
    match response.to_msgpack() {
        Ok(bytes) => {
            if let Err(e) = transport.send(Bytes::from(bytes)).await {
                let mut record = LogRecord::now(
                    LogLevel::Error,
                    "saikuro.rust.provider",
                    "failed to send response",
                );
                record.set_context("error", alloc::format!("{e}"));
                log.emit(&record).await;
            }
        }
        Err(e) => {
            let mut record = LogRecord::now(
                LogLevel::Error,
                "saikuro.rust.provider",
                "failed to encode response",
            );
            record.set_context("error", alloc::format!("{e}"));
            log.emit(&record).await;
        }
    }
}

async fn send_error(
    transport: &mut dyn AdapterTransport,
    id: InvocationId,
    code: ErrorCode,
    message: impl Into<String>,
) {
    let detail = ErrorDetail::new(code, message);
    let response = ResponseEnvelope::err(id, detail);
    let _ = send_response_raw(transport, &response).await;
}

async fn send_response_raw(
    transport: &mut dyn AdapterTransport,
    response: &ResponseEnvelope,
) -> Result<()> {
    let bytes = response
        .to_msgpack()
        .map_err(|e| Error::Codec(e.to_string()))?;
    transport
        .send(Bytes::from(bytes))
        .await
        .map_err(|e| Error::Transport(e.to_string()))
}
