//! `saikuro_schema::builder` ergonomic schema-construction API.

use crate::check_test;
use crate::shared_test;
use crate::TestSuite;
use saikuro_core::CapabilityToken;
use saikuro_schema::builder::{
    build_schema, ArgDescriptor, FunctionSchema, NamespaceSchema, TypeDescriptor, Visibility,
};

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "schema::builder_empty_namespace_converts_to_empty_core",
        builder_empty_namespace_converts_to_empty_core,
    );
    shared_test!(
        suite,
        "schema::builder_missing_returns_defaults_to_any",
        builder_missing_returns_defaults_to_any,
    );
    shared_test!(
        suite,
        "schema::builder_default_visibility_is_public",
        builder_default_visibility_is_public,
    );
    shared_test!(
        suite,
        "schema::builder_function_fields_are_preserved",
        builder_function_fields_are_preserved,
    );
    shared_test!(
        suite,
        "schema::builder_arg_descriptor_roundtrips",
        builder_arg_descriptor_roundtrips,
    );
    shared_test!(
        suite,
        "schema::builder_build_schema_collects_namespaces",
        builder_build_schema_collects_namespaces,
    );
}

fn builder_empty_namespace_converts_to_empty_core() -> Result<(), &'static str> {
    let ns = NamespaceSchema::new();
    let core = ns.to_core().map_err(|_| "to_core")?;
    check_test!(core.functions.is_empty(), "empty builder ns stays empty");
    check_test!(core.doc.is_none(), "doc stays absent");
    Ok(())
}

fn builder_missing_returns_defaults_to_any() -> Result<(), &'static str> {
    let mut ns = NamespaceSchema::new();
    ns.insert("anything", FunctionSchema::default());
    let core = ns.to_core().map_err(|_| "to_core")?;
    let f = core.functions.get("anything").ok_or("missing function")?;
    check_test!(
        f.returns == TypeDescriptor::primitive(saikuro_core::schema::PrimitiveType::Any),
        "unset return type must default to Any"
    );
    check_test!(
        f.visibility == Visibility::Public,
        "unset visibility must default to Public"
    );
    check_test!(f.args.is_empty(), "no args by default");
    check_test!(!f.idempotent, "not idempotent by default");
    check_test!(f.doc.is_none(), "no doc by default");
    Ok(())
}

fn builder_default_visibility_is_public() -> Result<(), &'static str> {
    check_test!(
        FunctionSchema::default().visibility == Visibility::Public,
        "default visibility must be Public"
    );
    Ok(())
}

fn builder_function_fields_are_preserved() -> Result<(), &'static str> {
    let mut ns = NamespaceSchema::new();
    ns.insert(
        "guard",
        FunctionSchema {
            doc: Some("guarded op".into()),
            idempotent: true,
            capabilities: crate::vec!["admin".into(), "audit".into()],
            args: crate::vec![],
            returns: Some(TypeDescriptor::primitive(
                saikuro_core::schema::PrimitiveType::String,
            )),
            visibility: Visibility::Internal,
        },
    );
    let core = ns.to_core().map_err(|_| "to_core")?;
    let f = core.functions.get("guard").ok_or("missing function")?;
    check_test!(f.doc.as_deref() == Some("guarded op"), "doc preserved");
    check_test!(f.idempotent, "idempotent preserved");
    check_test!(
        f.capabilities
            == crate::vec![
                CapabilityToken::from("admin"),
                CapabilityToken::from("audit")
            ],
        "capabilities must map to capability tokens"
    );
    check_test!(f.visibility == Visibility::Internal, "visibility preserved");
    check_test!(
        f.returns == TypeDescriptor::primitive(saikuro_core::schema::PrimitiveType::String),
        "explicit return type preserved"
    );
    Ok(())
}

fn builder_arg_descriptor_roundtrips() -> Result<(), &'static str> {
    let mut ns = NamespaceSchema::new();
    ns.insert(
        "greet",
        FunctionSchema {
            doc: None,
            idempotent: false,
            capabilities: crate::vec![],
            args: crate::vec![ArgDescriptor {
                name: "name".into(),
                r#type: TypeDescriptor::primitive(saikuro_core::schema::PrimitiveType::String),
                optional: true,
                doc: Some("who to greet".into()),
            }],
            returns: None,
            visibility: Visibility::Public,
        },
    );
    let core = ns.to_core().map_err(|_| "to_core")?;
    let f = core.functions.get("greet").ok_or("missing function")?;
    check_test!(f.args.len() == 1, "one arg expected");
    let arg = &f.args[0];
    check_test!(arg.name == "name", "arg name preserved");
    check_test!(arg.optional, "optional flag preserved");
    check_test!(
        arg.doc.as_deref() == Some("who to greet"),
        "arg doc preserved"
    );
    check_test!(
        arg.r#type == TypeDescriptor::primitive(saikuro_core::schema::PrimitiveType::String),
        "arg type preserved"
    );
    Ok(())
}

fn builder_build_schema_collects_namespaces() -> Result<(), &'static str> {
    let mut math = NamespaceSchema::new();
    math.insert("add", FunctionSchema::default());
    let mut meta = NamespaceSchema::new();
    meta.doc = Some("metadata".into());
    meta.insert("version", FunctionSchema::default());

    let mut map = crate::BTreeMap::new();
    map.insert("math".into(), math);
    map.insert("meta".into(), meta);

    let schema = build_schema(&map).map_err(|_| "build_schema")?;
    check_test!(schema.namespaces.len() == 2, "both namespaces collected");
    let math_ns = schema
        .namespaces
        .get("math")
        .ok_or("math namespace missing")?;
    check_test!(math_ns.functions.get("add").is_some(), "add present");
    let meta_ns = schema
        .namespaces
        .get("meta")
        .ok_or("meta namespace missing")?;
    check_test!(
        meta_ns.doc.as_deref() == Some("metadata"),
        "namespace doc preserved"
    );
    Ok(())
}
