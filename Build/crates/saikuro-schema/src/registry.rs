//! Schema registry:  the live, thread-safe store of all namespace schemas.
//!
//! The registry is the single source of truth for "what functions exist and
//! how are they typed?".  It is shared (via `Arc` (not the browser)) across the runtime's
//! components and updated atomically when new providers register or schemas
//! are hot-reloaded.
//!
//! In **development mode** providers announce their schemas at connection time
//! and the registry merges them in.  In **production mode** schemas are loaded
//! from a frozen file at startup and providers cannot alter them.

use dashmap::DashMap;
use parking_lot::RwLock;
use saikuro_core::schema::{FunctionSchema, NamespaceSchema, Schema};
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::validator::ValidationError;

//  Modes

/// Whether the registry accepts dynamic schema updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryMode {
    /// Development: providers can register/update schemas at runtime.
    Development,
    /// Production: the schema is frozen at startup; updates are rejected.
    Production,
}

//  Registration descriptor

/// All information a provider submits when it registers a namespace.
#[derive(Debug, Clone)]
pub struct NamespaceRegistration {
    /// The namespace name (e.g. `"math"`, `"events"`).
    pub namespace: String,
    /// The schema for this namespace.
    pub schema: NamespaceSchema,
    /// Opaque identifier for the provider connection (used for routing).
    pub provider_id: String,
}

//  Registry

/// The live schema registry.
///
/// All lookups are lock-free reads via `DashMap`.  Writes (registrations,
/// merges) are infrequent and go through a coarser `RwLock` that guards the
/// mode and global schema snapshot.
#[derive(Clone)]
pub struct SchemaRegistry {
    /// Per-namespace schemas and their owning provider ID.
    namespaces: Arc<DashMap<String, NamespaceEntry>>,
    /// Shared type library merged from all registered schemas.
    types: Arc<DashMap<String, saikuro_core::schema::TypeDefinition>>,
    /// Mode controlling whether dynamic updates are allowed.
    mode: Arc<RwLock<RegistryMode>>,
}

#[derive(Debug, Clone)]
struct NamespaceEntry {
    schema: NamespaceSchema,
    provider_id: String,
}

impl SchemaRegistry {
    /// Create a new registry in development mode.
    pub fn new() -> Self {
        Self {
            namespaces: Arc::new(DashMap::new()),
            types: Arc::new(DashMap::new()),
            mode: Arc::new(RwLock::new(RegistryMode::Development)),
        }
    }

    /// Create a registry pre-loaded from a full [`Schema`] document and
    /// immediately frozen into production mode.
    pub fn from_frozen_schema(schema: Schema) -> Self {
        let registry = Self::new();
        for (ns_name, ns_schema) in schema.namespaces {
            registry.namespaces.insert(
                ns_name.clone(),
                NamespaceEntry {
                    schema: ns_schema,
                    provider_id: "frozen".to_owned(),
                },
            );
        }
        for (type_name, type_def) in schema.types {
            registry.types.insert(type_name, type_def);
        }
        *registry.mode.write() = RegistryMode::Production;
        info!(
            "schema registry frozen with {} namespace(s)",
            registry.namespaces.len()
        );
        registry
    }

    /// Register (or replace) a namespace.
    ///
    /// In production mode this returns an error rather than mutating state.
    pub fn register(&self, registration: NamespaceRegistration) -> Result<(), RegistryError> {
        if *self.mode.read() == RegistryMode::Production {
            return Err(RegistryError::FrozenSchema(registration.namespace));
        }

        let ns = registration.namespace.clone();
        if self.namespaces.contains_key(&ns) {
            warn!(namespace = %ns, "overwriting existing namespace schema");
        } else {
            debug!(namespace = %ns, provider = %registration.provider_id, "registering namespace");
        }

        self.namespaces.insert(
            ns,
            NamespaceEntry {
                schema: registration.schema,
                provider_id: registration.provider_id,
            },
        );
        Ok(())
    }

    /// Merge an entire [`Schema`] document into the registry.
    ///
    /// Types are added to the shared type library; namespaces are registered
    /// under `provider_id`.
    pub fn merge_schema(
        &self,
        schema: Schema,
        provider_id: impl Into<String>,
    ) -> Result<(), RegistryError> {
        let provider_id = provider_id.into();
        // Merge types first (functions may reference them).
        for (name, typedef) in schema.types {
            self.types.insert(name, typedef);
        }
        for (ns_name, ns_schema) in schema.namespaces {
            self.register(NamespaceRegistration {
                namespace: ns_name,
                schema: ns_schema,
                provider_id: provider_id.clone(),
            })?;
        }
        Ok(())
    }

    /// Remove all namespaces owned by `provider_id`.
    ///
    /// Called when a provider disconnects.
    pub fn deregister_provider(&self, provider_id: &str) {
        self.namespaces.retain(|_ns, entry| {
            let keep = entry.provider_id != provider_id;
            if !keep {
                debug!(provider = %provider_id, "deregistered namespace on disconnect");
            }
            keep
        });
    }

    /// Look up the schema for a single function.
    ///
    /// `target` must be in `"namespace.function"` format.
    pub fn lookup_function(&self, target: &str) -> Result<FunctionRef, RegistryError> {
        let (ns_name, fn_name) = split_target(target)?;

        let entry = self
            .namespaces
            .get(ns_name)
            .ok_or_else(|| RegistryError::NamespaceNotFound(ns_name.to_owned()))?;

        let fn_schema = entry
            .schema
            .functions
            .get(fn_name)
            .ok_or_else(|| RegistryError::FunctionNotFound(target.to_owned()))?
            .clone();

        Ok(FunctionRef {
            namespace: ns_name.to_owned(),
            function: fn_name.to_owned(),
            schema: fn_schema,
            provider_id: entry.provider_id.clone(),
        })
    }

    /// Return the provider ID for the given namespace.
    pub fn provider_for_namespace(&self, namespace: &str) -> Option<String> {
        self.namespaces
            .get(namespace)
            .map(|e| e.provider_id.clone())
    }

    /// Return `true` if the given namespace is registered.
    pub fn has_namespace(&self, namespace: &str) -> bool {
        self.namespaces.contains_key(namespace)
    }

    /// Return all registered namespace names.
    pub fn namespace_names(&self) -> Vec<String> {
        self.namespaces.iter().map(|e| e.key().clone()).collect()
    }

    /// Export a snapshot of the full schema at this instant.
    pub fn snapshot(&self) -> Schema {
        let mut schema = Schema::new();
        for entry in self.namespaces.iter() {
            schema
                .namespaces
                .insert(entry.key().clone(), entry.value().schema.clone());
        }
        for entry in self.types.iter() {
            schema
                .types
                .insert(entry.key().clone(), entry.value().clone());
        }
        schema
    }

    /// Freeze the registry, preventing any further schema changes.
    pub fn freeze(&self) {
        *self.mode.write() = RegistryMode::Production;
        info!("schema registry frozen");
    }

    /// Return the current operating mode.
    pub fn mode(&self) -> RegistryMode {
        *self.mode.read()
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

//  Resolved reference

/// A fully-resolved reference to a function schema plus its owning provider.
#[derive(Debug, Clone)]
pub struct FunctionRef {
    pub namespace: String,
    pub function: String,
    pub schema: FunctionSchema,
    pub provider_id: String,
}

//  Registry error

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("namespace not found: {0}")]
    NamespaceNotFound(String),

    #[error("function not found: {0}")]
    FunctionNotFound(String),

    #[error("malformed target '{0}': must be 'namespace.function'")]
    MalformedTarget(String),

    #[error("schema is frozen; cannot register namespace '{0}' in production mode")]
    FrozenSchema(String),

    #[error("validation error: {0}")]
    Validation(#[from] ValidationError),
}

//  Helpers

/// Split a `"namespace.function"` target into its two components.
fn split_target(target: &str) -> Result<(&str, &str), RegistryError> {
    saikuro_core::split_target(target)
        .ok_or_else(|| RegistryError::MalformedTarget(target.to_owned()))
}
