//! Ergonomic builder types for constructing [`Schema`] values

use alloc::boxed::Box;
#[cfg(not(feature = "std"))]
use alloc::collections::BTreeMap as HashMap;
use alloc::string::String;
use alloc::vec::Vec;
#[cfg(feature = "std")]
use std::collections::HashMap;

use saikuro_event::Result;

pub use saikuro_core::schema::{TypeDescriptor, Visibility};

/// A simplified function schema used when registering functions.
#[derive(Debug, Clone, Default)]
pub struct FunctionSchema {
    /// Human-readable description.
    pub doc: Option<String>,
    /// Whether this function is safe to retry (no side effects, or idempotent ones).
    pub idempotent: bool,
    /// Capabilities required to invoke this function (as string tokens).
    pub capabilities: Vec<String>,
    /// Argument descriptors. Omit for untyped any-args.
    pub args: Vec<ArgDescriptor>,
    /// Return type. Defaults to `any` when `None`.
    pub returns: Option<TypeDescriptor>,
    /// Visibility. Defaults to `public`.
    pub visibility: Visibility,
}

/// A single argument descriptor for a function.
#[derive(Debug, Clone)]
pub struct ArgDescriptor {
    /// Parameter name.
    pub name: String,
    /// The type this argument must conform to.
    pub r#type: TypeDescriptor,
    /// If `true` this argument may be omitted by the caller.
    pub optional: bool,
    /// Human-readable documentation.
    pub doc: Option<String>,
}

/// A namespace schema built up by a [`Provider`](saikuro_client::Provider).
#[derive(Debug, Default, Clone)]
pub struct NamespaceSchema {
    /// Namespace-level doc string.
    pub doc: Option<String>,
    pub(crate) functions: HashMap<String, FunctionSchema>,
}

impl NamespaceSchema {
    /// Create an empty namespace schema.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a function under the given name.
    pub fn insert(&mut self, name: impl Into<String>, schema: FunctionSchema) {
        self.functions.insert(name.into(), schema);
    }

    /// Convert to the core [`NamespaceSchema`](saikuro_core::schema::NamespaceSchema).
    pub fn to_core(&self) -> Result<saikuro_core::schema::NamespaceSchema> {
        let mut functions = saikuro_core::schema::FunctionMap::new();
        for (name, fs) in &self.functions {
            let args: Vec<saikuro_core::schema::ArgumentDescriptor> = fs
                .args
                .iter()
                .map(|a| saikuro_core::schema::ArgumentDescriptor {
                    name: a.name.clone(),
                    r#type: a.r#type.clone(),
                    optional: a.optional,
                    doc: a.doc.clone(),
                    default: None,
                })
                .collect();

            let core_fn = saikuro_core::schema::FunctionSchema {
                args,
                returns: fs.returns.clone().unwrap_or(TypeDescriptor::Primitive {
                    r#type: saikuro_core::schema::PrimitiveType::Any,
                }),
                visibility: fs.visibility,
                capabilities: fs
                    .capabilities
                    .iter()
                    .map(|s| saikuro_core::CapabilityToken::from(s.as_str()))
                    .collect(),
                idempotent: fs.idempotent,
                doc: fs.doc.clone(),
            };
            functions.insert(name.clone(), core_fn);
        }

        Ok(saikuro_core::schema::NamespaceSchema {
            functions: Box::new(functions),
            doc: self.doc.clone(),
        })
    }
}

/// Build a full [`Schema`](saikuro_core::schema::Schema) from the given
/// namespace map.
pub fn build_schema(
    namespaces: &HashMap<String, NamespaceSchema>,
) -> Result<saikuro_core::schema::Schema> {
    let mut schema = saikuro_core::schema::Schema::new();
    for (ns_name, ns) in namespaces {
        schema.namespaces.insert(ns_name.clone(), ns.to_core()?);
    }
    Ok(schema)
}
