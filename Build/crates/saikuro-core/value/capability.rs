use alloc::string::String;
use core::fmt;
use serde::{Deserialize, Serialize};

/// Sentinel token value that grants access to all capabilities.
pub const WILDCARD_TOKEN: &str = "*";

/// Maximum number of distinct capability tokens a peer can hold.
pub const CAPABILITY_SET_CAPACITY: usize = 256;

/// Fixed-capacity set of capability tokens held by a peer.
pub type TokenSet = heapless::FnvIndexSet<CapabilityToken, CAPABILITY_SET_CAPACITY>;

/// A single capability token: a namespaced, human-readable permission string.
///
/// By convention tokens are dot-separated: `"<namespace>.<permission>"`.
/// The runtime treats them as opaque strings; no hierarchical wildcard
/// expansion is performed yet (exact match only).
/// TODO: Implement hierarchical wildcard expansion.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CapabilityToken(pub String);

impl CapabilityToken {
    /// Construct a new token from any string-like value.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Return the token string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CapabilityToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for CapabilityToken {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for CapabilityToken {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// The full set of capability tokens granted to a peer.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilitySet {
    tokens: TokenSet,
}

impl CapabilitySet {
    /// Construct an empty set (peer has no special capabilities).
    pub fn empty() -> Self {
        Self::default()
    }

    /// Construct a set from an iterator of tokens.
    pub fn from_tokens(
        iter: impl IntoIterator<Item = CapabilityToken>,
    ) -> Result<Self, &'static str> {
        let mut tokens = TokenSet::new();
        for token in iter {
            tokens
                .insert(token)
                .map_err(|_| "capability set capacity exceeded")?;
        }
        Ok(Self { tokens })
    }

    /// Construct an unrestricted set that passes all capability checks.
    /// This is used for internal/trusted peers only.
    pub fn all_powerful() -> Self {
        // Sentinel: we use a special token that the capability engine
        // recognises as granting everything.
        Self::from_tokens([CapabilityToken::new(WILDCARD_TOKEN)])
            .expect("wildcard token always fits in CAPABILITY_SET_CAPACITY")
    }

    /// Return `true` if this set grants the given capability.
    /// The wildcard token `"*"` grants every capability.
    pub fn grants(&self, required: &CapabilityToken) -> bool {
        self.tokens.contains(&CapabilityToken::new(WILDCARD_TOKEN))
            || self.tokens.contains(required)
    }

    /// Return `true` if this set satisfies *all* of the required capabilities.
    pub fn grants_all<'a>(&self, required: impl IntoIterator<Item = &'a CapabilityToken>) -> bool {
        required.into_iter().all(|cap| self.grants(cap))
    }

    /// Add a token to the set.
    pub fn insert(&mut self, token: CapabilityToken) -> Result<bool, CapabilityToken> {
        self.tokens.insert(token)
    }

    /// Return an iterator over all tokens in the set.
    pub fn iter(&self) -> impl Iterator<Item = &CapabilityToken> {
        self.tokens.iter()
    }

    /// Return the number of tokens in the set.
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    /// Return `true` if no tokens are held.
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }
}
