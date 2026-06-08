//! Saikuro provider: register Rust functions and serve them to the runtime.
//!

use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

use bytes::Bytes;
use saikuro_core::{
    envelope::{Envelope, InvocationType, ResponseEnvelope},
    error::{ErrorCode, ErrorDetail},
    invocation::InvocationId,
    schema::Schema,
};
use tracing::{debug, error, info, warn};

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
type HandlerFuture = Pin<Box<dyn Future<Output = Result<Value>> + Send>>;

/// A boxed handler that accepts args and returns a result.
type BoxedHandler = Arc<dyn Fn(HandlerArgs) -> HandlerFuture + Send + Sync>;

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
}

impl Provider {
    /// Create a new provider for the given namespace.
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            handlers: HashMap::new(),
            extra_namespaces: HashMap::new(),
        }
    }

    /// The namespace this provider publishes under.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    // Registration

    /// Register a function handler.
    ///
    /// The closure receives a `Vec<Value>` (JSON values) and must return a
    /// `Future<Output = Result<Value>>`.
    ///
    /// ```no_run
    /// # use saikuro::{Provider, Result};
    /// # let mut provider = Provider::new("math");
    /// provider.register("add", |args: Vec<serde_json::Value>| async move {
    ///     Ok(serde_json::json!(args[0].as_i64().unwrap_or(0) + args[1].as_i64().unwrap_or(0)))
    /// });
    /// ```
    pub fn register<F, Fut>(&mut self, name: impl Into<String>, handler: F)
    where
        F: Fn(HandlerArgs) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value>> + Send + 'static,
    {
        self.register_with_options(name, handler, RegisterOptions::default());
    }

    /// Register a function handler with schema metadata.
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
        debug!(namespace = %self.namespace, function = %name, "registering handler");
        let boxed: BoxedHandler = Arc::new(move |args| Box::pin(handler(args)));
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
    fn build_schema(&self) -> Schema {
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
        info!(namespace = %self.namespace, address = %addr, "connecting to runtime");
        let transport = connect(addr).await?;
        self.serve_on(transport).await
    }

    /// Serve on an already-connected transport.
    pub async fn serve_on(self, mut transport: Box<dyn AdapterTransport>) -> Result<()> {
        // Announce schema.
        self.announce(&mut *transport).await;

        // Serve loop.
        info!(namespace = %self.namespace, "provider ready, entering serve loop");
        let handlers = Arc::new(self.handlers);
        let namespace = Arc::new(self.namespace);

        loop {
            let frame = match transport.recv().await {
                Ok(Some(f)) => f,
                Ok(None) => {
                    info!(namespace = %namespace, "runtime closed connection");
                    break;
                }
                Err(e) => {
                    error!(namespace = %namespace, error = %e, "recv error");
                    break;
                }
            };

            let envelope = match Envelope::from_msgpack(&frame) {
                Ok(e) => e,
                Err(e) => {
                    warn!(error = %e, "malformed inbound envelope, skipping");
                    continue;
                }
            };

            // We handle dispatch inline (sequential per connection) because the
            // transport is not Clone.  Handlers that need true concurrency should
            // use the runtime's in-process provider API instead.
            match envelope.invocation_type {
                InvocationType::Call => {
                    dispatch_call(envelope, &handlers, &mut *transport).await;
                }
                InvocationType::Cast => {
                    // Fire-and-forget: dispatch the handler but send no response.
                    dispatch_cast(envelope, &handlers).await;
                }
                InvocationType::Batch => {
                    dispatch_batch(envelope, &handlers, &mut *transport).await;
                }
                other => {
                    warn!(
                        invocation_type = %other,
                        target = %envelope.target,
                        "provider received unsupported invocation type, skipping"
                    );
                }
            }
        }

        let _ = transport.close().await;
        Ok(())
    }

    // Announce

    async fn announce(&self, transport: &mut dyn AdapterTransport) {
        let schema = self.build_schema();
        let schema_value = match serde_json::to_value(&schema) {
            Ok(v) => json_to_core(v),
            Err(e) => {
                warn!(error = %e, "failed to serialize schema for announcement");
                return;
            }
        };

        let announce_env = Envelope::announce(schema_value);
        let frame = match announce_env.to_msgpack() {
            Ok(b) => Bytes::from(b),
            Err(e) => {
                warn!(error = %e, "failed to encode announce envelope");
                return;
            }
        };

        if let Err(e) = transport.send(frame).await {
            warn!(error = %e, "failed to send schema announce");
            return;
        }

        // Wait for the runtime ack.  The runtime must reply with ok_empty
        // before the provider can start serving; a timed-out or rejected ack
        // is non-fatal: the provider enters the serve loop regardless so that
        // direct-transport test setups (no runtime) work without a 5-second
        // delay.  A real deployment failure is surfaced via the tracing warning.
        match saikuro_exec::timeout(std::time::Duration::from_millis(500), transport.recv()).await {
            Ok(Ok(Some(ack_frame))) => match ResponseEnvelope::from_msgpack(&ack_frame) {
                Ok(ack) if ack.ok => {
                    debug!(namespace = %self.namespace, "schema announce acknowledged");
                }
                Ok(_) => {
                    warn!(namespace = %self.namespace, "schema announce rejected by runtime");
                }
                Err(e) => {
                    warn!(error = %e, "could not decode schema announce ack");
                }
            },
            Ok(Ok(None)) => {
                warn!(namespace = %self.namespace, "transport closed after schema announce");
            }
            Ok(Err(e)) => {
                warn!(error = %e, "error receiving schema announce ack");
            }
            Err(_) => {
                // Ack timed out.  Acceptable for direct-transport test setups;
                // in production this means the runtime is unresponsive.
                debug!(namespace = %self.namespace, "schema announce ack timed out, continuing");
            }
        }
    }
}
// Dispatch helpers
/// Dispatch a `Call` envelope, send the response (ok or error) to the runtime.
async fn dispatch_call(
    envelope: Envelope,
    handlers: &HashMap<String, HandlerEntry>,
    transport: &mut dyn AdapterTransport,
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
            send_response(transport, &response).await;
        }
        Err(Error::Remote { code, message, .. }) => {
            // Re-map the adapter's Remote error back onto the wire.  The code
            // is a PascalCase string from the remote side; round-trip it through
            // serde so unknown codes fall back to Internal.
            let error_code = parse_error_code(&code);
            send_error(transport, id, error_code, message).await;
        }
        Err(e) => {
            send_error(transport, id, ErrorCode::ProviderError, e.to_string()).await;
        }
    }
}

/// Dispatch a `Cast` envelope.  Runs the handler but never sends a response.
async fn dispatch_cast(envelope: Envelope, handlers: &HashMap<String, HandlerEntry>) {
    let fn_name = local_name(&envelope.target);
    let entry = match handlers.get(fn_name) {
        Some(e) => e,
        None => {
            // No handler: silently ignore.  Casts are fire-and-forget; the
            // caller does not expect a response or an error.
            debug!(target = %envelope.target, "cast: no handler registered, ignoring");
            return;
        }
    };

    let args: Vec<Value> = envelope.args.into_iter().map(core_to_json).collect();
    let handler = entry.handler.clone();

    if let Err(e) = handler(args).await {
        // Log the error but do not surface it to the caller.
        warn!(target = %envelope.target, error = %e, "cast handler returned error");
    }
}

/// Dispatch a `Batch` envelope.
///
/// Each item is dispatched in order.  Items that fail produce a null result
/// entry in the array; a structured per-item error envelope is not part of the
/// current batch wire format.  The batch as a whole always returns `ok`.
///
/// Items that are not of type `Call` (e.g. casts nested in a batch) are
/// executed but produce `null` in the result array.
async fn dispatch_batch(
    envelope: Envelope,
    handlers: &HashMap<String, HandlerEntry>,
    transport: &mut dyn AdapterTransport,
) {
    use saikuro_core::value::Value as CoreValue;

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
                                // Per-item failure: record null and log; the
                                // batch as a whole is not aborted.
                                warn!(
                                    target = %item.target,
                                    error = %e,
                                    "batch item handler error"
                                );
                                results.push(CoreValue::Null);
                            }
                        }
                    }
                    None => {
                        warn!(
                            target = %item.target,
                            "batch item: no handler registered"
                        );
                        results.push(CoreValue::Null);
                    }
                }
            }
            InvocationType::Cast => {
                // Execute the cast item but produce no result in the array.
                dispatch_cast(item, handlers).await;
                results.push(CoreValue::Null);
            }
            other => {
                warn!(
                    invocation_type = %other,
                    "batch item has unsupported type, skipping"
                );
                results.push(CoreValue::Null);
            }
        }
    }

    let response = ResponseEnvelope::ok(id, CoreValue::Array(results));
    send_response(transport, &response).await;
}
// Wire helpers
/// Extract the local function name from a fully-qualified `"namespace.fn"` target.
/// If there is no dot, returns the whole string.
fn local_name(target: &str) -> &str {
    match target.rsplit_once('.') {
        Some((_, name)) => name,
        None => target,
    }
}

/// Attempt to deserialise a PascalCase code string as an [`ErrorCode`].
/// Falls back to [`ErrorCode::Internal`] for unknown strings.
fn parse_error_code(s: &str) -> ErrorCode {
    // ErrorCode serialises as PascalCase via serde.  Wrap in a JSON string
    // and deserialise so that new codes added to the enum in future are
    // automatically handled without a match table here.
    serde_json::from_value(serde_json::Value::String(s.to_owned())).unwrap_or(ErrorCode::Internal)
}

async fn send_response(transport: &mut dyn AdapterTransport, response: &ResponseEnvelope) {
    match response.to_msgpack() {
        Ok(bytes) => {
            if let Err(e) = transport.send(Bytes::from(bytes)).await {
                error!(error = %e, "failed to send response");
            }
        }
        Err(e) => {
            error!(error = %e, "failed to encode response");
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
    send_response(transport, &response).await;
}
