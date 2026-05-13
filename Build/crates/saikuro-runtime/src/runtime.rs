//! The main Saikuro runtime and its builder.

use std::sync::Arc;

use parking_lot::RwLock;
use saikuro_router::{provider::ProviderRegistry, router::InvocationRouter};
use saikuro_schema::{
    capability_engine::CapabilityEngine, registry::SchemaRegistry, validator::InvocationValidator,
};
use tracing::info;

use crate::{
    config::{RuntimeConfig, RuntimeMode},
    handle::RuntimeHandle,
};

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

    pub fn mode(mut self, mode: RuntimeMode) -> Self {
        self.config.mode = mode;
        self
    }

    pub fn call_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.config.call_timeout = timeout;
        self
    }

    pub fn json_logs(mut self, enabled: bool) -> Self {
        self.config.json_logs = enabled;
        self
    }

    /// Build the runtime.  This does not start any listener loops; use
    /// [`RuntimeHandle`] methods to attach transports.
    pub fn build(self) -> SaikuroRuntime {
        SaikuroRuntime::from_config(self.config)
    }
}

// Runtime

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
        let schema_registry = SchemaRegistry::new();

        if config.mode == RuntimeMode::Production {
            schema_registry.freeze();
        }

        Self {
            config,
            schema_registry,
            provider_registry: ProviderRegistry::new(),
            capability_engine: CapabilityEngine::new(),
            shutdown: Arc::new(RwLock::new(false)),
        }
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

    /// Build an [`InvocationRouter`] configured for this runtime.
    pub fn router(&self) -> InvocationRouter {
        InvocationRouter::new(self.provider_registry.clone(), self.config.router_config())
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
}
