//! Globally-unique invocation identifiers.
//!
//! Every invocation :  whether a call, cast, stream open, or channel open :
//! carries an [`InvocationId`]. Responses are correlated back to their
//! originating invocation using this identifier. UUIDs v4 are used to ensure
//! global uniqueness without coordination.

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use serde_bytes::ByteBuf;
use std::fmt;
use uuid::Uuid;

/// A globally-unique identifier for a single invocation.
///
/// Internally this is a UUID v4 represented as a compact 16-byte array for
/// efficient wire encoding via MessagePack. The `Display` and `Debug`
/// implementations render it as the canonical hyphenated UUID string.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InvocationId(Uuid);

impl Serialize for InvocationId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            serializer.serialize_str(&self.0.to_string())
        } else {
            serializer.serialize_bytes(self.0.as_bytes())
        }
    }
}

impl<'de> Deserialize<'de> for InvocationId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum InvocationIdWire {
            String(String),
            Bytes(ByteBuf),
            Array(Vec<u8>),
        }

        let wire = InvocationIdWire::deserialize(deserializer)?;
        match wire {
            InvocationIdWire::String(text) => Uuid::parse_str(&text)
                .map(InvocationId)
                .map_err(de::Error::custom),
            InvocationIdWire::Bytes(bytes) => {
                let bytes = bytes.into_vec();
                let slice: [u8; 16] = bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| de::Error::custom("expected a 16 byte UUID"))?;
                Ok(InvocationId(Uuid::from_bytes(slice)))
            }
            InvocationIdWire::Array(bytes) => {
                let slice: [u8; 16] = bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| de::Error::custom("expected a 16 byte UUID"))?;
                Ok(InvocationId(Uuid::from_bytes(slice)))
            }
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::InvocationId;

    #[test]
    fn msgpack_roundtrip_uses_binary_uuid() {
        let id = InvocationId::new();
        let encoded = rmp_serde::to_vec_named(&id).expect("encode invocation id");
        let decoded: InvocationId = rmp_serde::from_slice(&encoded).expect("decode invocation id");
        assert_eq!(id, decoded);
    }

    #[test]
    fn msgpack_accepts_uuid_string_for_compatibility() {
        let uuid_text = "6f9619ff-8b86-d011-b42d-00cf4fc964ff";
        let encoded = rmp_serde::to_vec_named(&uuid_text).expect("encode uuid string payload");
        let decoded: InvocationId =
            rmp_serde::from_slice(&encoded).expect("decode uuid string payload");

        let text = decoded.to_string();
        assert_eq!(text, uuid_text);
    }
}
