//! Capability enforcement engine.
//!
//! The capability engine is responsible for one thing: answering the question
//! "does this peer hold all the capabilities required to invoke this function?".
//!
//! It is intentionally kept stateless and pure:  all state lives in the
//! [`CapabilitySet`] that the caller presents.  The engine never issues tokens;
//! that is the responsibility of the handshake layer (not yet in v1 scope).
//!
//! Sandbox mode:
//! When `sandbox_mode` is enabled, even requests that pass capability checks
//! are further restricted: the engine only exposes the subset of namespaces
//! declared in the peer's sandbox schema.  Additionally, functions with
//! [`Visibility::Internal`] visibility are treated as inaccessible:  only
//! `Public` functions are reachable by sandboxed peers.

use saikuro_core::{
    capability::{CapabilitySet, CapabilityToken},
    schema::{FunctionSchema, Visibility},
};
use tracing::debug;

use crate::registry::FunctionRef;

/// The result of a capability check.
#[derive(Debug)]
pub enum CapabilityOutcome {
    /// All required capabilities are held by the caller.
    Granted,
    /// The caller is missing this specific required capability.
    Denied { missing: CapabilityToken },
}

/// Engine that enforces capability requirements on invocations.
#[derive(Clone, Default)]
pub struct CapabilityEngine {
    /// When `true`, the engine enforces sandbox restrictions on top of normal
    /// capability checks: `Internal` functions become inaccessible and the
    /// accessible schema is filtered to only `Public` functions the peer has
    /// capabilities for.
    sandbox_mode: bool,
}

impl CapabilityEngine {
    /// Create a default capability engine (non-sandbox).
    pub fn new() -> Self {
        Self {
            sandbox_mode: false,
        }
    }

    /// Create an engine in sandbox mode.  Untrusted peers receive restricted
    /// schemas and additionally have their capability sets constrained.
    pub fn sandboxed() -> Self {
        Self { sandbox_mode: true }
    }

    /// Return `true` if this engine is in sandbox mode.
    pub fn is_sandboxed(&self) -> bool {
        self.sandbox_mode
    }

    /// Check whether `caller_caps` satisfies the requirements declared in
    /// `function_schema`.
    ///
    /// In sandbox mode, [`Visibility::Internal`] functions are always denied
    /// regardless of capabilities:  they are not accessible to untrusted peers.
    ///
    /// Returns [`CapabilityOutcome::Granted`] if all requirements are met, or
    /// [`CapabilityOutcome::Denied`] with the first missing token otherwise.
    pub fn check(
        &self,
        caller_caps: &CapabilitySet,
        function_schema: &FunctionSchema,
    ) -> CapabilityOutcome {
        // In sandbox mode, Internal-visibility functions are inaccessible.
        if self.sandbox_mode && function_schema.visibility == Visibility::Internal {
            debug!("sandbox: denying access to Internal function");
            return CapabilityOutcome::Denied {
                missing: CapabilityToken::new("$sandbox.public_only"),
            };
        }

        for required in &function_schema.capabilities {
            if !caller_caps.grants(required) {
                debug!(
                    missing = %required,
                    "capability check failed"
                );
                return CapabilityOutcome::Denied {
                    missing: required.clone(),
                };
            }
        }
        CapabilityOutcome::Granted
    }

    /// A convenience wrapper that checks against a resolved [`FunctionRef`].
    pub fn check_ref(
        &self,
        caller_caps: &CapabilitySet,
        func_ref: &FunctionRef,
    ) -> CapabilityOutcome {
        self.check(caller_caps, &func_ref.schema)
    }

    /// Filter a list of function names down to only those visible and callable
    /// with the given capability set.  Used to generate sandbox-restricted schemas.
    ///
    /// In sandbox mode this additionally excludes `Internal` functions.
    /// `Private` functions are always excluded.
    pub fn filter_accessible_functions<'a>(
        &self,
        functions: impl Iterator<Item = (&'a str, &'a FunctionSchema)>,
        caller_caps: &CapabilitySet,
    ) -> Vec<String> {
        functions
            .filter(|(_name, schema)| {
                // Private functions are never accessible.
                if schema.visibility == Visibility::Private {
                    return false;
                }
                matches!(self.check(caller_caps, schema), CapabilityOutcome::Granted)
            })
            .map(|(name, _)| name.to_owned())
            .collect()
    }
}
