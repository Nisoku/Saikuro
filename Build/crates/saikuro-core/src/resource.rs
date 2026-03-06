//! Resource handle type.
//!
//! A [`ResourceHandle`] is an opaque reference to large or external data that
//! is too expensive to inline in a regular response: a file on disk, a blob
//! in object storage, a database cursor, etc.
//!
//! The handle carries enough metadata for the recipient to:
//! - Identify the resource uniquely (`id`)
//! - Know how large it is without fetching it (`size`)
//! - Know its content type (`mime_type`)
//! - Optionally open it via a well-known URI scheme (`uri`)
//!
//! Handles are opaque to the Saikuro runtime: the runtime routes the `Resource`
//! envelope to the provider and returns whatever the provider placed in the
//! response `result` field.  The adapter is responsible for presenting a typed
//! [`ResourceHandle`] to its callers.
//!
//! # Wire format
//!
//! A `ResourceHandle` is serialised as a flat MessagePack map:
//!
//! ```json
//! {
//!   "id":        "<handle-uuid>",
//!   "mime_type": "application/octet-stream",  // optional
//!   "size":      12345,                         // optional, bytes
//!   "uri":       "saikuro://res/<id>"           // optional
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;

// ResourceHandle

/// An opaque, serialisable reference to large or external data.
///
/// Created by a provider and returned to callers as the `result` of a
/// `Resource`-type invocation.  Callers use the handle to retrieve,
/// stream, or otherwise interact with the referenced data without
/// transferring it inline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceHandle {
    /// Unique identifier for this resource instance.
    ///
    /// Typically a UUID v4.  Two handles with the same `id` refer to the
    /// same underlying resource.
    pub id: String,

    /// MIME type of the resource content, if known.
    ///
    /// Examples: `"application/octet-stream"`, `"image/png"`, `"text/csv"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,

    /// Total size of the resource in bytes, if known.
    ///
    /// `None` means the size is unknown or unbounded (e.g. a live stream).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,

    /// An optional URI that can be used to access the resource directly.
    ///
    /// The URI scheme is provider-defined.  Common examples:
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

    /// Convert this handle into a [`crate::value::Value`] map suitable for
    /// embedding in an envelope `result` field.
    pub fn to_value(&self) -> crate::value::Value {
        let bytes =
            rmp_serde::to_vec_named(self).expect("ResourceHandle serialisation is infallible");
        rmp_serde::from_slice(&bytes)
            .expect("ResourceHandle round-trip through Value is infallible")
    }

    /// Attempt to deserialise a [`ResourceHandle`] from a [`crate::value::Value`].
    ///
    /// Returns `None` if the value is not a map or is missing the `id` field.
    pub fn from_value(value: &crate::value::Value) -> Option<Self> {
        let bytes = rmp_serde::to_vec_named(value).ok()?;
        rmp_serde::from_slice(&bytes).ok()
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

//  Tests (minimal inline)

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_handle_roundtrips_through_value() {
        let h = ResourceHandle::new("abc-123")
            .with_mime_type("text/plain")
            .with_size(42)
            .with_uri("saikuro://res/abc-123");

        let v = h.to_value();
        let decoded = ResourceHandle::from_value(&v).expect("decode");
        assert_eq!(decoded, h);
    }

    #[test]
    fn resource_handle_minimal_roundtrip() {
        let h = ResourceHandle::new("xyz");
        let v = h.to_value();
        let decoded = ResourceHandle::from_value(&v).expect("decode");
        assert_eq!(decoded.id, "xyz");
        assert!(decoded.mime_type.is_none());
        assert!(decoded.size.is_none());
        assert!(decoded.uri.is_none());
    }
}
