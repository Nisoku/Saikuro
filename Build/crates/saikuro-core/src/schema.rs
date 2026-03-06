//! Schema definition types.
//!
//! These are the *data* types that describe the contract between providers
//! and callers.  They are kept in `saikuro-core` so that any crate in the
//! workspace can read schemas without depending on the heavier validation
//! and registry machinery in `saikuro-schema`.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

use crate::capability::CapabilityToken;

/// The protocol version this schema was compiled against.
pub const SCHEMA_VERSION: u32 = 1;

//  Primitive types

/// A scalar type name used in function argument and return-type declarations.
///
/// Extended types (user-defined structs) are represented as `TypeRef`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrimitiveType {
    Bool,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    String,
    Bytes,
    /// Dynamic / untyped: the runtime will pass the value through without
    /// checking its shape.  Use sparingly.
    Any,
    /// The function returns nothing (or the caller doesn't care about the value).
    Unit,
}

impl std::fmt::Display for PrimitiveType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Bool => "bool",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::String => "string",
            Self::Bytes => "bytes",
            Self::Any => "any",
            Self::Unit => "unit",
        };
        f.write_str(s)
    }
}

//  Type descriptors

/// A type descriptor that can appear anywhere a type is needed in the schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TypeDescriptor {
    /// A built-in scalar type.
    Primitive { r#type: PrimitiveType },
    /// A reference to a named type defined in `Schema::types`.
    Named { name: String },
    /// An optional wrapper (the value may be absent).
    Option { inner: Box<TypeDescriptor> },
    /// A variable-length list of items of the same type.
    Array { item: Box<TypeDescriptor> },
    /// A string-keyed map of items of the same value type.
    Map { value: Box<TypeDescriptor> },
    /// A stream of items produced by a server (used for stream invocations).
    Stream { item: Box<TypeDescriptor> },
    /// A bidirectional channel type (used for channel invocations).
    Channel {
        inbound: Box<TypeDescriptor>,
        outbound: Box<TypeDescriptor>,
    },
}

impl TypeDescriptor {
    /// Shorthand for a simple primitive.
    pub fn primitive(t: PrimitiveType) -> Self {
        Self::Primitive { r#type: t }
    }

    /// Shorthand for a named reference.
    pub fn named(name: impl Into<String>) -> Self {
        Self::Named { name: name.into() }
    }

    /// Wrap this descriptor in `Option`.
    pub fn optional(self) -> Self {
        Self::Option {
            inner: Box::new(self),
        }
    }

    /// Wrap this descriptor in `Array`.
    pub fn array_of(self) -> Self {
        Self::Array {
            item: Box::new(self),
        }
    }
}

//  Function schema

/// Visibility of a function to external callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    /// Callable by any peer that has the required capabilities.
    #[default]
    Public,
    /// Callable only by peers in the same cluster/process group.
    Internal,
    /// Not exposed at all; exists only for documentation purposes.
    Private,
}

/// A named argument to a function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgumentDescriptor {
    /// Parameter name (for documentation and named-argument calling style).
    pub name: String,
    /// The type this argument must conform to.
    pub r#type: TypeDescriptor,
    /// If `true` this argument may be omitted by the caller.
    #[serde(default)]
    pub optional: bool,
    /// Default value used when the argument is omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<crate::value::Value>,
    /// Human-readable documentation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

/// Schema description of a single callable function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionSchema {
    /// Ordered argument descriptors.
    #[serde(default)]
    pub args: Vec<ArgumentDescriptor>,

    /// Return type.
    #[serde(default = "default_unit")]
    pub returns: TypeDescriptor,

    /// Visibility level.
    #[serde(default)]
    pub visibility: Visibility,

    /// Capability tokens the caller must hold.
    #[serde(default)]
    pub capabilities: Vec<CapabilityToken>,

    /// Whether this function supports idempotent retries.
    #[serde(default)]
    pub idempotent: bool,

    /// Human-readable documentation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

fn default_unit() -> TypeDescriptor {
    TypeDescriptor::primitive(PrimitiveType::Unit)
}

//  Type definitions

/// A named field within a user-defined record type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDescriptor {
    /// The type of this field.
    pub r#type: TypeDescriptor,
    /// Whether this field may be absent.
    #[serde(default)]
    pub optional: bool,
    /// Human-readable documentation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

/// Variants for user-defined types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TypeDefinition {
    /// A product type (named fields).
    Record {
        fields: BTreeMap<String, FieldDescriptor>,
    },
    /// A sum type (tagged union of named variants).
    Enum { variants: Vec<String> },
    /// A newtype wrapper around another type.
    Alias { inner: TypeDescriptor },
}

//  Namespace schema

/// Schema for a single namespace: a logical grouping of related functions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceSchema {
    /// All functions exposed by this namespace.
    pub functions: HashMap<String, FunctionSchema>,
    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

//  Top-level schema

/// The root schema document: a versioned description of all namespaces and
/// types available in a Saikuro deployment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    /// Must equal [`SCHEMA_VERSION`].
    pub version: u32,

    /// All registered namespaces, keyed by namespace name.
    pub namespaces: HashMap<String, NamespaceSchema>,

    /// User-defined types, keyed by type name.
    #[serde(default)]
    pub types: HashMap<String, TypeDefinition>,
}

impl Schema {
    /// Create a new, empty schema at the current version.
    pub fn new() -> Self {
        Self {
            version: SCHEMA_VERSION,
            namespaces: HashMap::new(),
            types: HashMap::new(),
        }
    }

    /// Look up a function descriptor given a fully-qualified target string.
    ///
    /// Returns `None` if either the namespace or the function does not exist.
    pub fn lookup_function(&self, target: &str) -> Option<&FunctionSchema> {
        let dot = target.rfind('.')?;
        let ns = &target[..dot];
        let func = &target[dot + 1..];
        self.namespaces.get(ns)?.functions.get(func)
    }

    /// Return `true` if a namespace with the given name is registered.
    pub fn has_namespace(&self, name: &str) -> bool {
        self.namespaces.contains_key(name)
    }

    /// Deserialise a schema from JSON (used in tests and tooling).
    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }

    /// Serialise this schema to pretty-printed JSON.
    pub fn to_json_pretty(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }
}

impl Default for Schema {
    fn default() -> Self {
        Self::new()
    }
}
