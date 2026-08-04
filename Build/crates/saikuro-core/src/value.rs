//! Dynamic value type used across the wire.
//!
//! Saikuro carries typed arguments on the wire, but the runtime must be able
//! to handle values whose exact Rust type is not known at compile time.
//! [`Value`] is the universal representation that can model every type in the
//! Saikuro type system, round-trip through MessagePack without loss (bounded
//! by [`VALUE_MAP_CAPACITY`] for map values), and be validated against a
//! schema field descriptor.

use alloc::{borrow::ToOwned, boxed::Box, string::String, vec::Vec};
use serde::{Deserialize, Serialize};

/// Maximum number of entries a [`Value::Map`] can hold.
///
/// Saikuro's wire format is schema-driven: argument lists, error detail bags,
/// and log fields are all small by construction. This bound keeps `Value`
/// embeddable without a heap-based map. Deserialising a map larger than this
/// fails cleanly with a serde error rather than truncating.
pub const VALUE_MAP_CAPACITY: usize = 64;

/// Fixed-capacity, insertion-ordered map backing [`Value::Map`].
///
/// Insertion order is deterministic for a given construction sequence, which
/// keeps serialisation order stable for content-addressed hashing. Entries are
/// serialised in insertion order, so two semantically-equal maps built in
/// different orders are not byte-identical (and are not `PartialEq`-equal).
pub type ValueMap = heapless::FnvIndexMap<String, Value, VALUE_MAP_CAPACITY>;

/// A dynamically-typed value that can appear in an invocation argument list,
/// a return value, an error detail bag, or a schema default.
///
/// The set of variants is deliberately minimal:  it mirrors the MessagePack
/// type system so serialisation is lossless:  while still providing the
/// richness needed to express the full Saikuro type system.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(untagged)]
pub enum Value {
    /// Explicit absence of a value.
    #[default]
    Null,

    /// Boolean flag.
    Bool(bool),

    /// 64-bit signed integer. All integer wire values are widened to this.
    Int(i64),

    /// 64-bit unsigned integer, used when the value exceeds `i64::MAX`.
    UInt(u64),

    /// 64-bit IEEE-754 floating point.
    Float(f64),

    /// UTF-8 encoded text.
    String(String),

    /// Ordered sequence of values.
    ///
    /// **Must come before `Bytes`** in the enum so that during untagged
    /// deserialisation a msgpack array is matched as `Array` before the
    /// `serde_bytes`-annotated `Bytes` variant, which would otherwise
    /// greedily consume any byte-sequence (including integer arrays).
    Array(Vec<Value>),

    /// Raw binary blob (resource handles, opaque payloads, …).
    ///
    /// The `serde_bytes` annotation ensures that msgpack `bin` wire type is
    /// used instead of the default array-of-u8 encoding.  The variant is
    /// placed *after* `Array` so that an integer-element array is matched by
    /// `Array` first (correct), while a genuine `bin` blob fails the
    /// `Vec<Value>` check and falls through to this variant (also correct).
    #[serde(with = "serde_bytes")]
    Bytes(Vec<u8>),

    /// String-keyed mapping of values. A `Box<ValueMap>` breaks the recursive
    /// `Value -> ValueMap -> Value` cycle: heapless maps are stored inline, so
    /// without indirection `Value` would have infinite size. The `ValueMap` is
    /// an insertion-ordered fixed-capacity map, so serialisation order is
    /// deterministic, which makes content-addressed hashing predictable.
    Map(Box<ValueMap>),
}

/// Equality for [`Value`].
///
/// Implemented manually because the fixed-capacity map backing `Map` only
/// implements `PartialEq` when the value type is `Eq`, which `Value` cannot be
/// (it contains `f64`). Map equality is order-sensitive: two maps with the same
/// entries inserted in different orders are *not* equal, matching the byte-level
/// serialisation behaviour (see [`ValueMap`]).
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::UInt(a), Self::UInt(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a == b,
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Bytes(a), Self::Bytes(b)) => a == b,
            (Self::Array(a), Self::Array(b)) => a == b,
            (Self::Map(a), Self::Map(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .zip(b.iter())
                        .all(|((ka, va), (kb, vb))| ka == kb && va == vb)
            }
            _ => false,
        }
    }
}

impl Value {
    /// Returns `true` if this value is [`Value::Null`].
    #[inline]
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Attempt to borrow the inner `bool`. Returns `None` for other variants.
    #[inline]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Attempt to borrow the inner `i64`. `UInt` values that fit in `i64` are
    /// also narrowed. Returns `None` for other variants.
    #[inline]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(n) => Some(*n),
            Self::UInt(n) => i64::try_from(*n).ok(),
            _ => None,
        }
    }

    /// Attempt to borrow the inner `u64`. `Int` values that are non-negative
    /// are also widened. Returns `None` for other variants.
    #[inline]
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::UInt(n) => Some(*n),
            Self::Int(n) if *n >= 0 => Some(*n as u64),
            _ => None,
        }
    }

    /// Attempt to borrow the inner `f64`. Returns `None` for other variants.
    #[inline]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Float(f) => Some(*f),
            Self::Int(n) => Some(*n as f64),
            Self::UInt(n) => Some(*n as f64),
            _ => None,
        }
    }

    /// Attempt to borrow the inner string slice. Returns `None` for other variants.
    #[inline]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Attempt to borrow the inner byte slice. Returns `None` for other variants.
    #[inline]
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(b) => Some(b.as_slice()),
            _ => None,
        }
    }

    /// Attempt to borrow the inner array. Returns `None` for other variants.
    #[inline]
    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Attempt to borrow the inner map. Returns `None` for other variants.
    #[inline]
    pub fn as_map(&self) -> Option<&ValueMap> {
        match self {
            Self::Map(m) => Some(m),
            _ => None,
        }
    }

    /// Return the name of the variant as a static string, useful for error messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "bool",
            Self::Int(_) => "int",
            Self::UInt(_) => "uint",
            Self::Float(_) => "float",
            Self::String(_) => "string",
            Self::Bytes(_) => "bytes",
            Self::Array(_) => "array",
            Self::Map(_) => "map",
        }
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Self::Int(v as i64)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}

impl From<u32> for Value {
    fn from(v: u32) -> Self {
        Self::UInt(v as u64)
    }
}

impl From<u64> for Value {
    fn from(v: u64) -> Self {
        Self::UInt(v)
    }
}

impl From<f32> for Value {
    fn from(v: f32) -> Self {
        Self::Float(v as f64)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Self::Float(v)
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Self::String(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Self::String(v.to_owned())
    }
}

impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Self {
        Self::Bytes(v)
    }
}

impl From<Vec<Value>> for Value {
    fn from(v: Vec<Value>) -> Self {
        Self::Array(v)
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(v: Option<T>) -> Self {
        match v {
            Some(inner) => inner.into(),
            None => Self::Null,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{
        FunctionMap, FunctionSchema, NamespaceMap, NamespaceSchema, PrimitiveType, Schema,
        TypeDescriptor, Visibility,
    };

    /// Regression: Schema -> msgpack bytes -> Value -> msgpack bytes -> Schema must round-trip.
    #[test]
    fn schema_round_trip_via_value() {
        let mut functions = FunctionMap::new();
        functions
            .insert(
                "hello".to_owned(),
                FunctionSchema {
                    args: vec![],
                    returns: TypeDescriptor::primitive(PrimitiveType::Unit),
                    visibility: Visibility::Public,
                    capabilities: vec![],
                    idempotent: false,
                    doc: None,
                },
            )
            .expect("schema fits in FunctionMap capacity");
        let mut namespaces = NamespaceMap::new();
        namespaces
            .insert(
                "svc".to_owned(),
                NamespaceSchema {
                    functions: Box::new(functions),
                    doc: None,
                },
            )
            .expect("schema fits in NamespaceMap capacity");
        let schema = Schema {
            version: 1,
            namespaces: Box::new(namespaces),
            types: Box::new(crate::schema::TypeMap::new()),
        };

        let bytes1 = rmp_serde::to_vec_named(&schema).expect("schema to msgpack");
        let value: Value = rmp_serde::from_slice(&bytes1).expect("msgpack to Value");
        let bytes2 = rmp_serde::to_vec_named(&value).expect("Value to msgpack");
        let schema2: Schema = rmp_serde::from_slice(&bytes2).expect("msgpack to Schema");

        assert_eq!(schema2.version, 1);
        assert!(
            schema2.namespaces.contains_key("svc"),
            "namespace 'svc' not found after round-trip"
        );
    }

    /// Regression: Value::Array must not be confused with Value::Bytes.
    #[test]
    fn array_not_confused_with_bytes() {
        let original = Value::Array(vec![Value::Int(1), Value::Int(2)]);
        let bytes = rmp_serde::to_vec_named(&original).expect("serialize");
        let decoded: Value = rmp_serde::from_slice(&bytes).expect("deserialize");
        assert!(
            matches!(decoded, Value::Array(_)),
            "Expected Array, got: {decoded:?}"
        );
    }

    /// Regression: Value::Bytes must survive a round-trip as msgpack bin.
    #[test]
    fn bytes_round_trip() {
        let original = Value::Bytes(vec![0xde, 0xad, 0xbe, 0xef]);
        let bytes = rmp_serde::to_vec_named(&original).expect("serialize");
        let decoded: Value = rmp_serde::from_slice(&bytes).expect("deserialize");
        assert!(
            matches!(decoded, Value::Bytes(_)),
            "Expected Bytes, got: {decoded:?}"
        );
    }

    #[test]
    fn check_sizes() {
        std::eprintln!("Value: {} bytes", std::mem::size_of::<Value>());
        std::eprintln!("ValueMap: {} bytes", std::mem::size_of::<ValueMap>());
    }

    /// Value::Map with a nested map must round-trip.
    #[test]
    fn simple_map_round_trip() {
        let mut inner = ValueMap::new();
        inner.insert("b".to_owned(), Value::Int(2)).expect("fits");
        let mut outer = ValueMap::new();
        outer
            .insert("a".to_owned(), Value::Map(Box::new(inner)))
            .expect("fits");
        let original = Value::Map(Box::new(outer));
        let bytes = rmp_serde::to_vec_named(&original).expect("serialize");
        let decoded: Value = rmp_serde::from_slice(&bytes).expect("deserialize");
        assert_eq!(original, decoded);
    }
}
