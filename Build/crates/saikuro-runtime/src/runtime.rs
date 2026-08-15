use alloc::sync::Arc;
use core::sync::atomic::Ordering;
use core::time::Duration;

use portable_atomic::AtomicU64;
use saikuro_core::capability::CapabilitySet;
use saikuro_core::schema::Schema;
use saikuro_exec::{sleep, spawn, timeout, watch};
use saikuro_router::provider::ProviderRegistry;
use saikuro_schema::{
    capability_engine::CapabilityEngine, registry::SchemaRegistry, validator::InvocationValidator,
};
use spin::RwLock;
use tracing::{error, info};

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
}

impl RuntimeBuilder {
    fn new() -> Self {
        Self {
            config: RuntimeConfig::default(),
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

    /// Build the runtime.  This does not start any listener loops; use
    /// [`RuntimeHandle`] methods to attach transports, or [`SaikuroRuntime::serve`]
    /// to run a set of listeners until shutdown.
    pub fn build(self) -> SaikuroRuntime {
        SaikuroRuntime::from_config(self.config)
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
}

impl SaikuroRuntime {
    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder::new()
    }

    fn from_config(config: RuntimeConfig) -> Self {
        let schema_bytes = config.schema_bytes;
        let schema_registry = SchemaRegistry::new();

        let mut runtime = Self {
            config,
            schema_registry,
            provider_registry: ProviderRegistry::new(),
            capability_engine: CapabilityEngine::new(),
            shutdown: Arc::new(RwLock::new(false)),
        };

        // Register a baked-in schema (embedded / wasm / WASI) or a schema the
        // native entry point loaded from disk. Done before the production freeze.
        if let Some(bytes) = schema_bytes {
            match serde_json::from_slice::<Schema>(bytes) {
                Ok(schema) => {
                    if let Err(e) = runtime.schema_registry.merge_schema(schema, "static") {
                        error!(error = %e, "failed to merge static schema");
                    }
                }
                Err(e) => error!(error = %e, "failed to parse static schema"),
            }
        }

        if runtime.config.mode == crate::config::RuntimeMode::Production {
            runtime.schema_registry.freeze();
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
        }
    }

    /// Signal a graceful shutdown.
    pub fn shutdown(&self) {
        *self.shutdown.write() = true;
        info!("saikuro runtime shutting down");
    }

    pub fn is_shutdown(&self) -> bool {
        *self.shutdown.read()
    }

    /// Run a set of listeners until the host signals shutdown via `shutdown`.
    pub async fn serve<L: RuntimeListener>(
        &self,
        listeners: Vec<L>,
        mut shutdown: watch::Receiver<bool>,
    ) {
        let mut tasks = alloc::vec::Vec::new();
        for mut listener in listeners {
            let handle = self.handle();
            let mut rx = shutdown.clone();
            tasks.push(spawn(async move {
                loop {
                    saikuro_exec::select! {
                        result = listener.accept() => {
                            match result {
                                Ok(Some(transport)) => {
                                    let id = next_peer_id();
                                    info!(peer = %id, "connection accepted");
                                    handle.accept_transport(transport, id, CapabilitySet::default());
                                }
                                Ok(None) => {
                                    info!("listener closed");
                                    break;
                                }
                                Err(e) => {
                                    error!(error = %e, "accept error");
                                    sleep(Duration::from_millis(ACCEPT_BACKOFF_MS)).await;
                                }
                            }
                        }
                        changed = rx.changed() => {
                            if changed.is_err() || rx.borrow() {
                                info!("listener shutting down");
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

        info!("saikuro runtime listener set stopped");
    }
}
