use alloc::{borrow::ToOwned, boxed::Box, string::String, vec::Vec};
use serde::{ser::SerializeMap, Deserialize, Serialize, Serializer};

/// Maximum number of entries a [`Value::Map`] can hold.
pub const VALUE_MAP_CAPACITY: usize = 64;

/// Fixed-capacity map backing [`Value::Map`].
pub type ValueMap = heapless::FnvIndexMap<String, Value, VALUE_MAP_CAPACITY>;

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
    Map(#[serde(serialize_with = "serialize_value_map")] Box<ValueMap>),
}

fn serialize_value_map<S>(map: &ValueMap, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut entries: Vec<_> = map.iter().collect();
    entries.sort_unstable_by_key(|(key, _)| *key);
    let mut output = serializer.serialize_map(Some(entries.len()))?;
    for (key, value) in entries {
        output.serialize_entry(key, value)?;
    }
    output.end()
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
