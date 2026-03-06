//! Saikuro Schema
//!
//! This crate owns the runtime schema registry, invocation validator, and
//! capability enforcement engine.  It is the source of truth for "is this
//! invocation well-formed and permitted?".

pub mod capability_engine;
pub mod registry;
pub mod validator;

pub use capability_engine::CapabilityEngine;
pub use registry::{NamespaceRegistration, SchemaRegistry};
pub use validator::{InvocationValidator, ValidationReport};
