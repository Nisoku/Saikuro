use alloc::{
    borrow::ToOwned,
    boxed::Box,
    string::{String, ToString},
};
use saikuro_core::{
    envelope::{Envelope, InvocationType},
    schema::{ArgumentDescriptor, PrimitiveType, TypeDescriptor, Visibility},
    PROTOCOL_VERSION,
};

use saikuro_event::{SaikuroError, Value};

use crate::registry::{FunctionRef, SchemaRegistry};

/// The result of a successful validation pass.
#[derive(Debug)]
pub struct ValidationReport {
    /// The fully resolved function and its owning provider.
    pub function_ref: FunctionRef,
}

/// Stateless invocation validator.
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
    pub async fn validate(&self, envelope: &Envelope) -> Result<ValidationReport, SaikuroError> {
        // 1. Protocol version.
        if envelope.version != PROTOCOL_VERSION {
            return Err(SaikuroError::IncompatibleVersion {
                expected: PROTOCOL_VERSION,
                received: envelope.version,
            });
        }

        // Envelope structure.
        self.check_structural(envelope)?;

        match envelope.invocation_type {
            InvocationType::Batch => self.validate_batch(envelope).await,
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
            _ => self.validate_single(envelope).await,
        }
    }

    // Structural checks
    fn check_structural(&self, envelope: &Envelope) -> Result<(), SaikuroError> {
        let skip_target_check = matches!(
            envelope.invocation_type,
            InvocationType::Batch | InvocationType::Log | InvocationType::Announce
        );
        if !skip_target_check && !envelope.target.contains('.') {
            return Err(SaikuroError::MalformedEnvelope(format!(
                "target '{}' must be in 'namespace.function' format",
                envelope.target
            )));
        }

        // Batch-specific: must have items, must not have a target.
        if envelope.invocation_type == InvocationType::Batch {
            match &envelope.batch_items {
                None => return Err(SaikuroError::MissingBatch),
                Some(items) if items.is_empty() => return Err(SaikuroError::EmptyBatch),
                _ => {}
            }
        }

        Ok(())
    }

    // Single-invocation validation
    async fn validate_single(&self, envelope: &Envelope) -> Result<ValidationReport, SaikuroError> {
        // Schema lookup.
        let func_ref = self.registry.lookup_function(&envelope.target).await?;

        // Visibility.
        self.check_visibility(&envelope.target, &func_ref.schema.visibility)?;

        // Argument validation.
        self.check_arguments(&envelope.target, &func_ref.schema.args, &envelope.args)?;

        Ok(ValidationReport {
            function_ref: func_ref,
        })
    }

    // Batch validation
    async fn validate_batch(&self, envelope: &Envelope) -> Result<ValidationReport, SaikuroError> {
        let items = envelope.batch_items.as_ref().ok_or_else(|| {
            SaikuroError::MalformedEnvelope("batch envelope missing batch_items".into())
        })?;

        // Validate each item; collect the first error with its index.
        for (index, item) in items.iter().enumerate() {
            self.validate(item).await
                .map_err(|source| SaikuroError::BatchItemFailed {
                    index,
                    reason: source.to_string(),
                })?;
        }

        // For batch we return a synthetic report. The router will dispatch each
        // item individually and collect results.
        let first_ref = self.registry.lookup_function(&items[0].target).await?;

        Ok(ValidationReport {
            function_ref: first_ref,
        })
    }

    // Helpers
    fn check_visibility(
        &self,
        target: &str,
        visibility: &Visibility,
    ) -> Result<(), SaikuroError> {
        match visibility {
            Visibility::Public => Ok(()),
            Visibility::Internal if self.allow_internal => Ok(()),
            Visibility::Internal => Err(SaikuroError::VisibilityDenied {
                target: target.to_owned(),
                visibility: format!("{visibility:?}"),
            }),
            Visibility::Private => Err(SaikuroError::VisibilityDenied {
                target: target.to_owned(),
                visibility: format!("{visibility:?}"),
            }),
        }
    }

    fn check_arguments(
        &self,
        target: &str,
        declared: &[ArgumentDescriptor],
        provided: &[Value],
    ) -> Result<(), SaikuroError> {
        // Count required args (those without defaults and not optional).
        let required_count = declared
            .iter()
            .filter(|a| !a.optional && a.default.is_none())
            .count();

        if provided.len() < required_count {
            return Err(SaikuroError::ArgumentArity {
                expected: required_count,
                received: provided.len(),
            });
        }

        if provided.len() > declared.len() {
            return Err(SaikuroError::ArgumentArity {
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
    fn check_value_type(
        &self,
        target: &str,
        position: usize,
        name: &str,
        descriptor: &TypeDescriptor,
        value: &Value,
    ) -> Result<(), SaikuroError> {
        let type_error = |expected: &str| SaikuroError::ArgumentType {
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
                Err(SaikuroError::MalformedEnvelope(
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
    ) -> Result<(), SaikuroError> {
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
            Err(SaikuroError::ArgumentType {
                name: name.to_owned(),
                position,
                expected: prim.to_string(),
                received: value.type_name().to_owned(),
            })
        }
    }
}
