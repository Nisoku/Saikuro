use alloc::boxed::Box;
use alloc::vec::Vec;
use core::sync::atomic::Ordering;
use core::time::Duration;
#[cfg(not(target_has_atomic = "ptr"))]
use portable_atomic_util::Arc;
#[cfg(target_has_atomic = "ptr")]
use saikuro_core::Arc;

use portable_atomic::AtomicU64;
use saikuro_core::capability::CapabilitySet;
use saikuro_core::schema::Schema;
use saikuro_event::{LogLevel, LogRecord, LogSink};
use saikuro_exec::{sleep, spawn, timeout, watch};
use saikuro_router::provider::ProviderRegistry;
use saikuro_schema::{
    capability_engine::CapabilityEngine, registry::SchemaRegistry, validator::InvocationValidator,
};
use spin::RwLock;

use crate::transport_adapter::RuntimeListener;
use crate::{config::RuntimeConfig, handle::RuntimeHandle};

/// Milliseconds to wait before retrying after an accept error.
const ACCEPT_BACKOFF_MS: u64 = 50;

/// Monotonic counter for peer IDs (engine-agnostic, no `std::time`).
static PEER_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Allocate the next unique peer identifier for an accepted connection.
fn next_peer_id() -> alloc::string::String {
    let n = PEER_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    alloc::format!("peer-{n:08x}")
}

// Builder

/// Fluent builder for [`SaikuroRuntime`].
pub struct RuntimeBuilder {
    config: RuntimeConfig,
    log: Arc<dyn LogSink>,
}

impl RuntimeBuilder {
    fn new() -> Self {
        Self {
            config: RuntimeConfig::default(),
            log: Arc::from(Box::new(saikuro_event::NullSink) as Box<dyn LogSink>),
        }
    }

    pub fn config(mut self, config: RuntimeConfig) -> Self {
        self.config = config;
        self
    }

    pub fn mode(mut self, mode: crate::config::RuntimeMode) -> Self {
        self.config.mode = mode;
        self
    }

    pub fn call_timeout(mut self, timeout: Duration) -> Self {
        self.config.call_timeout = timeout;
        self
    }

    /// Supply the runtime schema as raw bytes (no `std::fs`). Used by the
    /// non-native entries (embedded / wasm / WASI) that bake the schema in.
    pub fn schema_bytes(mut self, bytes: &'static [u8]) -> Self {
        self.config.schema_bytes = Some(bytes);
        self
    }

    pub fn json_logs(mut self, enabled: bool) -> Self {
        self.config.json_logs = enabled;
        self
    }

    /// Set the log sink for the runtime.
    pub fn log_sink(mut self, log: Arc<dyn LogSink>) -> Self {
        self.log = log;
        self
    }

    /// Build the runtime.  This does not start any listener loops; use
    /// [`RuntimeHandle`] methods to attach transports, or [`SaikuroRuntime::serve`]
    /// to run a set of listeners until shutdown.
    pub async fn build(self) -> SaikuroRuntime {
        SaikuroRuntime::from_config(self.config, self.log).await
    }
}

/// The central Saikuro runtime instance.
///
/// Create one with `SaikuroRuntime::builder().build()` then use the returned
/// [`RuntimeHandle`] to interact with it from async tasks.
pub struct SaikuroRuntime {
    config: RuntimeConfig,
    schema_registry: SchemaRegistry,
    provider_registry: ProviderRegistry,
    capability_engine: CapabilityEngine,
    shutdown: Arc<RwLock<bool>>,
    log: Arc<dyn LogSink>,
}

impl SaikuroRuntime {
    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder::new()
    }

    async fn from_config(config: RuntimeConfig, log: Arc<dyn LogSink>) -> Self {
        let schema_bytes = config.schema_bytes;
        let schema_registry = SchemaRegistry::new();

        let runtime = Self {
            config,
            schema_registry,
            provider_registry: ProviderRegistry::new(),
            capability_engine: CapabilityEngine::new(),
            shutdown: Arc::new(RwLock::new(false)),
            log: log.clone(),
        };

        // Register a baked-in schema (embedded / wasm / WASI) or a schema the
        // native entry point loaded from disk. Done before the production freeze.
        if let Some(bytes) = schema_bytes {
            match serde_json::from_slice::<Schema>(bytes) {
                Ok(schema) => {
                    if let Err(e) = runtime.schema_registry.merge_schema(schema, "static").await {
                        let mut record = LogRecord::now(
                            LogLevel::Error,
                            "saikuro.runtime",
                            "failed to merge static schema",
                        );
                        record.set_context("error", alloc::format!("{}", e));
                        log.emit(&record).await;
                    }
                }
                Err(e) => {
                    let mut record = LogRecord::now(
                        LogLevel::Error,
                        "saikuro.runtime",
                        "failed to parse static schema",
                    );
                    record.set_context("error", alloc::format!("{}", e));
                    log.emit(&record).await;
                }
            }
        }

        if runtime.config.mode == crate::config::RuntimeMode::Production {
            runtime.schema_registry.freeze().await;
        }

        runtime
    }

    /// Return a shared reference to the schema registry.
    pub fn schema_registry(&self) -> &SchemaRegistry {
        &self.schema_registry
    }

    /// Return a shared reference to the provider registry.
    pub fn provider_registry(&self) -> &ProviderRegistry {
        &self.provider_registry
    }

    /// Return a shared reference to the capability engine.
    pub fn capability_engine(&self) -> &CapabilityEngine {
        &self.capability_engine
    }

    /// Build an [`InvocationValidator`] configured for this runtime.
    pub fn validator(&self) -> InvocationValidator {
        InvocationValidator::new(self.schema_registry.clone())
    }

    /// Produce a cheap [`RuntimeHandle`] that can be cloned and shared across
    /// tasks.
    pub fn handle(&self) -> RuntimeHandle {
        RuntimeHandle {
            schema_registry: self.schema_registry.clone(),
            provider_registry: self.provider_registry.clone(),
            capability_engine: self.capability_engine.clone(),
            config: self.config.clone(),
            shutdown: self.shutdown.clone(),
            log: self.log.clone(),
        }
    }

    /// Signal a graceful shutdown.
    pub async fn shutdown(&self) {
        *self.shutdown.write() = true;
        let record = LogRecord::now(
            LogLevel::Info,
            "saikuro.runtime",
            "saikuro runtime shutting down",
        );
        self.log.emit(&record).await;
    }

    pub fn is_shutdown(&self) -> bool {
        *self.shutdown.read()
    }

    /// Run a set of listeners until the host signals shutdown via `shutdown`.
    pub async fn serve<L: RuntimeListener + 'static>(
        &self,
        listeners: Vec<L>,
        mut shutdown: watch::Receiver<bool>,
    ) {
        let mut tasks = alloc::vec::Vec::new();
        for mut listener in listeners {
            let handle = self.handle();
            let mut rx = shutdown.clone();
            let log = self.log.clone();
            tasks.push(spawn(async move {
                loop {
                    saikuro_exec::select! {
                        result = listener.accept() => {
                            match result {
                                Ok(Some(transport)) => {
                                    let id = next_peer_id();
                                    let mut record = LogRecord::now(
                                        LogLevel::Info,
                                        "saikuro.runtime",
                                        "connection accepted",
                                    );
                                    record.set_context("peer", id.clone());
                                    log.emit(&record).await;
                                    handle.accept_transport(transport, id, CapabilitySet::default());
                                }
                                Ok(None) => {
                                    let record = LogRecord::now(
                                        LogLevel::Info,
                                        "saikuro.runtime",
                                        "listener closed",
                                    );
                                    log.emit(&record).await;
                                    break;
                                }
                                Err(e) => {
                                    let mut record = LogRecord::now(
                                        LogLevel::Error,
                                        "saikuro.runtime",
                                        "accept error",
                                    );
                                    record.set_context("error", alloc::format!("{}", e));
                                    log.emit(&record).await;
                                    sleep(Duration::from_millis(ACCEPT_BACKOFF_MS)).await;
                                }
                            }
                        }
                        changed = rx.changed() => {
                            if changed.is_err() || rx.borrow() {
                                let record = LogRecord::now(
                                    LogLevel::Info,
                                    "saikuro.runtime",
                                    "listener shutting down",
                                );
                                log.emit(&record).await;
                                break;
                            }
                        }
                    }
                }
                let _ = listener.close().await;
            }));
        }

        // Wait until the host signals shutdown.
        while !shutdown.borrow() {
            if shutdown.changed().await.is_err() {
                break;
            }
        }

        // Allow in-flight listener tasks to observe the shutdown flag.
        for task in tasks {
            let _ = timeout(Duration::from_secs(5), task).await;
        }

        let record = LogRecord::now(
            LogLevel::Info,
            "saikuro.runtime",
            "saikuro runtime listener set stopped",
        );
        self.log.emit(&record).await;
    }
}
