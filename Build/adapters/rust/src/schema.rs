//! Schema builder types for the Saikuro Rust adapter.
//!
//! Used by [`Provider`](crate::Provider) to construct the schema announcement
//! envelope that it sends to the runtime when it first connects.

#[cfg(not(feature = "std"))]
use alloc::collections::BTreeMap as HashMap;
#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::collections::HashMap;

use crate::error::{Error, Result};
use saikuro_core::schema::{
    ArgumentDescriptor, FunctionSchema as CoreFunctionSchema,
    NamespaceSchema as CoreNamespaceSchema, PrimitiveType, Schema, TypeDescriptor, Visibility,
};

/// A simplified function schema used when registering with [`Provider`](crate::Provider).
#[derive(Debug, Clone, Default)]
pub struct FunctionSchema {
    /// Human-readable description.
    pub doc: Option<String>,
    /// Whether this function is safe to retry (no side effects, or idempotent ones).
    pub idempotent: bool,
    /// Capabilities required to invoke this function.
    pub capabilities: Vec<String>,
    /// Argument descriptors (optional; omit for untyped any-args).
    pub args: Vec<ArgDescriptor>,
    /// Return type (optional; defaults to `any`).
    pub returns: Option<TypeDescriptor>,
    /// Visibility. Defaults to `public`.
    pub visibility: Visibility,
}

/// A single argument descriptor.
#[derive(Debug, Clone)]
pub struct ArgDescriptor {
    pub name: String,
    pub r#type: TypeDescriptor,
    pub optional: bool,
    pub doc: Option<String>,
}

/// A namespace schema, built up by a [`Provider`](crate::Provider).
#[derive(Debug, Default, Clone)]
pub struct NamespaceSchema {
    /// Namespace-level doc string.
    pub doc: Option<String>,
    pub(crate) functions: HashMap<String, FunctionSchema>,
}

impl NamespaceSchema {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a function schema.
    pub fn insert(&mut self, name: impl Into<String>, schema: FunctionSchema) {
        self.functions.insert(name.into(), schema);
    }

    /// Convert to the core `NamespaceSchema` for announcement.
    ///
    /// Fails when the function count exceeds the core schema's fixed map
    /// capacity, so a provider never announces a silently truncated namespace.
    pub fn to_core(&self) -> Result<CoreNamespaceSchema> {
        let mut functions = saikuro_core::schema::FunctionMap::new();
        for (name, fs) in &self.functions {
            let args: Vec<ArgumentDescriptor> = fs
                .args
                .iter()
                .map(|a| ArgumentDescriptor {
                    name: a.name.clone(),
                    r#type: a.r#type.clone(),
                    optional: a.optional,
                    doc: a.doc.clone(),
                    default: None,
                })
                .collect();

            let core_fn = CoreFunctionSchema {
                args,
                returns: fs.returns.clone().unwrap_or(TypeDescriptor::Primitive {
                    r#type: PrimitiveType::Any,
                }),
                visibility: fs.visibility,
                capabilities: fs
                    .capabilities
                    .iter()
                    .map(|s| saikuro_core::capability::CapabilityToken::from(s.as_str()))
                    .collect(),
                idempotent: fs.idempotent,
                doc: fs.doc.clone(),
            };
            functions
                .insert(name.clone(), core_fn)
                .map_err(|_| Error::SchemaCapacityExceeded)?;
        }

        Ok(CoreNamespaceSchema {
            functions: Box::new(functions),
            doc: self.doc.clone(),
        })
    }
}

/// Build a full [`Schema`] from the given namespaces.
///
/// Fails when the namespace count exceeds the core schema's fixed map
/// capacity, so a provider never announces a silently truncated schema.
///
/// Internal helper exposed for the crate's integration tests.
#[doc(hidden)]
pub fn build_schema(namespaces: &HashMap<String, NamespaceSchema>) -> Result<Schema> {
    let mut schema = Schema::new();
    for (ns_name, ns) in namespaces {
        schema
            .namespaces
            .insert(ns_name.clone(), ns.to_core()?)
            .map_err(|_| Error::SchemaCapacityExceeded)?;
    }
    Ok(schema)
}
