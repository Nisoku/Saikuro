use alloc::{borrow::ToOwned, string::String, vec::Vec};
use core::fmt;
use serde::{
    de::{Deserializer, MapAccess, Visitor},
    ser::{SerializeMap, Serializer},
    Deserialize, Serialize,
};

/// Mutable string-keyed map of [`Value`]s.
///
/// Backed by a `Vec<(String, Value)>` kept sorted by key.
#[derive(Debug, Clone, Default)]
pub struct ValueMap {
    entries: Vec<(String, Value)>,
}

impl ValueMap {
    /// Create an empty map. The memory model treats this as free: nothing is
    /// reserved until the first entry is inserted.
    #[inline]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Insert or replace the entry for `key`, returning the previous value if
    /// one was present.
    #[inline]
    pub fn insert(&mut self, key: String, value: Value) -> Option<Value> {
        match self
            .entries
            .binary_search_by(|(existing, _)| existing.as_str().cmp(key.as_str()))
        {
            Ok(position) => Some(core::mem::replace(&mut self.entries[position].1, value)),
            Err(position) => {
                self.entries.insert(position, (key, value));
                None
            }
        }
    }

    /// Borrow the value stored under `key`, if present.
    #[inline]
    pub fn get(&self, key: &str) -> Option<&Value> {
        let position = self
            .entries
            .binary_search_by(|(existing, _)| existing.as_str().cmp(key))
            .ok()?;
        Some(&self.entries[position].1)
    }

    /// Remove and return the value stored under `key`, if present.
    #[inline]
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let position = self
            .entries
            .binary_search_by(|(existing, _)| existing.as_str().cmp(key))
            .ok()?;
        Some(self.entries.remove(position).1)
    }

    /// Number of entries held by the map.
    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when the map holds no entries.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over `(key, value)` pairs in sorted key order.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.entries.iter().map(pair_ref)
    }
}

fn pair_ref(pair: &(String, Value)) -> (&String, &Value) {
    (&pair.0, &pair.1)
}

impl IntoIterator for ValueMap {
    type Item = (String, Value);
    type IntoIter = alloc::vec::IntoIter<(String, Value)>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl<'a> IntoIterator for &'a ValueMap {
    type Item = (&'a String, &'a Value);
    type IntoIter = core::iter::Map<
        core::slice::Iter<'a, (String, Value)>,
        fn(&'a (String, Value)) -> (&'a String, &'a Value),
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter().map(pair_ref)
    }
}

impl Serialize for ValueMap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut output = serializer.serialize_map(Some(self.entries.len()))?;
        for (key, value) in &self.entries {
            output.serialize_entry(key, value)?;
        }
        output.end()
    }
}

impl<'de> Deserialize<'de> for ValueMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ValueMapVisitor;

        impl<'de> Visitor<'de> for ValueMapVisitor {
            type Value = ValueMap;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string-keyed map of values")
            }

            fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut entries = Vec::with_capacity(access.size_hint().unwrap_or(0));
                while let Some((key, value)) = access.next_entry::<String, Value>()? {
                    entries.push((key, value));
                }
                // Sorted key order makes re-encoding canonical regardless of
                // the wire order the frame arrived in.
                entries.sort_unstable_by(|(left, _), (right, _)| left.as_str().cmp(right.as_str()));
                Ok(ValueMap { entries })
            }
        }

        deserializer.deserialize_map(ValueMapVisitor)
    }
}

/// A dynamically-typed value that can appear in an invocation argument list,
/// a return value, an error detail bag, or a schema default.
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
    Array(Vec<Value>),

    /// Raw binary blob (resource handles, opaque payloads, …).
    #[serde(with = "serde_bytes")]
    Bytes(Vec<u8>),

    /// String-keyed mapping of values.
    Map(ValueMap),
}

/// Equality for [`Value`].
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
                        .all(|(key, value)| b.get(key).is_some_and(|other| other == value))
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

/// Convert this core [`Value`] into a JSON-compatible [`serde_json::Value`].
pub fn core_to_json(v: Value) -> serde_json::Value {
    match serde_json::to_value(&v) {
        Ok(j) => j,
        Err(_) => serde_json::Value::Null,
    }
}

/// Convert a JSON-compatible [`serde_json::Value`] into a core [`Value`].
pub fn json_to_core(v: serde_json::Value) -> Value {
    match serde_json::from_value(v) {
        Ok(c) => c,
        Err(_) => Value::Null,
    }
}
