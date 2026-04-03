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

#[test]
fn empty_set_denies_required_cap() {
    let set = CapabilitySet::empty();
    assert!(!set.grants(&CapabilityToken::new("math.basic")));
}

#[test]
fn set_with_exact_token_grants() {
    let set = CapabilitySet::from_tokens([CapabilityToken::new("math.basic")]);
    assert!(set.grants(&CapabilityToken::new("math.basic")));
    assert!(!set.grants(&CapabilityToken::new("math.advanced")));
}

#[test]
fn wildcard_set_grants_everything() {
    let set = CapabilitySet::all_powerful();
    assert!(set.grants(&CapabilityToken::new("math.basic")));
    assert!(set.grants(&CapabilityToken::new("any.random.capability")));
    assert!(set.grants(&CapabilityToken::new("")));
}

#[test]
fn grants_all_requires_every_token() {
    let set =
        CapabilitySet::from_tokens([CapabilityToken::new("read"), CapabilityToken::new("write")]);
    let required = [CapabilityToken::new("read"), CapabilityToken::new("write")];
    assert!(set.grants_all(required.iter()));

    let missing_one = [
        CapabilityToken::new("read"),
        CapabilityToken::new("write"),
        CapabilityToken::new("admin"),
    ];
    assert!(!set.grants_all(missing_one.iter()));
}

#[test]
fn empty_set_satisfies_no_requirements() {
    let set = CapabilitySet::empty();
    let schema = fn_requiring(&["a", "b"]);
    let engine = CapabilityEngine::new();
    assert!(matches!(
        engine.check(&set, &schema),
        CapabilityOutcome::Denied { .. }
    ));
}

// CapabilityEngine

#[test]
fn engine_grants_function_with_no_required_caps() {
    let engine = CapabilityEngine::new();
    let schema = fn_no_caps();
    let result = engine.check(&CapabilitySet::empty(), &schema);
    assert!(
        matches!(result, CapabilityOutcome::Granted),
        "no required caps should always be granted"
    );
}

#[test]
fn engine_grants_when_caller_holds_required_cap() {
    let engine = CapabilityEngine::new();
    let schema = fn_requiring(&["data.read"]);
    let caps = CapabilitySet::from_tokens([CapabilityToken::new("data.read")]);
    assert!(matches!(
        engine.check(&caps, &schema),
        CapabilityOutcome::Granted
    ));
}

#[test]
fn engine_denies_when_caller_missing_cap() {
    let engine = CapabilityEngine::new();
    let schema = fn_requiring(&["data.write"]);
    let caps = CapabilitySet::from_tokens([CapabilityToken::new("data.read")]);
    let result = engine.check(&caps, &schema);
    match result {
        CapabilityOutcome::Denied { missing } => {
            assert_eq!(missing.as_str(), "data.write");
        }
        CapabilityOutcome::Granted => panic!("expected Denied, got Granted"),
    }
}

#[test]
fn engine_denies_on_first_missing_cap() {
    // Function requires both A and B; caller has only A.
    let engine = CapabilityEngine::new();
    let schema = fn_requiring(&["cap.a", "cap.b"]);
    let caps = CapabilitySet::from_tokens([CapabilityToken::new("cap.a")]);
    assert!(matches!(
        engine.check(&caps, &schema),
        CapabilityOutcome::Denied { .. }
    ));
}

#[test]
fn engine_grants_with_all_powerful_set() {
    let engine = CapabilityEngine::new();
    let schema = fn_requiring(&["super.secret", "another.cap"]);
    let result = engine.check(&CapabilitySet::all_powerful(), &schema);
    assert!(matches!(result, CapabilityOutcome::Granted));
}

#[test]
fn engine_sandboxed_still_checks_caps() {
    let engine = CapabilityEngine::sandboxed();
    let schema = fn_requiring(&["secret.op"]);
    let caps = CapabilitySet::empty();
    assert!(matches!(
        engine.check(&caps, &schema),
        CapabilityOutcome::Denied { .. }
    ));
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
fn capability_token_display_and_equality() {
    let t1 = CapabilityToken::new("ns.perm");
    let t2: CapabilityToken = "ns.perm".into();
    assert_eq!(t1, t2);
    assert_eq!(t1.to_string(), "ns.perm");
    assert_eq!(t1.as_str(), "ns.perm");
}

#[test]
fn capability_set_insert_and_len() {
    let mut set = CapabilitySet::empty();
    assert_eq!(set.len(), 0);
    assert!(set.is_empty());

    set.insert(CapabilityToken::new("a"));
    set.insert(CapabilityToken::new("b"));
    assert_eq!(set.len(), 2);
    assert!(!set.is_empty());

    // Duplicate insert should not grow the set.
    set.insert(CapabilityToken::new("a"));
    assert_eq!(set.len(), 2);
}

#[test]
fn capability_set_iter_contains_all_tokens() {
    let tokens = vec![
        CapabilityToken::new("x"),
        CapabilityToken::new("y"),
        CapabilityToken::new("z"),
    ];
    let set = CapabilitySet::from_tokens(tokens.clone());
    let collected: std::collections::HashSet<_> = set.iter().cloned().collect();
    for t in &tokens {
        assert!(collected.contains(t));
    }
}
