//! Globally-unique invocation identifiers.
//!
//! Every invocation :  whether a call, cast, stream open, or channel open :
//! carries an [`InvocationId`]. Responses are correlated back to their
//! originating invocation using this identifier. UUIDs v4 are used to ensure
//! global uniqueness without coordination.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// A globally-unique identifier for a single invocation.
///
/// Internally this is a UUID v4 represented as a compact 16-byte array for
/// efficient wire encoding via MessagePack. The `Display` and `Debug`
/// implementations render it as the canonical hyphenated UUID string.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InvocationId(Uuid);

impl InvocationId {
    /// Generate a fresh, globally-unique invocation identifier.
    #[inline]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Construct from an existing UUID.
    #[inline]
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Return the underlying UUID.
    #[inline]
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    /// Return the raw 16-byte representation, suitable for compact wire encoding.
    #[inline]
    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }
}

impl Default for InvocationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for InvocationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for InvocationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "InvocationId({})", self.0)
    }
}

impl From<Uuid> for InvocationId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<InvocationId> for Uuid {
    fn from(id: InvocationId) -> Self {
        id.0
    }
}

impl std::str::FromStr for InvocationId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}
