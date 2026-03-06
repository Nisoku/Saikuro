//! Schema registry and invocation validator tests

use saikuro_core::{
    envelope::{Envelope, InvocationType},
    error::ErrorCode,
    schema::{
        ArgumentDescriptor, FunctionSchema, NamespaceSchema, PrimitiveType, TypeDescriptor,
        Visibility,
    },
    value::Value,
};
use saikuro_schema::{
    registry::{NamespaceRegistration, SchemaRegistry},
    validator::{InvocationValidator, ValidationError},
};
use std::collections::HashMap;

// Helpers

fn two_arg_fn(vis: Visibility) -> FunctionSchema {
    FunctionSchema {
        args: vec![
            ArgumentDescriptor {
                name: "a".into(),
                r#type: TypeDescriptor::primitive(PrimitiveType::I64),
                optional: false,
                default: None,
                doc: None,
            },
            ArgumentDescriptor {
                name: "b".into(),
                r#type: TypeDescriptor::primitive(PrimitiveType::I64),
                optional: false,
                default: None,
                doc: None,
            },
        ],
        returns: TypeDescriptor::primitive(PrimitiveType::I64),
        visibility: vis,
        capabilities: vec![],
        idempotent: true,
        doc: Some("add two integers".into()),
    }
}

fn unit_fn() -> FunctionSchema {
    FunctionSchema {
        args: vec![],
        returns: TypeDescriptor::primitive(PrimitiveType::Unit),
        visibility: Visibility::Public,
        capabilities: vec![],
        idempotent: false,
        doc: None,
    }
}

fn make_registry_with_math() -> SchemaRegistry {
    let registry = SchemaRegistry::new();
    let mut functions = HashMap::new();
    functions.insert("add".into(), two_arg_fn(Visibility::Public));
    functions.insert("noop".into(), unit_fn());
    functions.insert("internal_op".into(), {
        let mut f = unit_fn();
        f.visibility = Visibility::Internal;
        f
    });
    functions.insert("secret".into(), {
        let mut f = unit_fn();
        f.visibility = Visibility::Private;
        f
    });

    registry
        .register(NamespaceRegistration {
            namespace: "math".into(),
            schema: NamespaceSchema {
                functions,
                doc: None,
            },
            provider_id: "provider-1".into(),
        })
        .unwrap();

    registry
}

// Tests

#[test]
fn lookup_existing_function() {
    let registry = make_registry_with_math();
    let func = registry.lookup_function("math.add");
    assert!(func.is_ok(), "math.add should exist");
    assert_eq!(func.unwrap().schema.args.len(), 2);
}

#[test]
fn lookup_unknown_namespace() {
    let registry = make_registry_with_math();
    let result = registry.lookup_function("unknown.fn");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("namespace not found") || err.contains("unknown"),
        "{err}"
    );
}

#[test]
fn lookup_unknown_function_in_known_namespace() {
    let registry = make_registry_with_math();
    let result = registry.lookup_function("math.nonexistent");
    assert!(result.is_err());
}

#[test]
fn valid_call_passes_validation() {
    let registry = make_registry_with_math();
    let validator = InvocationValidator::new(registry);
    let env = Envelope::call("math.add", vec![Value::Int(1), Value::Int(2)]);
    assert!(validator.validate(&env).is_ok());
}

#[test]
fn wrong_arity_fails_validation() {
    let registry = make_registry_with_math();
    let validator = InvocationValidator::new(registry);

    // too few args
    let env_few = Envelope::call("math.add", vec![Value::Int(1)]);
    let err = validator.validate(&env_few).unwrap_err();
    assert!(matches!(err, ValidationError::ArgumentArity { .. }));
    assert_eq!(err.error_code(), ErrorCode::InvalidArguments);

    // too many args
    let env_many = Envelope::call(
        "math.add",
        vec![Value::Int(1), Value::Int(2), Value::Int(3)],
    );
    let err = validator.validate(&env_many).unwrap_err();
    assert!(matches!(err, ValidationError::ArgumentArity { .. }));
}

#[test]
fn wrong_type_fails_validation() {
    let registry = make_registry_with_math();
    let validator = InvocationValidator::new(registry);

    // "hello" is not an integer
    let env = Envelope::call(
        "math.add",
        vec![Value::String("hello".into()), Value::Int(2)],
    );
    let err = validator.validate(&env).unwrap_err();
    assert!(
        matches!(err, ValidationError::ArgumentType { .. }),
        "expected ArgumentType, got {err:?}"
    );
    assert_eq!(err.error_code(), ErrorCode::InvalidArguments);
}

#[test]
fn internal_visibility_denied_for_external_callers() {
    let registry = make_registry_with_math();
    let validator = InvocationValidator::new(registry);

    let env = Envelope::call("math.internal_op", vec![]);
    let err = validator.validate(&env).unwrap_err();
    assert!(
        matches!(err, ValidationError::VisibilityDenied { .. }),
        "expected VisibilityDenied, got {err:?}"
    );
    assert_eq!(err.error_code(), ErrorCode::CapabilityDenied);
}

#[test]
fn private_function_denied_for_external_callers() {
    let registry = make_registry_with_math();
    let validator = InvocationValidator::new(registry);

    let env = Envelope::call("math.secret", vec![]);
    let err = validator.validate(&env).unwrap_err();
    assert!(
        matches!(err, ValidationError::VisibilityDenied { .. }),
        "expected VisibilityDenied for private fn, got {err:?}"
    );
}

#[test]
fn batch_with_no_items_fails() {
    let registry = make_registry_with_math();
    let validator = InvocationValidator::new(registry);

    let mut env = Envelope::call("", vec![]);
    env.invocation_type = InvocationType::Batch;
    env.target = String::new();
    env.batch_items = None;

    let err = validator.validate(&env).unwrap_err();
    assert!(
        matches!(err, ValidationError::EmptyBatch),
        "expected EmptyBatch, got {err:?}"
    );
    assert_eq!(err.error_code(), ErrorCode::MalformedEnvelope);
}

#[test]
fn batch_with_empty_items_fails() {
    let registry = make_registry_with_math();
    let validator = InvocationValidator::new(registry);

    let mut env = Envelope::call("", vec![]);
    env.invocation_type = InvocationType::Batch;
    env.target = String::new();
    env.batch_items = Some(vec![]);

    let err = validator.validate(&env).unwrap_err();
    assert!(matches!(err, ValidationError::EmptyBatch));
}

#[test]
fn malformed_target_without_dot_fails() {
    let registry = make_registry_with_math();
    let validator = InvocationValidator::new(registry);

    let env = Envelope::call("nofunctionpart", vec![]);
    let err = validator.validate(&env).unwrap_err();
    assert!(
        matches!(err, ValidationError::MalformedEnvelope(_)),
        "expected MalformedEnvelope, got {err:?}"
    );
}

#[test]
fn optional_argument_may_be_omitted() {
    // Register a function with one required and one optional argument.
    let registry = SchemaRegistry::new();
    let mut functions = HashMap::new();
    functions.insert(
        "greet".into(),
        FunctionSchema {
            args: vec![
                ArgumentDescriptor {
                    name: "name".into(),
                    r#type: TypeDescriptor::primitive(PrimitiveType::String),
                    optional: false,
                    default: None,
                    doc: None,
                },
                ArgumentDescriptor {
                    name: "greeting".into(),
                    r#type: TypeDescriptor::primitive(PrimitiveType::String),
                    optional: true,
                    default: Some(Value::String("Hello".into())),
                    doc: None,
                },
            ],
            returns: TypeDescriptor::primitive(PrimitiveType::String),
            visibility: Visibility::Public,
            capabilities: vec![],
            idempotent: false,
            doc: None,
        },
    );
    registry
        .register(NamespaceRegistration {
            namespace: "greet".into(),
            schema: NamespaceSchema {
                functions,
                doc: None,
            },
            provider_id: "p".into(),
        })
        .unwrap();

    let validator = InvocationValidator::new(registry);
    // Providing only the required argument should pass.
    let env = Envelope::call("greet.greet", vec![Value::String("Alice".into())]);
    assert!(
        validator.validate(&env).is_ok(),
        "one-arg call to two-arg fn (second optional) should pass"
    );
}
