//! Provider abstraction and registry.
//!
//! A **provider** is any entity that can handle invocations for one or more
//! namespaces.  In practice it is a connected language adapter (Python,
//! TypeScript, …) that has registered its schema and is listening for work.
//!
//! The [`ProviderRegistry`] maps namespace names to [`ProviderHandle`]s.
//! Each handle wraps a MPSC sender so the router can dispatch work
//! without blocking.

use alloc::{
    borrow::ToOwned, boxed::Box, collections::BTreeMap, string::String, sync::Arc, vec::Vec,
};
use async_trait::async_trait;
use saikuro_core::{envelope::Envelope, sync::RwLock, RegistrationToken, ResponseEnvelope};
use saikuro_exec::{mpsc, oneshot};
use tracing::{debug, warn};

use crate::error::{Result, RouterError};

// Pending call tracker

/// A one-shot channel waiting for the response to a single Call invocation.
pub type PendingCallSender = oneshot::Sender<ResponseEnvelope>;
pub type PendingCallReceiver = oneshot::Receiver<ResponseEnvelope>;

// Provider trait

/// An abstract provider that can receive invocations.
///
/// The `send_invocation` method is the only interface the router uses; concrete
/// provider implementations may queue, dispatch, or transform the envelope in
/// any way they choose.
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
///
/// Internally holds a bounded MPSC sender; backpressure naturally propagates
/// from here back to the caller when the provider's work queue is full.
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
///
/// Both indexes live behind a single [`RwLock`] so `register` and `deregister`
/// keep them consistent atomically.  A namespace taken over by a new provider
/// is removed from the old provider's record, and deregistration never removes
/// a namespace that a later provider now owns.
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
    ///
    /// If a namespace already has a provider, the old one is replaced.  The
    /// namespace is then removed from the old provider's record so a later
    /// deregistration of the old provider cannot reclaim the new provider's
    /// namespace.  Re-registering a provider with fewer namespaces releases
    /// the routes it no longer owns (unless a newer provider took them over).
    pub fn register(&self, handle: ProviderHandle) {
        let provider_id = handle.id().to_owned();
        let registration_token = handle.registration_token();
        let provider_key = (provider_id.clone(), registration_token);
        let namespaces = handle.namespaces().to_vec();

        let mut state = self.inner.write();

        // A re-registering provider that dropped a namespace must release its
        // route.  Remove each previously-owned namespace that is absent from
        // the new list, but only while it still points at this provider (a
        // newer provider may have taken it over).
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
                debug!(namespace = %ns, provider = %provider_id, "released dropped namespace route");
            }
        }

        for ns in &namespaces {
            match state.by_namespace.insert(ns.clone(), handle.clone()) {
                Some(old) => {
                    warn!(namespace = %ns, provider = %provider_id, "replacing existing namespace provider");
                    if old.id() != provider_id || old.registration_token() != registration_token {
                        let old_key = (old.id().to_owned(), old.registration_token());
                        if let Some(old_ns_list) = state.by_provider.get_mut(&old_key) {
                            old_ns_list.retain(|n| n != ns);
                        }
                    }
                }
                None => {
                    debug!(namespace = %ns, provider = %provider_id, "registering provider for namespace")
                }
            }
        }
        state.by_provider.insert(provider_key, namespaces);
    }

    /// Remove all namespaces owned by one specific provider registration.
    ///
    /// A namespace is removed from the lookup index only while it still points
    /// at this registration; namespaces taken over by a newer registration are
    /// left alone even when it uses the same provider ID.
    pub fn deregister(&self, provider_id: &str, registration_token: RegistrationToken) {
        let mut state = self.inner.write();
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
                debug!(namespace = %ns, provider = %provider_id, "deregistered namespace provider");
            }
        }
    }

    /// Look up the provider for a namespace.
    pub fn get(&self, namespace: &str) -> Option<ProviderHandle> {
        self.inner.read().by_namespace.get(namespace).cloned()
    }

    /// Return `true` if a live provider exists for the namespace.
    pub fn has_live_provider(&self, namespace: &str) -> bool {
        self.inner
            .read()
            .by_namespace
            .get(namespace)
            .map(|h| h.is_alive())
            .unwrap_or(false)
    }
}
