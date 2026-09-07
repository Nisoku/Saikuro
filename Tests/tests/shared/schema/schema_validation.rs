
use crate::common;
use crate::Box;
use crate::shared_test;
use crate::TestSuite;
use saikuro_core::envelope::Envelope;
use saikuro_event::{ErrorCode, Value};
use saikuro_schema::registry::SchemaRegistry;
use saikuro_schema::validator::InvocationValidator;

pub fn register(suite: &mut TestSuite) {
    shared_test!(suite, "schema::lookup_existing_function", lookup_existing_function);
    shared_test!(suite, "schema::lookup_unknown_namespace", lookup_unknown_namespace);
    shared_test!(suite,
        "schema::lookup_unknown_function_in_known_ns",
        lookup_unknown_function_in_known_ns,
    );
    shared_test!(suite,
        "schema::valid_call_passes_validation",
        valid_call_passes_validation,
    );
    shared_test!(suite, "schema::wrong_arity_fails", wrong_arity_fails);
    shared_test!(suite, "schema::wrong_type_fails", wrong_type_fails);
    shared_test!(suite,
        "schema::internal_visibility_denied",
        internal_visibility_denied,
    );
    shared_test!(suite, "schema::private_function_denied", private_function_denied);
    shared_test!(suite, "schema::batch_no_items_fails", batch_no_items_fails);
    shared_test!(suite, "schema::batch_empty_items_fails", batch_empty_items_fails);
    shared_test!(suite, "schema::malformed_target_no_dot", malformed_target_no_dot);
    shared_test!(suite,
        "schema::optional_argument_may_be_omitted",
        optional_argument_may_be_omitted,
    );
}

fn lookup_existing_function() -> Result<(), &'static str> {
    crate::block_on(async {
        let reg = SchemaRegistry::new();
        common::register_namespace(&reg, "math", "add").await;
        let val = InvocationValidator::new(reg);
        let env = Envelope::call("math.add", vec![]).map_err(|_| "create")?;
        val.validate(&env).await.map_err(|_| "validate failed")?;
        Ok::<(), &'static str>(())
    })
}

fn lookup_unknown_namespace() -> Result<(), &'static str> {
    crate::block_on(async {
        let reg = SchemaRegistry::new();
        let val = InvocationValidator::new(reg);
        let env = Envelope::call("nope.fn", vec![]).map_err(|_| "create")?;
        match val.validate(&env).await {
            Ok(_) => Err("should have failed"),
            Err(e) => {
                assert_eq!(e.error_code(), ErrorCode::NamespaceNotFound);
                Ok(())
            }
        }
    })
}

fn lookup_unknown_function_in_known_ns() -> Result<(), &'static str> {
    crate::block_on(async {
        let reg = SchemaRegistry::new();
        common::register_namespace(&reg, "math", "add").await;
        let val = InvocationValidator::new(reg);
        let env = Envelope::call("math.sub", vec![]).map_err(|_| "create")?;
        match val.validate(&env).await {
            Ok(_) => Err("should have failed"),
            Err(e) => {
                assert_eq!(e.error_code(), ErrorCode::FunctionNotFound);
                Ok(())
            }
        }
    })
}

fn valid_call_passes_validation() -> Result<(), &'static str> {
    crate::block_on(async {
        let reg = SchemaRegistry::new();
        common::register_namespace(&reg, "svc", "run").await;
        let val = InvocationValidator::new(reg);
        let env = Envelope::call("svc.run", vec![]).map_err(|_| "create")?;
        val.validate(&env).await.map_err(|_| "should pass")?;
        Ok(())
    })
}

fn wrong_arity_fails() -> Result<(), &'static str> {
    crate::block_on(async {
        let mut functions = saikuro_core::schema::FunctionMap::new();
        let _ = functions.insert(
            "add".into(),
            saikuro_core::schema::FunctionSchema {
                args: vec![saikuro_core::schema::ArgumentDescriptor {
                    name: "x".into(),
                    r#type: saikuro_core::schema::TypeDescriptor::primitive(
                        saikuro_core::schema::PrimitiveType::I64,
                    ),
                    optional: false,
                    default: None,
                    doc: None,
                }],
                returns: saikuro_core::schema::TypeDescriptor::primitive(
                    saikuro_core::schema::PrimitiveType::I64,
                ),
                visibility: saikuro_core::schema::Visibility::Public,
                capabilities: vec![],
                idempotent: false,
                doc: None,
            },
        );
        let mut namespaces = saikuro_core::schema::NamespaceMap::new();
        let _ = namespaces.insert(
            "math".into(),
            saikuro_core::schema::NamespaceSchema {
                functions: Box::new(functions),
                doc: None,
            },
        );
        let schema = saikuro_core::schema::Schema {
            version: 1,
            namespaces: Box::new(namespaces),
            types: Box::new(saikuro_core::schema::TypeMap::new()),
        };
        let reg = SchemaRegistry::new();
        reg.merge_schema(schema, "test")
            .await
            .map_err(|_| "merge")?;
        let val = InvocationValidator::new(reg);
        let env = Envelope::call("math.add", vec![]).map_err(|_| "create")?;
        match val.validate(&env).await {
            Ok(_) => Err("should fail on wrong arity"),
            Err(e) => {
                assert_eq!(e.error_code(), ErrorCode::InvalidArguments);
                Ok(())
            }
        }
    })
}

fn wrong_type_fails() -> Result<(), &'static str> {
    crate::block_on(async {
        let mut functions = saikuro_core::schema::FunctionMap::new();
        let _ = functions.insert(
            "greet".into(),
            saikuro_core::schema::FunctionSchema {
                args: vec![saikuro_core::schema::ArgumentDescriptor {
                    name: "name".into(),
                    r#type: saikuro_core::schema::TypeDescriptor::primitive(
                        saikuro_core::schema::PrimitiveType::String,
                    ),
                    optional: false,
                    default: None,
                    doc: None,
                }],
                returns: saikuro_core::schema::TypeDescriptor::primitive(
                    saikuro_core::schema::PrimitiveType::Unit,
                ),
                visibility: saikuro_core::schema::Visibility::Public,
                capabilities: vec![],
                idempotent: false,
                doc: None,
            },
        );
        let mut namespaces = saikuro_core::schema::NamespaceMap::new();
        let _ = namespaces.insert(
            "greeter".into(),
            saikuro_core::schema::NamespaceSchema {
                functions: Box::new(functions),
                doc: None,
            },
        );
        let schema = saikuro_core::schema::Schema {
            version: 1,
            namespaces: Box::new(namespaces),
            types: Box::new(saikuro_core::schema::TypeMap::new()),
        };
        let reg = SchemaRegistry::new();
        reg.merge_schema(schema, "test")
            .await
            .map_err(|_| "merge")?;
        let val = InvocationValidator::new(reg);
        let env = Envelope::call("greeter.greet", vec![Value::Int(42)]).map_err(|_| "create")?;
        match val.validate(&env).await {
            Ok(_) => Err("should fail on wrong type"),
            Err(e) => {
                assert_eq!(e.error_code(), ErrorCode::InvalidArguments);
                Ok(())
            }
        }
    })
}

fn internal_visibility_denied() -> Result<(), &'static str> {
    crate::block_on(async {
        let mut functions = saikuro_core::schema::FunctionMap::new();
        let _ = functions.insert(
            "secret".into(),
            saikuro_core::schema::FunctionSchema {
                args: vec![],
                returns: saikuro_core::schema::TypeDescriptor::primitive(
                    saikuro_core::schema::PrimitiveType::Unit,
                ),
                visibility: saikuro_core::schema::Visibility::Internal,
                capabilities: vec![],
                idempotent: false,
                doc: None,
            },
        );
        let mut namespaces = saikuro_core::schema::NamespaceMap::new();
        let _ = namespaces.insert(
            "priv".into(),
            saikuro_core::schema::NamespaceSchema {
                functions: Box::new(functions),
                doc: None,
            },
        );
        let schema = saikuro_core::schema::Schema {
            version: 1,
            namespaces: Box::new(namespaces),
            types: Box::new(saikuro_core::schema::TypeMap::new()),
        };
        let reg = SchemaRegistry::new();
        reg.merge_schema(schema, "test")
            .await
            .map_err(|_| "merge")?;
        let val = InvocationValidator::new(reg);
        let env = Envelope::call("priv.secret", vec![]).map_err(|_| "create")?;
        match val.validate(&env).await {
            Ok(_) => Err("should deny internal"),
            Err(e) => {
                assert_eq!(e.error_code(), ErrorCode::CapabilityDenied);
                Ok(())
            }
        }
    })
}

fn private_function_denied() -> Result<(), &'static str> {
    crate::block_on(async {
        let mut functions = saikuro_core::schema::FunctionMap::new();
        let _ = functions.insert(
            "hidden".into(),
            saikuro_core::schema::FunctionSchema {
                args: vec![],
                returns: saikuro_core::schema::TypeDescriptor::primitive(
                    saikuro_core::schema::PrimitiveType::Unit,
                ),
                visibility: saikuro_core::schema::Visibility::Private,
                capabilities: vec![],
                idempotent: false,
                doc: None,
            },
        );
        let mut namespaces = saikuro_core::schema::NamespaceMap::new();
        let _ = namespaces.insert(
            "priv".into(),
            saikuro_core::schema::NamespaceSchema {
                functions: Box::new(functions),
                doc: None,
            },
        );
        let schema = saikuro_core::schema::Schema {
            version: 1,
            namespaces: Box::new(namespaces),
            types: Box::new(saikuro_core::schema::TypeMap::new()),
        };
        let reg = SchemaRegistry::new();
        reg.merge_schema(schema, "test")
            .await
            .map_err(|_| "merge")?;
        let val = InvocationValidator::new(reg);
        let env = Envelope::call("priv.hidden", vec![]).map_err(|_| "create")?;
        match val.validate(&env).await {
            Ok(_) => Err("should deny private"),
            Err(e) => {
                assert_eq!(e.error_code(), ErrorCode::CapabilityDenied);
                Ok(())
            }
        }
    })
}

fn batch_no_items_fails() -> Result<(), &'static str> {
    crate::block_on(async {
        let reg = SchemaRegistry::new();
        let val = InvocationValidator::new(reg);
        let mut env = Envelope::call("$saikuro.batch", vec![]).map_err(|_| "create")?;
        env.invocation_type = saikuro_core::envelope::InvocationType::Batch;
        env.batch_items = Some(vec![]);
        match val.validate(&env).await {
            Ok(_) => Err("should fail"),
            Err(e) => {
                assert_eq!(e.error_code(), ErrorCode::MalformedEnvelope);
                Ok(())
            }
        }
    })
}

fn batch_empty_items_fails() -> Result<(), &'static str> {
    crate::block_on(async {
        let reg = SchemaRegistry::new();
        let val = InvocationValidator::new(reg);
        let mut env = Envelope::call("$saikuro.batch", vec![]).map_err(|_| "create")?;
        env.invocation_type = saikuro_core::envelope::InvocationType::Batch;
        env.batch_items = Some(vec![]);
        match val.validate(&env).await {
            Ok(_) => Err("should fail"),
            Err(e) => {
                assert_eq!(e.error_code(), ErrorCode::MalformedEnvelope);
                Ok(())
            }
        }
    })
}

fn malformed_target_no_dot() -> Result<(), &'static str> {
    crate::block_on(async {
        let reg = SchemaRegistry::new();
        let val = InvocationValidator::new(reg);
        let env = Envelope::call("nodothere", vec![]).map_err(|_| "create")?;
        match val.validate(&env).await {
            Ok(_) => Err("should fail"),
            Err(e) => {
                assert!(
                    e.error_code() == ErrorCode::MalformedTarget
                        || e.error_code() == ErrorCode::MalformedEnvelope
                        || e.error_code() == ErrorCode::NamespaceNotFound,
                    "unexpected {:?}",
                    e.error_code()
                );
                Ok(())
            }
        }
    })
}

fn optional_argument_may_be_omitted() -> Result<(), &'static str> {
    crate::block_on(async {
        let mut functions = saikuro_core::schema::FunctionMap::new();
        let _ = functions.insert(
            "greet".into(),
            saikuro_core::schema::FunctionSchema {
                args: vec![saikuro_core::schema::ArgumentDescriptor {
                    name: "name".into(),
                    r#type: saikuro_core::schema::TypeDescriptor::primitive(
                        saikuro_core::schema::PrimitiveType::String,
                    ),
                    optional: true,
                    default: Some(Value::String("world".into())),
                    doc: None,
                }],
                returns: saikuro_core::schema::TypeDescriptor::primitive(
                    saikuro_core::schema::PrimitiveType::Unit,
                ),
                visibility: saikuro_core::schema::Visibility::Public,
                capabilities: vec![],
                idempotent: false,
                doc: None,
            },
        );
        let mut namespaces = saikuro_core::schema::NamespaceMap::new();
        let _ = namespaces.insert(
            "greeter".into(),
            saikuro_core::schema::NamespaceSchema {
                functions: Box::new(functions),
                doc: None,
            },
        );
        let schema = saikuro_core::schema::Schema {
            version: 1,
            namespaces: Box::new(namespaces),
            types: Box::new(saikuro_core::schema::TypeMap::new()),
        };
        let reg = SchemaRegistry::new();
        reg.merge_schema(schema, "test")
            .await
            .map_err(|_| "merge")?;
        let val = InvocationValidator::new(reg);
        let env = Envelope::call("greeter.greet", vec![]).map_err(|_| "create")?;
        val.validate(&env)
            .await
            .map_err(|_| "optional arg should be ok")?;
        Ok(())
    })
}
