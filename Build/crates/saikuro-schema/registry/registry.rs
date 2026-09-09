use alloc::{borrow::ToOwned, boxed::Box, collections::BTreeMap, string::String, vec::Vec};
#[cfg(not(target_has_atomic = "ptr"))]
use portable_atomic_util::Arc;
use saikuro_core::schema::{
    FunctionSchema, NamespaceSchema, Schema, TypeDefinition, SCHEMA_NAMESPACES_CAPACITY,
    SCHEMA_TYPES_CAPACITY,
};
#[cfg(target_has_atomic = "ptr")]
use saikuro_core::Arc;
use saikuro_core::RegistrationToken;
use saikuro_exec::sync::RwLock;

use saikuro_event::SaikuroError;

/// Whether the registry accepts dynamic schema updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryMode {
    /// Development: providers can register/update schemas at runtime.
    Development,
    /// Production: the schema is frozen at startup; updates are rejected.
    Production,
}

// Registration descriptor
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

// Registry
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
    /// Function schemas stored behind `Arc` so [`SchemaRegistry::lookup_function`]
    /// can hand out function references without deep-cloning the schema.
    functions: BTreeMap<String, Arc<FunctionSchema>>,
    doc: Option<String>,
    provider_id: String,
    registration_token: RegistrationToken,
}

/// The live schema registry.
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
            let (functions, doc) = into_arc_functions(ns_schema);
            schemata.namespaces.insert(
                ns_name,
                NamespaceEntry {
                    functions,
                    doc,
                    provider_id: "frozen".to_owned(),
                    registration_token: frozen_token,
                },
            );
        }
        for (type_name, type_def) in (*schema.types).into_iter() {
            schemata.types.insert(type_name, type_def);
        }
        Self {
            inner: Arc::new(RwLock::new(schemata)),
        }
    }

    /// Register (or replace) a namespace.
    /// In production mode this returns an error rather than mutating state.
    pub async fn register(&self, registration: NamespaceRegistration) -> Result<(), SaikuroError> {
        let mut schemata = self.inner.write().await;
        if schemata.mode == RegistryMode::Production {
            return Err(SaikuroError::FrozenSchema(registration.namespace));
        }

        let ns = registration.namespace.clone();
        if !schemata.namespaces.contains_key(&ns)
            && schemata.namespaces.len() == SCHEMA_NAMESPACES_CAPACITY
        {
            return Err(SaikuroError::SchemaCapacity);
        }
        let (functions, doc) = into_arc_functions(registration.schema);
        schemata.namespaces.insert(
            ns,
            NamespaceEntry {
                functions,
                doc,
                provider_id: registration.provider_id,
                registration_token: registration.registration_token,
            },
        );
        Ok(())
    }

    /// Merge an entire [`Schema`] document into the registry.
    pub async fn merge_schema(
        &self,
        schema: Schema,
        provider_id: impl Into<String>,
    ) -> Result<(), SaikuroError> {
        self.merge_schema_with_token(schema, provider_id, RegistrationToken::new())
            .await
    }

    /// Merge a schema document under an existing provider registration.
    pub async fn merge_schema_with_token(
        &self,
        schema: Schema,
        provider_id: impl Into<String>,
        registration_token: RegistrationToken,
    ) -> Result<(), SaikuroError> {
        let provider_id = provider_id.into();

        // The whole merge happens under one write guard so a concurrent
        // `freeze()` cannot interleave between the type and namespace phases.
        let mut schemata = self.inner.write().await;

        if schemata.mode == RegistryMode::Production {
            let ns = schema.namespaces.keys().next().cloned().unwrap_or_default();
            return Err(SaikuroError::FrozenSchema(ns));
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
            return Err(SaikuroError::SchemaCapacity);
        }

        // Merge types first (functions may reference them).
        for (name, typedef) in (*schema.types).into_iter() {
            schemata.types.insert(name, typedef);
        }
        for (ns_name, ns_schema) in (*schema.namespaces).into_iter() {
            let ns = ns_name.clone();
            let (functions, doc) = into_arc_functions(ns_schema);
            schemata.namespaces.insert(
                ns,
                NamespaceEntry {
                    functions,
                    doc,
                    provider_id: provider_id.clone(),
                    registration_token,
                },
            );
        }
        Ok(())
    }

    /// Remove namespaces owned by one specific provider registration.
    pub async fn deregister_provider(
        &self,
        provider_id: &str,
        registration_token: RegistrationToken,
    ) {
        let mut schemata = self.inner.write().await;
        if schemata.mode == RegistryMode::Production {
            return;
        }
        schemata.namespaces.retain(|_ns, entry| {
            entry.provider_id != provider_id || entry.registration_token != registration_token
        });
    }

    /// Look up the schema for a single function.
    /// `target` must be in `"namespace.function"` format.
    pub async fn lookup_function(&self, target: &str) -> Result<FunctionRef, SaikuroError> {
        let (ns_name, fn_name) = split_target(target)?;

        let schemata = self.inner.read().await;
        let entry = schemata
            .namespaces
            .get(ns_name)
            .ok_or_else(|| SaikuroError::NamespaceNotFound(ns_name.to_owned()))?;

        let fn_schema = entry
            .functions
            .get(fn_name)
            .ok_or_else(|| SaikuroError::FunctionNotFound(target.to_owned()))?
            .clone();

        Ok(FunctionRef {
            namespace: ns_name.to_owned(),
            function: fn_name.to_owned(),
            schema: fn_schema,
            provider_id: entry.provider_id.clone(),
        })
    }

    /// Return the provider ID for the given namespace.
    pub async fn provider_for_namespace(&self, namespace: &str) -> Option<String> {
        self.inner
            .read()
            .await
            .namespaces
            .get(namespace)
            .map(|e| e.provider_id.clone())
    }

    /// Return `true` if the given namespace is registered.
    pub async fn has_namespace(&self, namespace: &str) -> bool {
        self.inner.read().await.namespaces.contains_key(namespace)
    }

    /// Return all registered namespace names (in key order).
    pub async fn namespace_names(&self) -> Vec<String> {
        self.inner.read().await.namespaces.keys().cloned().collect()
    }

    /// Export a snapshot of the full schema at this instant.
    pub async fn snapshot(&self) -> Result<Schema, SaikuroError> {
        let schemata = self.inner.read().await;
        check_capacity(&schemata)?;
        collect_snapshot(&schemata, |_, _, _| true)
    }

    /// Export a schema containing only the function schemas that satisfy
    /// `keep(-> namespace, -> function, -> schema)`, plus all shared type
    /// definitions.
    pub async fn snapshot_filtered(
        &self,
        keep: impl Fn(&str, &str, &FunctionSchema) -> bool,
    ) -> Result<Schema, SaikuroError> {
        let schemata = self.inner.read().await;
        check_capacity(&schemata)?;
        collect_snapshot(&schemata, keep)
    }

    /// Freeze the registry, preventing any further schema changes.
    pub async fn freeze(&self) {
        self.inner.write().await.mode = RegistryMode::Production;
    }

    /// Return the current operating mode.
    pub async fn mode(&self) -> RegistryMode {
        self.inner.read().await.mode
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// A fully-resolved reference to a function schema plus its owning provider.
#[derive(Debug, Clone)]
pub struct FunctionRef {
    /// Namespace that owns the function.
    pub namespace: String,
    /// Function name within the namespace.
    pub function: String,
    /// Resolved function schema, shared with the registry (O(1) clone).
    pub schema: Arc<FunctionSchema>,
    /// Provider that registered the namespace.
    pub provider_id: String,
}

/// Reject the operation when the registry has grown past its fixed capacities.
fn check_capacity(schemata: &Schemata) -> Result<(), SaikuroError> {
    if schemata.namespaces.len() > SCHEMA_NAMESPACES_CAPACITY
        || schemata.types.len() > SCHEMA_TYPES_CAPACITY
    {
        Err(SaikuroError::SchemaCapacity)
    } else {
        Ok(())
    }
}

/// Deep-clone the registry into a plain [`Schema`], keeping only the function
/// schemas accepted by `keep` (called with namespace, function name, schema).
/// Namespaces that end up with no kept functions are omitted.
fn collect_snapshot(
    schemata: &Schemata,
    keep: impl Fn(&str, &str, &FunctionSchema) -> bool,
) -> Result<Schema, SaikuroError> {
    let mut schema = Schema::new();
    for (ns_name, entry) in schemata.namespaces.iter() {
        let mut functions = BTreeMap::new();
        for (fn_name, fn_schema) in entry.functions.iter() {
            if !keep(ns_name, fn_name, fn_schema) {
                continue;
            }
            functions.insert(fn_name.clone(), (**fn_schema).clone());
        }
        if functions.is_empty() {
            continue;
        }
        schema.namespaces.insert(
            ns_name.clone(),
            saikuro_core::schema::NamespaceSchema {
                functions: Box::new(functions),
                doc: entry.doc.clone(),
            },
        );
    }
    for (name, type_def) in schemata.types.iter() {
        schema.types.insert(name.clone(), type_def.clone());
    }
    Ok(schema)
}

/// Split a `"namespace.function"` target into its two components.
fn split_target(target: &str) -> Result<(&str, &str), SaikuroError> {
    saikuro_core::split_target(target)
        .ok_or_else(|| SaikuroError::MalformedTarget(target.to_owned()))
}

/// Move a namespace schema into reference-counted function entries
fn into_arc_functions(
    ns: NamespaceSchema,
) -> (BTreeMap<String, Arc<FunctionSchema>>, Option<String>) {
    let functions = ns
        .functions
        .into_iter()
        .map(|(name, schema)| (name, Arc::new(schema)))
        .collect();
    (functions, ns.doc)
}
