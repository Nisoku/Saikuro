//! Capability enforcement tests

use saikuro_core::{
    capability::{CapabilitySet, CapabilityToken},
    schema::{FunctionSchema, PrimitiveType, TypeDescriptor, Visibility},
};
use saikuro_schema::capability_engine::{CapabilityEngine, CapabilityOutcome};

// Helpers

fn fn_requiring(caps: &[&str]) -> FunctionSchema {
    FunctionSchema {
        args: vec![],
        returns: TypeDescriptor::primitive(PrimitiveType::Unit),
        visibility: Visibility::Public,
        capabilities: caps.iter().map(|s| CapabilityToken::new(*s)).collect(),
        idempotent: false,
        doc: None,
    }
}

fn fn_no_caps() -> FunctionSchema {
    fn_requiring(&[])
}

// CapabilitySet


// CapabilityEngine


#[test]
fn engine_grants_with_all_powerful_set() {
    let engine = CapabilityEngine::new();
    let schema = fn_requiring(&["super.secret", "another.cap"]);
    let result = engine.check(&CapabilitySet::all_powerful(), &schema);
    assert!(matches!(result, CapabilityOutcome::Granted));
}


#[test]
fn filter_accessible_functions_respects_caps() {
    let engine = CapabilityEngine::new();

    let public_fn = fn_no_caps();
    let protected_fn = fn_requiring(&["admin"]);

    let functions: Vec<(&str, &FunctionSchema)> =
        vec![("public_op", &public_fn), ("admin_op", &protected_fn)];

    let caps = CapabilitySet::empty();
    let accessible = engine.filter_accessible_functions(functions.into_iter(), &caps);
    assert_eq!(accessible, vec!["public_op"]);
}

#[test]
fn filter_accessible_functions_all_with_wildcard() {
    let engine = CapabilityEngine::new();

    let public_fn = fn_no_caps();
    let protected_fn = fn_requiring(&["admin"]);

    let functions: Vec<(&str, &FunctionSchema)> =
        vec![("public_op", &public_fn), ("admin_op", &protected_fn)];

    let caps = CapabilitySet::all_powerful();
    let mut accessible = engine.filter_accessible_functions(functions.into_iter(), &caps);
    accessible.sort(); // DashMap iteration order is non-deterministic
    assert_eq!(accessible, vec!["admin_op", "public_op"]);
}


#[test]
fn capability_set_iter_contains_all_tokens() {
    let tokens = vec![
        CapabilityToken::new("x"),
        CapabilityToken::new("y"),
        CapabilityToken::new("z"),
    ];
    let set = CapabilitySet::from_tokens(tokens.clone()).unwrap();
    let collected: std::collections::HashSet<_> = set.iter().cloned().collect();
    for t in &tokens {
        assert!(collected.contains(t));
    }
}
