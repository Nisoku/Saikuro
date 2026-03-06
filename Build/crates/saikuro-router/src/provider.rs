//! Provider abstraction and registry.
//!
//! A **provider** is any entity that can handle invocations for one or more
//! namespaces.  In practice it is a connected language adapter (Python,
//! TypeScript, …) that has registered its schema and is listening for work.
//!
//! The [`ProviderRegistry`] maps namespace names to [`ProviderHandle`]s.
//! Each handle wraps a tokio MPSC sender so the router can dispatch work
//! without blocking.

use async_trait::async_trait;
use dashmap::DashMap;
use saikuro_core::{envelope::Envelope, ResponseEnvelope};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
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
    namespaces: Vec<String>,
    sender: mpsc::Sender<ProviderWorkItem>,
}

impl ProviderHandle {
    pub fn new(
        id: impl Into<String>,
        namespaces: Vec<String>,
        sender: mpsc::Sender<ProviderWorkItem>,
    ) -> Self {
        Self {
            id: id.into(),
            namespaces,
            sender,
        }
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
    /// namespace -> provider handle
    by_namespace: Arc<DashMap<String, ProviderHandle>>,
    /// provider_id -> list of namespaces (for cleanup on disconnect)
    by_provider: Arc<DashMap<String, Vec<String>>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a provider handle for the given namespaces.
    ///
    /// If a namespace already has a provider, the old one is replaced and a
    /// warning is emitted.
    pub fn register(&self, handle: ProviderHandle) {
        let provider_id = handle.id().to_owned();
        let namespaces = handle.namespaces().to_vec();

        for ns in &namespaces {
            if self.by_namespace.contains_key(ns.as_str()) {
                warn!(namespace = %ns, provider = %provider_id, "replacing existing namespace provider");
            } else {
                debug!(namespace = %ns, provider = %provider_id, "registering provider for namespace");
            }
            self.by_namespace.insert(ns.clone(), handle.clone());
        }

        self.by_provider.insert(provider_id, namespaces);
    }

    /// Remove all namespace registrations for the given provider ID.
    pub fn deregister(&self, provider_id: &str) {
        if let Some((_, namespaces)) = self.by_provider.remove(provider_id) {
            for ns in namespaces {
                self.by_namespace.remove(&ns);
                debug!(namespace = %ns, provider = %provider_id, "deregistered namespace provider");
            }
        }
    }

    /// Look up the provider for a namespace.
    pub fn get(&self, namespace: &str) -> Option<ProviderHandle> {
        self.by_namespace.get(namespace).map(|r| r.clone())
    }

    /// Return `true` if a live provider exists for the namespace.
    pub fn has_live_provider(&self, namespace: &str) -> bool {
        self.by_namespace
            .get(namespace)
            .map(|h| h.is_alive())
            .unwrap_or(false)
    }
}
