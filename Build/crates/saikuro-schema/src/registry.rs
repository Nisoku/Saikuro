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
//!
//! All state lives in a single `RwLock` from `saikuro-core::sync` so that the
//! mode check and the mutations it guards are atomic (a registered namespace
//! can never be half-applied against a changing mode).  The lock is held only
//! for short map operations and never across an `await`.  Keys are ordered
//! `BTreeMap`s for deterministic iteration on both host and MCU targets.

use alloc::{borrow::ToOwned, collections::BTreeMap, string::String, sync::Arc, vec::Vec};
use saikuro_core::schema::{
    FunctionSchema, NamespaceSchema, Schema, TypeDefinition, SCHEMA_NAMESPACES_CAPACITY,
    SCHEMA_TYPES_CAPACITY,
};
use saikuro_core::sync::RwLock;
use saikuro_core::RegistrationToken;
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
    /// Identity of this specific provider registration.
    pub registration_token: RegistrationToken,
}

//  Registry

/// All registry state, guarded as a unit by [`SchemaRegistry`]'s lock.
struct Schemata {
    /// Per-namespace schemas and their owning provider ID.
    namespaces: BTreeMap<String, NamespaceEntry>,
    /// Shared type library merged from all registered schemas.
    types: BTreeMap<String, TypeDefinition>,
    /// Mode controlling whether dynamic updates are allowed.
    mode: RegistryMode,
}

#[derive(Debug, Clone)]
struct NamespaceEntry {
    schema: NamespaceSchema,
    provider_id: String,
    registration_token: RegistrationToken,
}

/// The live schema registry.
///
/// Reads are shared-lock `BTreeMap` lookups; writes (registrations, merges)
/// go through the exclusive lock and are infrequent.
#[derive(Clone)]
pub struct SchemaRegistry {
    inner: Arc<RwLock<Schemata>>,
}

impl SchemaRegistry {
    /// Create a new registry in development mode.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(Schemata {
                namespaces: BTreeMap::new(),
                types: BTreeMap::new(),
                mode: RegistryMode::Development,
            })),
        }
    }

    /// Create a registry pre-loaded from a full [`Schema`] document and
    /// immediately frozen into production mode.
    pub fn from_frozen_schema(schema: Schema) -> Self {
        let mut schemata = Schemata {
            namespaces: BTreeMap::new(),
            types: BTreeMap::new(),
            mode: RegistryMode::Production,
        };
        let frozen_token = RegistrationToken::new();
        for (ns_name, ns_schema) in (*schema.namespaces).into_iter() {
            schemata.namespaces.insert(
                ns_name,
                NamespaceEntry {
                    schema: ns_schema,
                    provider_id: "frozen".to_owned(),
                    registration_token: frozen_token,
                },
            );
        }
        for (type_name, type_def) in (*schema.types).into_iter() {
            schemata.types.insert(type_name, type_def);
        }
        info!(
            "schema registry frozen with {} namespace(s)",
            schemata.namespaces.len()
        );
        Self {
            inner: Arc::new(RwLock::new(schemata)),
        }
    }

    /// Register (or replace) a namespace.
    ///
    /// In production mode this returns an error rather than mutating state.
    pub fn register(&self, registration: NamespaceRegistration) -> Result<(), RegistryError> {
        let mut schemata = self.inner.write();
        if schemata.mode == RegistryMode::Production {
            return Err(RegistryError::FrozenSchema(registration.namespace));
        }

        let ns = registration.namespace.clone();
        if !schemata.namespaces.contains_key(&ns)
            && schemata.namespaces.len() == SCHEMA_NAMESPACES_CAPACITY
        {
            return Err(RegistryError::SchemaCapacity);
        }
        if schemata.namespaces.contains_key(&ns) {
            warn!(namespace = %ns, "overwriting existing namespace schema");
        } else {
            debug!(namespace = %ns, provider = %registration.provider_id, "registering namespace");
        }

        schemata.namespaces.insert(
            ns,
            NamespaceEntry {
                schema: registration.schema,
                provider_id: registration.provider_id,
                registration_token: registration.registration_token,
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
        self.merge_schema_with_token(schema, provider_id, RegistrationToken::new())
    }

    /// Merge a schema document under an existing provider registration.
    pub fn merge_schema_with_token(
        &self,
        schema: Schema,
        provider_id: impl Into<String>,
        registration_token: RegistrationToken,
    ) -> Result<(), RegistryError> {
        let provider_id = provider_id.into();

        // The whole merge happens under one write guard so a concurrent
        // `freeze()` cannot interleave between the type and namespace phases.
        let mut schemata = self.inner.write();

        if schemata.mode == RegistryMode::Production {
            let ns = schema.namespaces.keys().next().cloned().unwrap_or_default();
            return Err(RegistryError::FrozenSchema(ns));
        }

        let new_namespaces = schema
            .namespaces
            .keys()
            .filter(|name| !schemata.namespaces.contains_key(*name))
            .count();
        let new_types = schema
            .types
            .keys()
            .filter(|name| !schemata.types.contains_key(*name))
            .count();
        if schemata.namespaces.len() + new_namespaces > SCHEMA_NAMESPACES_CAPACITY
            || schemata.types.len() + new_types > SCHEMA_TYPES_CAPACITY
        {
            return Err(RegistryError::SchemaCapacity);
        }

        // Merge types first (functions may reference them).
        for (name, typedef) in (*schema.types).into_iter() {
            schemata.types.insert(name, typedef);
        }
        for (ns_name, ns_schema) in (*schema.namespaces).into_iter() {
            let ns = ns_name.clone();
            if schemata.namespaces.contains_key(&ns) {
                warn!(namespace = %ns, "overwriting existing namespace schema");
            } else {
                debug!(namespace = %ns, provider = %provider_id, "registering namespace");
            }
            schemata.namespaces.insert(
                ns,
                NamespaceEntry {
                    schema: ns_schema,
                    provider_id: provider_id.clone(),
                    registration_token,
                },
            );
        }
        Ok(())
    }

    /// Remove namespaces owned by one specific provider registration.
    ///
    /// A stale disconnect cannot remove schemas from a newer registration that
    /// reused the same provider ID.
    pub fn deregister_provider(&self, provider_id: &str, registration_token: RegistrationToken) {
        let mut schemata = self.inner.write();
        if schemata.mode == RegistryMode::Production {
            return;
        }
        schemata.namespaces.retain(|_ns, entry| {
            let keep =
                entry.provider_id != provider_id || entry.registration_token != registration_token;
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

        let schemata = self.inner.read();
        let entry = schemata
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
        self.inner
            .read()
            .namespaces
            .get(namespace)
            .map(|e| e.provider_id.clone())
    }

    /// Return `true` if the given namespace is registered.
    pub fn has_namespace(&self, namespace: &str) -> bool {
        self.inner.read().namespaces.contains_key(namespace)
    }

    /// Return all registered namespace names (in key order).
    pub fn namespace_names(&self) -> Vec<String> {
        self.inner.read().namespaces.keys().cloned().collect()
    }

    /// Export a snapshot of the full schema at this instant.
    ///
    /// The registry stores its maps in unbounded `BTreeMap`s while the
    /// exported [`Schema`] uses fixed-capacity heapless maps, so a registry
    /// larger than the schema's capacity fails rather than truncating the
    /// snapshot silently.
    pub fn snapshot(&self) -> Result<Schema, RegistryError> {
        let mut schema = Schema::new();
        let schemata = self.inner.read();
        for (name, entry) in schemata.namespaces.iter() {
            schema
                .namespaces
                .insert(name.clone(), entry.schema.clone())
                .map_err(|_| RegistryError::SchemaCapacity)?;
        }
        for (name, type_def) in schemata.types.iter() {
            schema
                .types
                .insert(name.clone(), type_def.clone())
                .map_err(|_| RegistryError::SchemaCapacity)?;
        }
        Ok(schema)
    }

    /// Freeze the registry, preventing any further schema changes.
    pub fn freeze(&self) {
        self.inner.write().mode = RegistryMode::Production;
        info!("schema registry frozen");
    }

    /// Return the current operating mode.
    pub fn mode(&self) -> RegistryMode {
        self.inner.read().mode
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

    #[error("schema registry capacity exceeded")]
    SchemaCapacity,
}

//  Helpers

/// Split a `"namespace.function"` target into its two components.
fn split_target(target: &str) -> Result<(&str, &str), RegistryError> {
    saikuro_core::split_target(target)
        .ok_or_else(|| RegistryError::MalformedTarget(target.to_owned()))
}
