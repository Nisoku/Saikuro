use alloc::{borrow::ToOwned, boxed::Box, string::String};
use core::fmt;
use serde::{Deserialize, Serialize};

use saikuro_event::{Value, ValueMap};

// ResourceHandle
/// An opaque, serialisable reference to large or external data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceHandle {
    /// Unique identifier for this resource instance.
    /// Two handles with the same `id` refer to the
    /// same underlying resource.
    pub id: String,

    /// MIME type of the resource content, if known.
    /// Examples: `"application/octet-stream"`, `"image/png"`, `"text/csv"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,

    /// Total size of the resource in bytes, if known.
    /// `None` means the size is unknown or unbounded (e.g. a live stream).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,

    /// An optional URI that can be used to access the resource directly.
    /// The URI scheme is provider-defined. Common examples:
    /// - `saikuro://res/<id>`: Saikuro-internal reference
    /// - `https://storage.example.com/blobs/<id>`: direct object-storage URL
    /// - `file:///var/data/<id>`: local filesystem path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

impl ResourceHandle {
    /// Create a handle with only the required `id` field.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            mime_type: None,
            size: None,
            uri: None,
        }
    }

    /// Set the MIME type.
    pub fn with_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    /// Set the byte size.
    pub fn with_size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    /// Set the direct-access URI.
    pub fn with_uri(mut self, uri: impl Into<String>) -> Self {
        self.uri = Some(uri.into());
        self
    }

    /// Convert this handle into a [`Value`] map suitable for embedding in an
    /// envelope `result` field.
    pub fn to_value(&self) -> Value {
        // A handle serialises to at most 4 fields, well under VALUE_MAP_CAPACITY.
        let mut map = ValueMap::new();
        map.insert("id".to_owned(), Value::String(self.id.clone()))
            .expect("resource handle map fits in VALUE_MAP_CAPACITY");
        if let Some(mime) = &self.mime_type {
            map.insert("mime_type".to_owned(), Value::String(mime.clone()))
                .expect("resource handle map fits in VALUE_MAP_CAPACITY");
        }
        if let Some(size) = self.size {
            map.insert("size".to_owned(), Value::UInt(size))
                .expect("resource handle map fits in VALUE_MAP_CAPACITY");
        }
        if let Some(uri) = &self.uri {
            map.insert("uri".to_owned(), Value::String(uri.clone()))
                .expect("resource handle map fits in VALUE_MAP_CAPACITY");
        }
        Value::Map(Box::new(map))
    }

    /// Attempt to deserialise a [`ResourceHandle`] from a [`Value`].
    ///
    /// Returns `None` if the value is not a map or is missing the `id` field.
    pub fn from_value(value: &Value) -> Option<Self> {
        let map = match value {
            Value::Map(m) => m,
            _ => return None,
        };
        let id = match map.get("id")? {
            Value::String(s) => s.clone(),
            _ => return None,
        };
        Some(ResourceHandle {
            id,
            mime_type: match map.get("mime_type") {
                Some(Value::String(s)) => Some(s.clone()),
                _ => None,
            },
            size: match map.get("size") {
                Some(Value::UInt(n)) => Some(*n),
                Some(Value::Int(n)) if *n >= 0 => Some(*n as u64),
                _ => None,
            },
            uri: match map.get("uri") {
                Some(Value::String(s)) => Some(s.clone()),
                _ => None,
            },
        })
    }
}

impl fmt::Display for ResourceHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Resource({})", self.id)?;
        if let Some(ref mime) = self.mime_type {
            write!(f, " [{mime}]")?;
        }
        if let Some(size) = self.size {
            write!(f, " {size}B")?;
        }
        Ok(())
    }
}
