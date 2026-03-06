//! [`RuntimeHandle`]:  the cheap, cloneable interface to a running Saikuro
//! runtime that async tasks and adapters interact with.
//!
//! The handle exposes the full high-level API:
//! - Schema registration / lookup
//! - Provider registration / deregistration
//! - Dispatching invocations programmatically (for in-process providers)
//! - Connecting transports and spawning connection handlers

use std::sync::Arc;

use parking_lot::RwLock;
use saikuro_core::{
    capability::CapabilitySet, envelope::Envelope, schema::Schema, ResponseEnvelope,
};
use saikuro_router::{
    provider::{ProviderHandle, ProviderRegistry, ProviderWorkItem},
    router::{InvocationRouter, RouterConfig},
};
use saikuro_schema::{
    capability_engine::CapabilityEngine,
    registry::{NamespaceRegistration, SchemaRegistry},
    validator::InvocationValidator,
};
use saikuro_transport::traits::Transport;
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::{config::RuntimeConfig, connection::ConnectionHandler, error::Result};

/// A cheap, `Clone`-able handle to a running [`SaikuroRuntime`].
///
/// All internal state is `Arc`-wrapped so cloning is O(1).
#[derive(Clone)]
pub struct RuntimeHandle {
    pub(crate) schema_registry: SchemaRegistry,
    pub(crate) provider_registry: ProviderRegistry,
    pub(crate) capability_engine: CapabilityEngine,
    pub(crate) config: RuntimeConfig,
    pub(crate) shutdown: Arc<RwLock<bool>>,
}

impl RuntimeHandle {
    // Schema

    /// Register or merge a schema document from a newly-connected provider.
    pub fn register_schema(&self, schema: Schema, provider_id: impl Into<String>) -> Result<()> {
        self.schema_registry
            .merge_schema(schema, provider_id)
            .map_err(Into::into)
    }

    /// Register a single namespace from a provider.
    pub fn register_namespace(&self, reg: NamespaceRegistration) -> Result<()> {
        self.schema_registry.register(reg).map_err(Into::into)
    }

    /// Deregister all schemas owned by a provider (called on disconnect).
    pub fn deregister_provider_schema(&self, provider_id: &str) {
        self.schema_registry.deregister_provider(provider_id);
    }

    /// Export a snapshot of the current schema state.
    pub fn schema_snapshot(&self) -> Schema {
        self.schema_registry.snapshot()
    }

    // Providers

    /// Register a provider handle so the router can dispatch to it.
    pub fn register_provider(&self, handle: ProviderHandle) {
        self.provider_registry.register(handle);
    }

    /// Deregister a provider by ID (called on disconnect).
    pub fn deregister_provider(&self, provider_id: &str) {
        self.provider_registry.deregister(provider_id);
        self.schema_registry.deregister_provider(provider_id);
    }

    // Dispatch

    /// Dispatch an invocation directly (bypassing transport encoding).
    ///
    /// Used by in-process providers and the test harness.
    /// Validation and capability-checking are still performed.
    pub async fn dispatch(
        &self,
        envelope: Envelope,
        caller_caps: &CapabilitySet,
    ) -> ResponseEnvelope {
        let validator = InvocationValidator::new(self.schema_registry.clone());
        let router = self.build_router();

        // Validate
        let validation = match validator.validate(&envelope) {
            Ok(r) => r,
            Err(e) => {
                return ResponseEnvelope::err(
                    envelope.id,
                    saikuro_core::error::ErrorDetail::new(e.error_code(), e.to_string()),
                );
            }
        };

        // Capability check
        use saikuro_schema::capability_engine::CapabilityOutcome;
        if let CapabilityOutcome::Denied { missing } = self
            .capability_engine
            .check_ref(caller_caps, &validation.function_ref)
        {
            return ResponseEnvelope::err(
                envelope.id,
                saikuro_core::error::ErrorDetail::new(
                    saikuro_core::error::ErrorCode::CapabilityDenied,
                    format!("missing capability '{missing}' for '{}'", envelope.target),
                ),
            );
        }

        router.dispatch(envelope).await
    }

    // Transport connection

    /// Accept a connected transport and spawn a connection handler task.
    ///
    /// `peer_id` is a stable identifier for the peer (used in logs and for
    /// provider deregistration).
    ///
    /// `peer_caps` are the capabilities granted to this peer; they are checked
    /// on every invocation it sends.
    pub fn accept_transport<T: Transport>(
        &self,
        transport: T,
        peer_id: impl Into<String>,
        peer_caps: CapabilitySet,
    ) {
        let peer_id = peer_id.into();
        let (sender, receiver) = transport.split();
        let handler = ConnectionHandler {
            peer_id: peer_id.clone(),
            sender,
            receiver,
            validator: InvocationValidator::new(self.schema_registry.clone()),
            capability_engine: self.capability_engine.clone(),
            router: self.build_router(),
            peer_capabilities: peer_caps,
            max_message_size: self.config.max_message_size,
            schema_registry: self.schema_registry.clone(),
            provider_registry: self.provider_registry.clone(),
        };

        info!(peer = %peer_id, "spawning connection handler");
        tokio::spawn(handler.run());
    }

    // In-process provider registration

    /// Register a Rust closure as an in-process provider for a namespace.
    ///
    /// The closure receives an [`Envelope`] and must return a
    /// [`ResponseEnvelope`].  It runs in a spawned task for each invocation.
    ///
    /// This is the primary API for writing Rust-native providers.
    pub fn register_fn_provider<F, Fut>(
        &self,
        provider_id: impl Into<String>,
        namespaces: Vec<String>,
        handler: F,
    ) where
        F: Fn(Envelope) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ResponseEnvelope> + Send + 'static,
    {
        let provider_id = provider_id.into();
        let (work_tx, mut work_rx) = mpsc::channel::<ProviderWorkItem>(256);

        let handle = ProviderHandle::new(provider_id.clone(), namespaces.clone(), work_tx);
        self.provider_registry.register(handle);

        let handler = Arc::new(handler);

        debug!(provider = %provider_id, "in-process provider registered");

        tokio::spawn(async move {
            while let Some(item) = work_rx.recv().await {
                let handler = handler.clone();
                tokio::spawn(async move {
                    let response = handler(item.envelope).await;
                    if let Some(tx) = item.response_tx {
                        let _ = tx.send(response);
                    }
                });
            }
        });
    }

    // Helpers

    fn build_router(&self) -> InvocationRouter {
        InvocationRouter::new(
            self.provider_registry.clone(),
            RouterConfig {
                call_timeout: self.config.call_timeout,
                stream_channel_capacity: self.config.stream_buffer_capacity,
                channel_capacity: self.config.stream_buffer_capacity,
            },
        )
    }

    pub fn is_shutdown(&self) -> bool {
        *self.shutdown.read()
    }
}
