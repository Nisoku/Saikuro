use crate::shared_test;
use crate::TestSuite;
use saikuro_core::envelope::Envelope;
use saikuro_core::schema::{FunctionMap, FunctionSchema, Schema};
use saikuro_core::{InvocationType, PROTOCOL_VERSION};
use saikuro_event::Value;
use saikuro_schema::registry::SchemaRegistry;

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "core::envelope_target_accessors",
        envelope_target_accessors,
    );
    shared_test!(
        suite,
        "core::envelope_target_accessors_invalid",
        envelope_target_accessors_invalid,
    );
    shared_test!(
        suite,
        "core::envelope_meta_capacity_overflow_errors",
        envelope_meta_capacity_overflow_errors,
    );
    shared_test!(
        suite,
        "core::envelope_meta_roundtrips_through_wire",
        envelope_meta_roundtrips_through_wire,
    );
    shared_test!(
        suite,
        "core::envelope_from_msgpack_typed_decodes_schema",
        envelope_from_msgpack_typed_decodes_schema,
    );
}

fn envelope_target_accessors() -> Result<(), &'static str> {
    let env = Envelope::call("math.add", crate::vec![]).map_err(|_| "create envelope")?;
    crate::check_test!(
        env.namespace() == Some("math"),
        "namespace() must return the portion before the last dot"
    );
    crate::check_test!(
        env.function_name() == Some("add"),
        "function_name() must return the portion after the last dot"
    );

    let dotted = Envelope::call("a.b.c", crate::vec![]).map_err(|_| "create envelope")?;
    crate::check_test!(
        dotted.namespace() == Some("a.b"),
        "namespace() must split on the last dot, not the first"
    );
    crate::check_test!(
        dotted.function_name() == Some("c"),
        "function_name() must take everything after the last dot"
    );
    Ok(())
}

fn envelope_target_accessors_invalid() -> Result<(), &'static str> {
    let no_dot = Envelope::call("nofunction", crate::vec![]).map_err(|_| "create envelope")?;
    crate::check_test!(
        no_dot.namespace().is_none(),
        "a target without a dot must not produce a namespace"
    );
    crate::check_test!(
        no_dot.function_name().is_none(),
        "a target without a dot must not produce a function name"
    );
    Ok(())
}

fn envelope_meta_capacity_overflow_errors() -> Result<(), &'static str> {
    let mut env = Envelope::call("ns.fn", crate::vec![]).map_err(|_| "create envelope")?;
    for i in 0..saikuro_core::envelope::ENVELOPE_META_CAPACITY {
        env.meta_mut()
            .insert(crate::format!("k{i}"), Value::Int(i as i64))
            .map_err(|_| "at-capacity insert must succeed")?;
    }
    crate::check_test!(
        env.meta_mut()
            .insert("overflow".into(), Value::Null)
            .is_err(),
        "one entry past the metadata capacity must error"
    );
    crate::check_test!(
        env.meta().expect("meta present").len() == saikuro_core::envelope::ENVELOPE_META_CAPACITY,
        "the overflowing entry must not be inserted"
    );
    Ok(())
}

fn envelope_meta_roundtrips_through_wire() -> Result<(), &'static str> {
    let mut env = Envelope::call("ns.fn", crate::vec![]).map_err(|_| "create envelope")?;
    for i in 0..saikuro_core::envelope::ENVELOPE_META_CAPACITY {
        env.meta_mut()
            .insert(crate::format!("k{i}"), Value::Int(i as i64))
            .map_err(|_| "at-capacity insert must succeed")?;
    }
    let bytes = env.to_msgpack().map_err(|_| "to_msgpack")?;
    let decoded = Envelope::from_msgpack(&bytes).map_err(|_| "from_msgpack")?;
    crate::check_test!(
        decoded.meta().expect("meta present").len()
            == saikuro_core::envelope::ENVELOPE_META_CAPACITY,
        "a full meta bag must survive a wire roundtrip"
    );
    crate::check_test!(
        decoded.meta().expect("meta present").get("k15") == Some(&Value::Int(15)),
        "the last metadata entry must survive the wire"
    );
    Ok(())
}

fn envelope_from_msgpack_typed_decodes_schema() -> Result<(), &'static str> {
    crate::block_on(async {
        let mut schema = Schema::new();
        let mut functions = FunctionMap::new();
        functions.insert(
            "negate".into(),
            FunctionSchema {
                args: crate::vec![],
                returns: saikuro_core::schema::TypeDescriptor::primitive(
                    saikuro_core::schema::PrimitiveType::I64,
                ),
                visibility: saikuro_core::schema::Visibility::Public,
                capabilities: crate::vec![],
                idempotent: false,
                doc: None,
            },
        );
        schema.namespaces.insert(
            "nums".into(),
            saikuro_core::schema::NamespaceSchema {
                functions: crate::Box::new(functions),
                doc: None,
            },
        );
        schema.version = PROTOCOL_VERSION;

        let value = {
            let bytes = saikuro_core::to_vec(&schema).expect("serialize schema");
            saikuro_core::from_slice::<Value>(&bytes).expect("schema to Value")
        };
        let announce = Envelope::announce(value).map_err(|_| "create announce")?;
        let bytes = announce.to_msgpack().map_err(|_| "to_msgpack")?;

        let typed = Envelope::<Schema>::from_msgpack_typed(&bytes)
            .map_err(|_| "typed decode of announce must succeed")?;
        crate::check_test!(
            typed.invocation_type == InvocationType::Announce,
            "the typed decode must preserve the invocation type"
        );
        crate::check_test!(
            typed.args.len() == 1,
            "the announce must carry exactly one schema argument"
        );
        crate::check_test!(
            typed.args[0].has_namespace("nums"),
            "the decoded schema must expose the announced namespace"
        );

        let registry = SchemaRegistry::new();
        registry
            .merge_schema(schema, "nums-provider")
            .await
            .map_err(|_| "merge schema")?;
        crate::check_test!(
            registry.has_namespace("nums").await,
            "the registry must expose the merged namespace"
        );
        let resolved = registry
            .lookup_function("nums.negate")
            .await
            .map_err(|_| "lookup merged function")?;
        crate::check_test!(
            resolved.namespace == "nums" && resolved.function == "negate",
            "lookup must resolve the merged function"
        );
        crate::check_test!(
            resolved.provider_id == "nums-provider",
            "the resolved function must carry its provider"
        );
        Ok(())
    })
}
