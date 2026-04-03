//! Invocation validator.
//!
//! The validator sits between the transport layer and the router.  Every
//! inbound [`Envelope`] passes through here before being dispatched:
//!
//! 1. Protocol version check.
//! 2. Envelope structural integrity (required fields present, well-formed
//!    target, batch items non-empty when type is Batch, …).
//! 3. Schema lookup (does the target function exist?).
//! 4. Argument arity and type checking.
//! 5. Visibility enforcement (private/internal functions are not callable
//!    from external peers).
//! 6. Capability checking is delegated to [`CapabilityEngine`].
//!
//! All errors are returned as typed [`ValidationError`] values so the
//! runtime can produce the right [`ErrorCode`] on the wire.

use saikuro_core::{
    envelope::{Envelope, InvocationType},
    error::ErrorCode,
    schema::{ArgumentDescriptor, PrimitiveType, TypeDescriptor, Visibility},
    value::Value,
    PROTOCOL_VERSION,
};
use thiserror::Error;

use crate::registry::{FunctionRef, RegistryError, SchemaRegistry};

//  Errors

/// A validation failure.
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("incompatible protocol version: expected {expected}, got {received}")]
    IncompatibleVersion { expected: u32, received: u32 },

    #[error("malformed envelope: {0}")]
    MalformedEnvelope(String),

    #[error("schema error: {0}")]
    Schema(Box<RegistryError>),

    #[error("wrong number of arguments: expected {expected}, got {received}")]
    ArgumentArity { expected: usize, received: usize },

    #[error("argument '{name}' (position {position}): expected {expected}, got {received}")]
    ArgumentType {
        name: String,
        position: usize,
        expected: String,
        received: String,
    },

    #[error("function '{target}' is {visibility:?} and cannot be called by this peer")]
    VisibilityDenied {
        target: String,
        visibility: Visibility,
    },

    #[error("batch envelope must contain at least one item")]
    EmptyBatch,

    #[error("batch item at index {index}: {source}")]
    BatchItem {
        index: usize,
        #[source]
        source: Box<ValidationError>,
    },
}

impl From<RegistryError> for ValidationError {
    fn from(e: RegistryError) -> Self {
        ValidationError::Schema(Box::new(e))
    }
}

impl ValidationError {
    /// Map this error to the appropriate wire [`ErrorCode`].
    pub fn error_code(&self) -> ErrorCode {
        match self {
            Self::IncompatibleVersion { .. } => ErrorCode::IncompatibleVersion,
            Self::MalformedEnvelope(_) => ErrorCode::MalformedEnvelope,
            Self::Schema(e) => match e.as_ref() {
                RegistryError::NamespaceNotFound(_) => ErrorCode::NamespaceNotFound,
                RegistryError::FunctionNotFound(_) => ErrorCode::FunctionNotFound,
                RegistryError::MalformedTarget(_) => ErrorCode::MalformedEnvelope,
                RegistryError::FrozenSchema(_) => ErrorCode::Internal,
                RegistryError::Validation(_) => ErrorCode::InvalidArguments,
            },
            Self::ArgumentArity { .. } | Self::ArgumentType { .. } => ErrorCode::InvalidArguments,
            Self::VisibilityDenied { .. } => ErrorCode::CapabilityDenied,
            Self::EmptyBatch => ErrorCode::MalformedEnvelope,
            Self::BatchItem { source, .. } => source.error_code(),
        }
    }
}

//  Report

/// The result of a successful validation pass.  Carries the resolved function
/// reference so the router doesn't need to look it up again.
#[derive(Debug)]
pub struct ValidationReport {
    /// The fully resolved function and its owning provider.
    pub function_ref: FunctionRef,
}

//  Validator

/// Stateless invocation validator.
///
/// This is `Clone`-cheap because the [`SchemaRegistry`] behind it is `Arc`-shared.
#[derive(Clone)]
pub struct InvocationValidator {
    registry: SchemaRegistry,
    /// Whether external peers can call `internal` functions.
    /// Set to `true` for trusted intra-cluster peers.
    allow_internal: bool,
}

impl InvocationValidator {
    pub fn new(registry: SchemaRegistry) -> Self {
        Self {
            registry,
            allow_internal: false,
        }
    }

    /// Build a validator that also permits `internal` visibility functions.
    pub fn with_internal_access(registry: SchemaRegistry) -> Self {
        Self {
            registry,
            allow_internal: true,
        }
    }

    /// Validate a single envelope.
    ///
    /// For [`InvocationType::Batch`] each item is validated recursively.
    pub fn validate(&self, envelope: &Envelope) -> Result<ValidationReport, ValidationError> {
        // 1. Protocol version.
        if envelope.version != PROTOCOL_VERSION {
            return Err(ValidationError::IncompatibleVersion {
                expected: PROTOCOL_VERSION,
                received: envelope.version,
            });
        }

        // 2. Envelope structure.
        self.check_structural(envelope)?;

        match envelope.invocation_type {
            InvocationType::Batch => self.validate_batch(envelope),
            // Log and Announce are system envelopes handled before schema lookup;
            // they bypass function-level validation entirely.  Return a synthetic
            // report that won't be used for capability checking.
            InvocationType::Log | InvocationType::Announce => Ok(ValidationReport {
                function_ref: crate::registry::FunctionRef {
                    namespace: String::new(),
                    function: String::new(),
                    schema: saikuro_core::schema::FunctionSchema {
                        args: vec![],
                        returns: saikuro_core::schema::TypeDescriptor::primitive(
                            saikuro_core::schema::PrimitiveType::Unit,
                        ),
                        visibility: saikuro_core::schema::Visibility::Public,
                        capabilities: vec![],
                        idempotent: false,
                        doc: None,
                    },
                    provider_id: String::new(),
                },
            }),
            _ => self.validate_single(envelope),
        }
    }

    // Structural checks

    fn check_structural(&self, envelope: &Envelope) -> Result<(), ValidationError> {
        // Target must be "namespace.function":  except for system envelope types
        // (Log, Announce, Batch) that use special targets or no target at all.
        let skip_target_check = matches!(
            envelope.invocation_type,
            InvocationType::Batch | InvocationType::Log | InvocationType::Announce
        );
        if !skip_target_check && !envelope.target.contains('.') {
            return Err(ValidationError::MalformedEnvelope(format!(
                "target '{}' must be in 'namespace.function' format",
                envelope.target
            )));
        }

        // Batch-specific: must have items, must not have a target.
        if envelope.invocation_type == InvocationType::Batch {
            match &envelope.batch_items {
                None => return Err(ValidationError::EmptyBatch),
                Some(items) if items.is_empty() => return Err(ValidationError::EmptyBatch),
                _ => {}
            }
        }

        Ok(())
    }

    // Single-invocation validation

    fn validate_single(&self, envelope: &Envelope) -> Result<ValidationReport, ValidationError> {
        // Schema lookup.
        let func_ref = self.registry.lookup_function(&envelope.target)?;

        // Visibility.
        self.check_visibility(&envelope.target, &func_ref.schema.visibility)?;

        // Argument validation.
        self.check_arguments(&envelope.target, &func_ref.schema.args, &envelope.args)?;

        Ok(ValidationReport {
            function_ref: func_ref,
        })
    }

    // Batch validation

    fn validate_batch(&self, envelope: &Envelope) -> Result<ValidationReport, ValidationError> {
        let items = envelope.batch_items.as_ref().unwrap(); // checked in structural pass

        // Validate each item; collect the first error with its index.
        for (index, item) in items.iter().enumerate() {
            self.validate(item)
                .map_err(|source| ValidationError::BatchItem {
                    index,
                    source: Box::new(source),
                })?;
        }

        // For batch we return a synthetic report.  The router will dispatch each
        // item individually and collect results.
        // We use the first item's function ref as the representative report.
        let first_ref = self.registry.lookup_function(&items[0].target)?;

        Ok(ValidationReport {
            function_ref: first_ref,
        })
    }

    // Helpers

    fn check_visibility(
        &self,
        target: &str,
        visibility: &Visibility,
    ) -> Result<(), ValidationError> {
        match visibility {
            Visibility::Public => Ok(()),
            Visibility::Internal if self.allow_internal => Ok(()),
            Visibility::Internal => Err(ValidationError::VisibilityDenied {
                target: target.to_owned(),
                visibility: Visibility::Internal,
            }),
            Visibility::Private => Err(ValidationError::VisibilityDenied {
                target: target.to_owned(),
                visibility: Visibility::Private,
            }),
        }
    }

    fn check_arguments(
        &self,
        target: &str,
        declared: &[ArgumentDescriptor],
        provided: &[Value],
    ) -> Result<(), ValidationError> {
        // Count required args (those without defaults and not optional).
        let required_count = declared
            .iter()
            .filter(|a| !a.optional && a.default.is_none())
            .count();

        if provided.len() < required_count {
            return Err(ValidationError::ArgumentArity {
                expected: required_count,
                received: provided.len(),
            });
        }

        if provided.len() > declared.len() {
            return Err(ValidationError::ArgumentArity {
                expected: declared.len(),
                received: provided.len(),
            });
        }

        // Type-check each provided argument.
        for (position, (arg_schema, provided_value)) in
            declared.iter().zip(provided.iter()).enumerate()
        {
            self.check_value_type(
                target,
                position,
                &arg_schema.name,
                &arg_schema.r#type,
                provided_value,
            )?;
        }

        Ok(())
    }

    /// Recursively check that `value` is compatible with `descriptor`.
    ///
    /// We apply structural subtype checking rather than exact nominal checking:
    /// e.g. an `i32` value is accepted where `i64` is declared.
    fn check_value_type(
        &self,
        target: &str,
        position: usize,
        name: &str,
        descriptor: &TypeDescriptor,
        value: &Value,
    ) -> Result<(), ValidationError> {
        let type_error = |expected: &str| ValidationError::ArgumentType {
            name: name.to_owned(),
            position,
            expected: expected.to_owned(),
            received: value.type_name().to_owned(),
        };

        match descriptor {
            TypeDescriptor::Primitive { r#type } => {
                self.check_primitive(target, position, name, r#type, value)
            }

            TypeDescriptor::Named { .. } => {
                // Named types must be maps (record) or strings (enum variants).
                // Full structural validation against the type definition is a
                // future enhancement; for now we accept maps and strings.
                match value {
                    Value::Map(_) | Value::String(_) => Ok(()),
                    Value::Null => Ok(()), // null is always acceptable for named types
                    _ => Err(type_error("map or string for named type")),
                }
            }

            TypeDescriptor::Option { inner } => {
                if value.is_null() {
                    return Ok(());
                }
                self.check_value_type(target, position, name, inner, value)
            }

            TypeDescriptor::Array { item } => {
                let items = value.as_array().ok_or_else(|| type_error("array"))?;
                for (i, item_value) in items.iter().enumerate() {
                    let inner_name = format!("{name}[{i}]");
                    self.check_value_type(target, i, &inner_name, item, item_value)?;
                }
                Ok(())
            }

            TypeDescriptor::Map { value: val_type } => {
                let map = value.as_map().ok_or_else(|| type_error("map"))?;
                for (k, v) in map {
                    self.check_value_type(target, position, k, val_type, v)?;
                }
                Ok(())
            }

            // Stream and Channel types appear only in return-type positions;
            // they cannot appear in argument lists.
            TypeDescriptor::Stream { .. } | TypeDescriptor::Channel { .. } => {
                Err(ValidationError::MalformedEnvelope(
                    "stream/channel types are not valid argument types".to_owned(),
                ))
            }
        }
    }

    fn check_primitive(
        &self,
        _target: &str,
        position: usize,
        name: &str,
        prim: &PrimitiveType,
        value: &Value,
    ) -> Result<(), ValidationError> {
        let ok = match prim {
            PrimitiveType::Bool => value.as_bool().is_some(),
            PrimitiveType::I8 | PrimitiveType::I16 | PrimitiveType::I32 | PrimitiveType::I64 => {
                value.as_i64().is_some()
            }
            PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64 => {
                value.as_u64().is_some()
            }
            PrimitiveType::F32 | PrimitiveType::F64 => value.as_f64().is_some(),
            PrimitiveType::String => value.as_str().is_some(),
            PrimitiveType::Bytes => value.as_bytes().is_some(),
            PrimitiveType::Any => true,
            PrimitiveType::Unit => value.is_null(),
        };

        if ok {
            Ok(())
        } else {
            Err(ValidationError::ArgumentType {
                name: name.to_owned(),
                position,
                expected: prim.to_string(),
                received: value.type_name().to_owned(),
            })
        }
    }
}
