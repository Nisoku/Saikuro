use alloc::string::{String, ToString};
use alloc::vec::Vec;
#[cfg(not(target_has_atomic = "ptr"))]
use portable_atomic_util::Arc;
#[cfg(target_has_atomic = "ptr")]
use saikuro_core::Arc;

use saikuro_core::{
    capability::CapabilitySet, envelope::Envelope, schema::Schema, RegistrationToken,
    ResponseEnvelope,
};
use saikuro_event::{LogLevel, LogRecord, LogSink};
use saikuro_exec::mpsc;
use saikuro_router::{
    provider::{ProviderHandle, ProviderRegistry, ProviderWorkItem},
    router::InvocationRouter,
};
use saikuro_schema::{
    capability_engine::CapabilityEngine,
    registry::{NamespaceRegistration, SchemaRegistry},
    validator::InvocationValidator,
};
use spin::RwLock;

use crate::config::RuntimeConfig;
use crate::connection::ConnectionHandler;
use crate::transport_adapter::RuntimeTransport;
use saikuro_event::Result;
use saikuro_exec::JoinHandle;

/// Guard returned by [`RuntimeHandle::register_fn_provider`].
///
/// Dropping this handle deregisters the provider from routing but does
/// **not** wait for the worker task to exit.  Call [`shutdown`](Self::shutdown)
/// to deregister *and* await the worker's exit.
#[must_use]
pub struct FnProviderHandle {
    provider_id: String,
    registration_token: RegistrationToken,
    provider_registry: ProviderRegistry,
    schema_registry: SchemaRegistry,
    join: JoinHandle<()>,
}

impl FnProviderHandle {
    /// Deregister the provider from routing and wait for the worker task
    /// to finish processing any in-flight requests.
    pub async fn shutdown(self) {
        self.provider_registry
            .deregister(&self.provider_id, self.registration_token)
            .await;
        self.schema_registry
            .deregister_provider(&self.provider_id, self.registration_token)
            .await;
        let _ = self.join.await;
    }

    /// The provider identity.
    pub fn id(&self) -> &str {
        &self.provider_id
    }

    /// The registration token for this provider.
    pub fn registration_token(&self) -> RegistrationToken {
        self.registration_token
    }
}

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
    pub(crate) log: Arc<dyn LogSink>,
}

impl RuntimeHandle {
    // Schema

    /// Register or merge a schema document from a newly-connected provider.
    pub async fn register_schema(
        &self,
        schema: Schema,
        provider_id: impl Into<String>,
    ) -> Result<()> {
        self.schema_registry.merge_schema(schema, provider_id).await
    }

    /// Register or merge a schema under an existing provider registration.
    pub async fn register_schema_with_token(
        &self,
        schema: Schema,
        provider_id: impl Into<String>,
        registration_token: RegistrationToken,
    ) -> Result<()> {
        self.schema_registry
            .merge_schema_with_token(schema, provider_id, registration_token)
            .await
    }

    /// Register a single namespace from a provider.
    pub async fn register_namespace(&self, reg: NamespaceRegistration) -> Result<()> {
        self.schema_registry.register(reg).await
    }

    /// Deregister all schemas owned by a provider (called on disconnect).
    pub async fn deregister_provider_schema(
        &self,
        provider_id: &str,
        registration_token: RegistrationToken,
    ) {
        self.schema_registry
            .deregister_provider(provider_id, registration_token)
            .await;
    }

    /// Export a snapshot of the current schema state.
    pub async fn schema_snapshot(&self) -> Result<Schema> {
        self.schema_registry.snapshot().await
    }

    // Providers

    /// Register a provider handle so the router can dispatch to it.
    pub async fn register_provider(&self, handle: ProviderHandle) {
        self.provider_registry.register(handle).await;
    }

    /// Deregister one provider generation from routing and schema ownership.
    pub async fn deregister_provider(
        &self,
        provider_id: &str,
        registration_token: RegistrationToken,
    ) {
        self.provider_registry
            .deregister(provider_id, registration_token)
            .await;
        self.schema_registry
            .deregister_provider(provider_id, registration_token)
            .await;
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
        let validation = match validator.validate(&envelope).await {
            Ok(r) => r,
            Err(e) => {
                return ResponseEnvelope::err(
                    envelope.id,
                    saikuro_event::ErrorDetail::new(e.error_code(), e.to_string()),
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
                saikuro_event::ErrorDetail::new(
                    saikuro_event::ErrorCode::CapabilityDenied,
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
    pub fn accept_transport<T: RuntimeTransport + 'static>(
        &self,
        transport: T,
        peer_id: impl Into<String>,
        peer_caps: CapabilitySet,
    ) {
        let peer_id = peer_id.into();
        let (sender, receiver) = transport.split();
        let handler = ConnectionHandler {
            peer_id: peer_id.clone(),
            registration_token: RegistrationToken::new(),
            sender,
            receiver,
            validator: InvocationValidator::new(self.schema_registry.clone()),
            capability_engine: self.capability_engine.clone(),
            router: self.build_router(),
            peer_capabilities: peer_caps,
            max_message_size: self.config.max_message_size,
            schema_registry: self.schema_registry.clone(),
            provider_registry: self.provider_registry.clone(),
            log: self.log.clone(),
        };

        let log = self.log.clone();
        let peer_id_clone = peer_id.clone();
        saikuro_exec::spawn(async move {
            let mut record = LogRecord::now(
                LogLevel::Info,
                "saikuro.runtime",
                "spawning connection handler",
            );
            record.set_context("peer", peer_id_clone);
            log.emit(&record).await;
            handler.run().await;
        });
    }

    // In-process provider registration

    /// Register a Rust closure as an in-process provider for a namespace.
    ///
    /// The closure receives an [`Envelope`] and must return a
    /// [`ResponseEnvelope`].  It runs in a spawned task for each invocation.
    ///
    /// This is the primary API for writing Rust-native providers.
    pub async fn register_fn_provider<F, Fut>(
        &self,
        provider_id: impl Into<String>,
        namespaces: Vec<String>,
        handler: F,
    ) -> FnProviderHandle
    where
        F: Fn(Envelope) -> Fut + Send + Sync + 'static,
        Fut: core::future::Future<Output = ResponseEnvelope> + Send + 'static,
    {
        let provider_id = provider_id.into();
        let registration_token = RegistrationToken::new();
        let (work_tx, mut work_rx) =
            mpsc::channel::<ProviderWorkItem>(saikuro_exec::ChannelCapacity::MAX);

        let handle = ProviderHandle::with_registration_token(
            provider_id.clone(),
            registration_token,
            namespaces.clone(),
            work_tx,
        );
        self.provider_registry.register(handle).await;

        let handler = Arc::new(handler);

        {
            let mut record = LogRecord::now(
                LogLevel::Debug,
                "saikuro.runtime",
                "in-process provider registered",
            );
            record.set_context("provider", provider_id.clone());
            self.log.emit(&record).await;
        }

        let join = saikuro_exec::spawn(async move {
            while let Some(item) = work_rx.recv().await {
                let handler = handler.clone();
                saikuro_exec::spawn(async move {
                    let response = handler(item.envelope).await;
                    if let Some(tx) = item.response_tx {
                        let _ = tx.send(response);
                    }
                });
            }
        });

        FnProviderHandle {
            provider_id,
            registration_token,
            provider_registry: self.provider_registry.clone(),
            schema_registry: self.schema_registry.clone(),
            join,
        }
    }

    // Helpers

    fn build_router(&self) -> InvocationRouter {
        InvocationRouter::new(self.provider_registry.clone(), self.config.router_config())
    }

    pub fn is_shutdown(&self) -> bool {
        *self.shutdown.read()
    }
}
