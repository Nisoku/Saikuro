use crate::shared_test;
use crate::TestSuite;
use crate::Vec;
use saikuro_core::capability::{CapabilitySet, CapabilityToken};
use saikuro_core::schema::{FunctionSchema, PrimitiveType, TypeDescriptor, Visibility};
use saikuro_schema::capability_engine::{CapabilityEngine, CapabilityOutcome};

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "schema::empty_set_denies_required",
        empty_set_denies_required,
    );
    shared_test!(suite, "schema::exact_token_grants", exact_token_grants);
    shared_test!(
        suite,
        "schema::wildcard_grants_everything",
        wildcard_grants_everything,
    );
    shared_test!(
        suite,
        "schema::grants_all_requires_every_token",
        grants_all_requires_every_token,
    );
    shared_test!(
        suite,
        "schema::empty_set_satisfies_no_requirements",
        empty_set_satisfies_no_requirements,
    );
    shared_test!(
        suite,
        "schema::engine_grants_no_required_caps",
        engine_grants_no_required_caps,
    );
    shared_test!(
        suite,
        "schema::engine_grants_when_holds_required",
        engine_grants_when_holds_required,
    );
    shared_test!(
        suite,
        "schema::engine_denies_when_missing",
        engine_denies_when_missing,
    );
    shared_test!(
        suite,
        "schema::engine_denies_first_missing",
        engine_denies_first_missing,
    );
    shared_test!(
        suite,
        "schema::engine_sandboxed_still_checks",
        engine_sandboxed_still_checks,
    );
    shared_test!(
        suite,
        "schema::cap_token_display_and_eq",
        cap_token_display_and_eq
    );
    shared_test!(
        suite,
        "schema::cap_set_insert_and_len",
        cap_set_insert_and_len
    );
    shared_test!(
        suite,
        "schema::engine_grants_with_all_powerful_set",
        engine_grants_with_all_powerful_set,
    );
    shared_test!(
        suite,
        "schema::filter_accessible_functions_respects_caps",
        filter_accessible_functions_respects_caps,
    );
    shared_test!(
        suite,
        "schema::filter_accessible_functions_all_with_wildcard",
        filter_accessible_functions_all_with_wildcard,
    );
    shared_test!(
        suite,
        "schema::capability_set_iter_contains_all_tokens",
        capability_set_iter_contains_all_tokens,
    );
}

fn make_function_with_caps(caps: Vec<CapabilityToken>) -> FunctionSchema {
    FunctionSchema {
        args: vec![],
        returns: TypeDescriptor::primitive(PrimitiveType::Unit),
        visibility: Visibility::Public,
        capabilities: caps,
        idempotent: false,
        doc: None,
    }
}

fn empty_set_denies_required() -> Result<(), &'static str> {
    let engine = CapabilityEngine::default();
    let caller = CapabilitySet::empty();
    let schema = make_function_with_caps(vec![CapabilityToken::new("admin")]);
    assert!(matches!(
        engine.check(&caller, &schema),
        CapabilityOutcome::Denied { .. }
    ));
    Ok(())
}

fn exact_token_grants() -> Result<(), &'static str> {
    let engine = CapabilityEngine::default();
    let caller = CapabilitySet::from_tokens([CapabilityToken::new("admin")]).map_err(|_| "caps")?;
    let schema = make_function_with_caps(vec![CapabilityToken::new("admin")]);
    assert!(matches!(
        engine.check(&caller, &schema),
        CapabilityOutcome::Granted
    ));
    Ok(())
}

fn wildcard_grants_everything() -> Result<(), &'static str> {
    let engine = CapabilityEngine::default();
    let caller = CapabilitySet::all_powerful();
    let schema = make_function_with_caps(vec![CapabilityToken::new("anything")]);
    assert!(matches!(
        engine.check(&caller, &schema),
        CapabilityOutcome::Granted
    ));
    Ok(())
}

fn grants_all_requires_every_token() -> Result<(), &'static str> {
    let engine = CapabilityEngine::default();
    let caller = CapabilitySet::from_tokens([CapabilityToken::new("a"), CapabilityToken::new("b")])
        .map_err(|_| "caps")?;
    let schema =
        make_function_with_caps(vec![CapabilityToken::new("a"), CapabilityToken::new("b")]);
    assert!(matches!(
        engine.check(&caller, &schema),
        CapabilityOutcome::Granted
    ));
    let schema2 =
        make_function_with_caps(vec![CapabilityToken::new("a"), CapabilityToken::new("c")]);
    assert!(matches!(
        engine.check(&caller, &schema2),
        CapabilityOutcome::Denied { .. }
    ));
    Ok(())
}

fn empty_set_satisfies_no_requirements() -> Result<(), &'static str> {
    let engine = CapabilityEngine::default();
    let caller = CapabilitySet::empty();
    let schema = make_function_with_caps(vec![]);
    assert!(matches!(
        engine.check(&caller, &schema),
        CapabilityOutcome::Granted
    ));
    Ok(())
}

fn engine_grants_no_required_caps() -> Result<(), &'static str> {
    let engine = CapabilityEngine::default();
    let caller = CapabilitySet::empty();
    let schema = make_function_with_caps(vec![]);
    assert!(matches!(
        engine.check(&caller, &schema),
        CapabilityOutcome::Granted
    ));
    Ok(())
}

fn engine_grants_when_holds_required() -> Result<(), &'static str> {
    let engine = CapabilityEngine::default();
    let caller = CapabilitySet::from_tokens([CapabilityToken::new("read")]).map_err(|_| "caps")?;
    let schema = make_function_with_caps(vec![CapabilityToken::new("read")]);
    assert!(matches!(
        engine.check(&caller, &schema),
        CapabilityOutcome::Granted
    ));
    Ok(())
}

fn engine_denies_when_missing() -> Result<(), &'static str> {
    let engine = CapabilityEngine::default();
    let caller = CapabilitySet::empty();
    let schema = make_function_with_caps(vec![CapabilityToken::new("write")]);
    assert!(matches!(
        engine.check(&caller, &schema),
        CapabilityOutcome::Denied { .. }
    ));
    Ok(())
}

fn engine_denies_first_missing() -> Result<(), &'static str> {
    let engine = CapabilityEngine::default();
    let caller = CapabilitySet::from_tokens([CapabilityToken::new("a")]).map_err(|_| "caps")?;
    let schema =
        make_function_with_caps(vec![CapabilityToken::new("a"), CapabilityToken::new("b")]);
    assert!(matches!(
        engine.check(&caller, &schema),
        CapabilityOutcome::Denied { .. }
    ));
    Ok(())
}

fn engine_sandboxed_still_checks() -> Result<(), &'static str> {
    let engine = CapabilityEngine::sandboxed();
    let caller = CapabilitySet::from_tokens([CapabilityToken::new("x")]).map_err(|_| "caps")?;
    let schema = make_function_with_caps(vec![CapabilityToken::new("x")]);
    assert!(matches!(
        engine.check(&caller, &schema),
        CapabilityOutcome::Granted
    ));
    let caller2 = CapabilitySet::empty();
    assert!(matches!(
        engine.check(&caller2, &schema),
        CapabilityOutcome::Denied { .. }
    ));
    Ok(())
}

fn cap_token_display_and_eq() -> Result<(), &'static str> {
    let t1 = CapabilityToken::new("admin");
    let t2 = CapabilityToken::new("admin");
    let t3 = CapabilityToken::new("user");
    assert_eq!(t1, t2);
    assert_ne!(t1, t3);
    let s = alloc::format!("{}", t1);
    assert!(s.contains("admin"));
    Ok(())
}

fn cap_set_insert_and_len() -> Result<(), &'static str> {
    let mut set = CapabilitySet::empty();
    set.insert(CapabilityToken::new("a"))
        .map_err(|_| "insert a")?;
    set.insert(CapabilityToken::new("b"))
        .map_err(|_| "insert b")?;
    set.insert(CapabilityToken::new("a"))
        .map_err(|_| "insert a dup")?;
    assert_eq!(set.len(), 2);
    Ok(())
}

fn engine_grants_with_all_powerful_set() -> Result<(), &'static str> {
    let engine = CapabilityEngine::default();
    let schema = make_function_with_caps(vec![
        CapabilityToken::new("super.secret"),
        CapabilityToken::new("another.cap"),
    ]);
    assert!(matches!(
        engine.check(&CapabilitySet::all_powerful(), &schema),
        CapabilityOutcome::Granted
    ));
    Ok(())
}

fn filter_accessible_functions_respects_caps() -> Result<(), &'static str> {
    let engine = CapabilityEngine::default();

    let public_fn = make_function_with_caps(vec![]);
    let protected_fn = make_function_with_caps(vec![CapabilityToken::new("admin")]);

    let functions = vec![("public_op", &public_fn), ("admin_op", &protected_fn)];

    let caps = CapabilitySet::empty();
    let accessible = engine.filter_accessible_functions(functions.into_iter(), &caps);
    assert_eq!(accessible, vec!["public_op"]);
    Ok(())
}

fn filter_accessible_functions_all_with_wildcard() -> Result<(), &'static str> {
    let engine = CapabilityEngine::default();

    let public_fn = make_function_with_caps(vec![]);
    let protected_fn = make_function_with_caps(vec![CapabilityToken::new("admin")]);

    let functions = vec![("public_op", &public_fn), ("admin_op", &protected_fn)];

    let caps = CapabilitySet::all_powerful();
    let accessible = engine.filter_accessible_functions(functions.into_iter(), &caps);
    assert_eq!(accessible, vec!["public_op", "admin_op"]);
    Ok(())
}

fn capability_set_iter_contains_all_tokens() -> Result<(), &'static str> {
    let tokens = crate::Vec::from([
        CapabilityToken::new("x"),
        CapabilityToken::new("y"),
        CapabilityToken::new("z"),
    ]);
    let set = CapabilitySet::from_tokens(tokens.clone()).map_err(|_| "from_tokens")?;
    let collected: crate::Vec<_> = set.iter().cloned().collect();
    assert_eq!(collected.len(), tokens.len());
    for t in &tokens {
        assert!(collected.contains(t), "set must contain {} ", t);
    }
    Ok(())
}
