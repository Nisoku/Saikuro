use alloc::{
    borrow::ToOwned, boxed::Box, collections::BTreeMap, string::String, sync::Arc, vec::Vec,
};
use async_trait::async_trait;
use saikuro_core::{envelope::Envelope, RegistrationToken, ResponseEnvelope};
use saikuro_exec::sync::RwLock;
use saikuro_exec::{mpsc, oneshot};

use crate::error::{Result, RouterError};

// Pending call tracker
/// A one-shot channel waiting for the response to a single Call invocation.
pub type PendingCallSender = oneshot::Sender<ResponseEnvelope>;
pub type PendingCallReceiver = oneshot::Receiver<ResponseEnvelope>;

// Provider trait
/// An abstract provider that can receive invocations.
#[async_trait]
pub trait Provider: Send + Sync + 'static {
    /// The unique identifier for this provider connection.
    fn id(&self) -> &str;

    /// The namespaces this provider handles.
    fn namespaces(&self) -> &[String];

    /// Send an invocation envelope to this provider.
    ///
    /// For `Call` invocations the caller attaches a `response_tx` oneshot
    /// sender; the provider must eventually call `response_tx.send(...)` to
    /// complete the call.
    async fn send_invocation(
        &self,
        envelope: Envelope,
        response_tx: Option<PendingCallSender>,
    ) -> Result<()>;

    /// Returns `true` if this provider is still alive and can accept work.
    fn is_alive(&self) -> bool;
}

// ProviderHandle

/// Work item sent through the provider's dispatch channel.
pub struct ProviderWorkItem {
    pub envelope: Envelope,
    pub response_tx: Option<PendingCallSender>,
}

/// A cheap, cloneable handle to a connected provider.
#[derive(Clone)]
pub struct ProviderHandle {
    id: String,
    registration_token: RegistrationToken,
    namespaces: Vec<String>,
    sender: mpsc::Sender<ProviderWorkItem>,
}

impl ProviderHandle {
    pub fn new(
        id: impl Into<String>,
        namespaces: Vec<String>,
        sender: mpsc::Sender<ProviderWorkItem>,
    ) -> Self {
        Self::with_registration_token(id, RegistrationToken::new(), namespaces, sender)
    }

    /// Build a provider handle for an existing registration.
    pub fn with_registration_token(
        id: impl Into<String>,
        registration_token: RegistrationToken,
        namespaces: Vec<String>,
        sender: mpsc::Sender<ProviderWorkItem>,
    ) -> Self {
        Self {
            id: id.into(),
            registration_token,
            namespaces,
            sender,
        }
    }

    /// Return the identity of this specific provider registration.
    pub fn registration_token(&self) -> RegistrationToken {
        self.registration_token
    }
}

#[async_trait]
impl Provider for ProviderHandle {
    fn id(&self) -> &str {
        &self.id
    }

    fn namespaces(&self) -> &[String] {
        &self.namespaces
    }

    async fn send_invocation(
        &self,
        envelope: Envelope,
        response_tx: Option<PendingCallSender>,
    ) -> Result<()> {
        self.sender
            .send(ProviderWorkItem {
                envelope,
                response_tx,
            })
            .await
            .map_err(|_| RouterError::ProviderUnavailable(self.id.clone()))
    }

    fn is_alive(&self) -> bool {
        !self.sender.is_closed()
    }
}

// ProviderRegistry
/// Thread-safe registry mapping namespace names to provider handles.
#[derive(Clone, Default)]
pub struct ProviderRegistry {
    inner: Arc<RwLock<RegistryState>>,
}

#[derive(Default)]
struct RegistryState {
    /// namespace -> provider handle
    by_namespace: BTreeMap<String, ProviderHandle>,
    /// provider identity -> list of namespaces (for cleanup on disconnect)
    by_provider: BTreeMap<(String, RegistrationToken), Vec<String>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a provider handle for the given namespaces.
    pub async fn register(&self, handle: ProviderHandle) {
        let provider_id = handle.id().to_owned();
        let registration_token = handle.registration_token();
        let provider_key = (provider_id.clone(), registration_token);
        let namespaces = handle.namespaces().to_vec();

        let mut state = self.inner.write().await;

        // A re-registering provider that dropped a namespace must release its route.
        let dropped: Vec<String> = state
            .by_provider
            .get(&provider_key)
            .map(|owned| {
                owned
                    .iter()
                    .filter(|ns| !namespaces.contains(ns))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        for ns in &dropped {
            if state
                .by_namespace
                .get(ns)
                .map(|h| h.id() == provider_id && h.registration_token() == registration_token)
                .unwrap_or(false)
            {
                state.by_namespace.remove(ns);
            }
        }

        for ns in &namespaces {
            match state.by_namespace.insert(ns.clone(), handle.clone()) {
                Some(old) => {
                    if old.id() != provider_id || old.registration_token() != registration_token {
                        let old_key = (old.id().to_owned(), old.registration_token());
                        if let Some(old_ns_list) = state.by_provider.get_mut(&old_key) {
                            old_ns_list.retain(|n| n != ns);
                        }
                    }
                }
                None => {
                }
            }
        }
        state.by_provider.insert(provider_key, namespaces);
    }

    /// Remove all namespaces owned by one specific provider registration.
    pub async fn deregister(&self, provider_id: &str, registration_token: RegistrationToken) {
        let mut state = self.inner.write().await;
        let provider_key = (provider_id.to_owned(), registration_token);
        if let Some(namespaces) = state.by_provider.remove(&provider_key) {
            for ns in namespaces {
                if state
                    .by_namespace
                    .get(&ns)
                    .map(|h| h.id() == provider_id && h.registration_token() == registration_token)
                    .unwrap_or(false)
                {
                    state.by_namespace.remove(&ns);
                }
            }
        }
    }

    /// Look up the provider for a namespace.
    pub async fn get(&self, namespace: &str) -> Option<ProviderHandle> {
        self.inner.read().await.by_namespace.get(namespace).cloned()
    }

    /// Return `true` if a live provider exists for the namespace.
    pub async fn has_live_provider(&self, namespace: &str) -> bool {
        self.inner
            .read()
            .by_namespace
            .get(namespace)
            .map(|h| h.is_alive())
            .unwrap_or(false)
    }
}
