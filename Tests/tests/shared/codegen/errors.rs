use crate::shared_test;
use crate::String;
use crate::TestSuite;
use saikuro_codegen::{
    convert_type, generate_types_from_schema, namespace_public_functions, CodegenError,
    TypeConverter,
};
use saikuro_core::schema::{
    FieldDescriptor, FieldMap, FunctionMap, FunctionSchema, NamespaceSchema, PrimitiveType, Schema,
    TypeDefinition, TypeDescriptor, Visibility,
};

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "codegen::codegen_error_display_and_debug",
        codegen_error_display_and_debug,
    );
    shared_test!(
        suite,
        "codegen::convert_type_primitives_mapped_by_converter",
        convert_type_primitives_mapped_by_converter,
    );
    shared_test!(
        suite,
        "codegen::convert_type_nested_wrappers",
        convert_type_nested_wrappers,
    );
    shared_test!(
        suite,
        "codegen::namespace_public_functions_filters_private_and_sorts",
        namespace_public_functions_filters_private_and_sorts,
    );
    shared_test!(
        suite,
        "codegen::generate_types_from_schema_emits_all_kinds",
        generate_types_from_schema_emits_all_kinds,
    );
    shared_test!(
        suite,
        "codegen::generate_types_from_schema_sorts_types",
        generate_types_from_schema_sorts_types,
    );
    shared_test!(
        suite,
        "codegen::generate_types_from_schema_propagates_error",
        generate_types_from_schema_propagates_error,
    );
    shared_test!(
        suite,
        "codegen::generate_types_from_schema_empty_types",
        generate_types_from_schema_empty_types,
    );
}

/// A tiny converter with predictable wrappers so `convert_type` output is
/// structurally assertable.
struct TestConverter;

impl TypeConverter for TestConverter {
    fn primitive_name(&self, t: &PrimitiveType) -> &'static str {
        match t {
            PrimitiveType::String => "Str",
            PrimitiveType::I64 => "I64",
            PrimitiveType::Unit => "Unit",
            _ => "Ty",
        }
    }

    fn named_type(&self, name: &str) -> String {
        crate::format!("Named[{name}]")
    }

    fn wrap_option(&self, inner: &str) -> String {
        crate::format!("Opt<{inner}>")
    }

    fn wrap_array(&self, inner: &str) -> String {
        crate::format!("Arr<{inner}>")
    }

    fn wrap_map(&self, value: &str) -> String {
        crate::format!("Map<String>{value}>")
    }

    fn wrap_stream(&self, item: &str) -> String {
        crate::format!("Stream<{item}>")
    }

    fn wrap_channel(&self, inbound: &str, outbound: &str) -> String {
        crate::format!("Chan<{inbound},{outbound}>")
    }
}

fn codegen_error_display_and_debug() -> Result<(), &'static str> {
    let unsupported = CodegenError::UnsupportedType("f64".into());
    crate::check_test!(
        crate::format!("{unsupported}") == "unsupported type for target language: f64",
        "UnsupportedType display must match the documented wording"
    );
    let schema_err = CodegenError::Schema("bad schema".into());
    crate::check_test!(
        crate::format!("{schema_err}") == "schema error: bad schema",
        "Schema display must match the documented wording"
    );
    let template_err = CodegenError::Template("boom".into());
    crate::check_test!(
        crate::format!("{template_err}") == "template error: boom",
        "Template display must match the documented wording"
    );
    crate::check_test!(
        crate::format!("{unsupported:?}").contains("UnsupportedType"),
        "Debug must expose the variant name"
    );
    Ok(())
}

fn convert_type_primitives_mapped_by_converter() -> Result<(), &'static str> {
    let conv = TestConverter;
    crate::check_test!(
        convert_type(&TypeDescriptor::primitive(PrimitiveType::String), &conv) == "Str",
        "string primitive must map through the converter"
    );
    crate::check_test!(
        convert_type(&TypeDescriptor::primitive(PrimitiveType::I64), &conv) == "I64",
        "i64 primitive must map through the converter"
    );
    crate::check_test!(
        convert_type(&TypeDescriptor::named("Person"), &conv) == "Named[Person]",
        "named type must map through the converter"
    );
    Ok(())
}

fn convert_type_nested_wrappers() -> Result<(), &'static str> {
    let conv = TestConverter;
    let optional = TypeDescriptor::primitive(PrimitiveType::String).optional();
    crate::check_test!(
        convert_type(&optional, &conv) == "Opt<Str>",
        "option must wrap the inner type"
    );

    let array = TypeDescriptor::Array {
        item: crate::Box::new(TypeDescriptor::primitive(PrimitiveType::I64)),
    };
    crate::check_test!(
        convert_type(&array, &conv) == "Arr<I64>",
        "array must wrap the item type"
    );

    let map = TypeDescriptor::Map {
        value: crate::Box::new(TypeDescriptor::primitive(PrimitiveType::String)),
    };
    crate::check_test!(
        convert_type(&map, &conv) == "Map<String>Str>",
        "map must wrap the value type"
    );

    let stream = TypeDescriptor::Stream {
        item: crate::Box::new(TypeDescriptor::primitive(PrimitiveType::I64)),
    };
    crate::check_test!(
        convert_type(&stream, &conv) == "Stream<I64>",
        "stream must wrap the item type"
    );

    let channel = TypeDescriptor::Channel {
        inbound: crate::Box::new(TypeDescriptor::primitive(PrimitiveType::String)),
        outbound: crate::Box::new(TypeDescriptor::primitive(PrimitiveType::I64)),
    };
    crate::check_test!(
        convert_type(&channel, &conv) == "Chan<Str,I64>",
        "channel must wrap inbound and outbound types"
    );

    let nested_array_of_options = TypeDescriptor::Array {
        item: crate::Box::new(TypeDescriptor::primitive(PrimitiveType::String).optional()),
    };
    crate::check_test!(
        convert_type(&nested_array_of_options, &conv) == "Arr<Opt<Str>>",
        "nested wrappers must recurse"
    );
    Ok(())
}

fn make_function_map() -> FunctionMap {
    let mut functions = FunctionMap::new();
    for (name, vis) in [
        ("alpha", Visibility::Public),
        ("bravo", Visibility::Public),
        ("charlie", Visibility::Internal),
        ("delta", Visibility::Private),
    ] {
        functions.insert(
            name.into(),
            FunctionSchema {
                args: crate::vec![],
                returns: TypeDescriptor::primitive(PrimitiveType::Unit),
                visibility: vis,
                capabilities: crate::vec![],
                idempotent: false,
                doc: None,
            },
        );
    }
    functions
}

fn namespace_public_functions_filters_private_and_sorts() -> Result<(), &'static str> {
    let ns = NamespaceSchema {
        functions: crate::Box::new(make_function_map()),
        doc: None,
    };
    let public = namespace_public_functions(&ns);
    let named: crate::Vec<&str> = public.iter().map(|(name, _)| *name).collect();
    crate::check_test!(
        named == crate::vec!["alpha", "bravo", "charlie"],
        "private functions must be filtered, the rest sorted by name"
    );
    Ok(())
}

fn make_schema_with_types() -> Schema {
    let mut schema = Schema::new();

    let mut fields = FieldMap::new();
    fields.insert(
        "name".into(),
        FieldDescriptor {
            r#type: TypeDescriptor::primitive(PrimitiveType::String),
            optional: false,
            doc: None,
        },
    );
    schema.types.insert(
        "Person".into(),
        TypeDefinition::Record {
            fields: crate::Box::new(fields),
        },
    );
    schema.types.insert(
        "Color".into(),
        TypeDefinition::Enum {
            variants: crate::vec!["Red".into(), "Green".into(), "Blue".into()],
        },
    );
    schema.types.insert(
        "UserId".into(),
        TypeDefinition::Alias {
            inner: TypeDescriptor::primitive(PrimitiveType::String),
        },
    );
    schema
}

fn generate_types_from_schema_emits_all_kinds() -> Result<(), &'static str> {
    let schema = make_schema_with_types();
    let out = generate_types_from_schema(
        &schema,
        crate::vec!["// header".into()],
        |name, fields| {
            let mut lines = crate::vec![crate::format!("record {name}")];
            for field in fields.keys() {
                lines.push(crate::format!("  field {field}"));
            }
            Ok(lines)
        },
        |name, variants| {
            Ok(crate::vec![crate::format!(
                "enum {name} {{{}}}",
                variants.join(", ")
            )])
        },
        |name, _inner| Ok(crate::vec![crate::format!("alias {name}")]),
    )
    .map_err(|_| "generate_types_from_schema must succeed")?;
    crate::check_test!(
        out.contains("record Person"),
        "record types must be emitted"
    );
    crate::check_test!(out.contains("field name"), "record fields must be emitted");
    crate::check_test!(
        out.contains("enum Color {Red, Green, Blue}"),
        "enum types must be emitted"
    );
    crate::check_test!(out.contains("alias UserId"), "alias types must be emitted");
    crate::check_test!(
        out.starts_with("// header"),
        "header must be emitted before any type"
    );
    Ok(())
}

fn generate_types_from_schema_sorts_types() -> Result<(), &'static str> {
    let mut schema = Schema::new();
    for (name, inner) in [
        ("Zebra", PrimitiveType::String),
        ("apple", PrimitiveType::U8),
        ("mango", PrimitiveType::Bool),
    ] {
        schema.types.insert(
            name.into(),
            TypeDefinition::Alias {
                inner: TypeDescriptor::primitive(inner),
            },
        );
    }
    let out = generate_types_from_schema(
        &schema,
        crate::vec![],
        |_name, _fields| Ok(crate::vec![]),
        |_name, _variants| Ok(crate::vec![]),
        |name, _inner| Ok(crate::vec![crate::format!("alias {name}")]),
    )
    .map_err(|_| "sorted generation must succeed")?;
    crate::check_test!(
        out == "alias Zebra\nalias apple\nalias mango",
        "types must be emitted in byte-sorted order (uppercase before lowercase)"
    );
    Ok(())
}

fn generate_types_from_schema_propagates_error() -> Result<(), &'static str> {
    let schema = make_schema_with_types();
    let result = generate_types_from_schema(
        &schema,
        crate::vec![],
        |name, _fields| {
            if name == "Person" {
                Err(CodegenError::Schema("record too weird".into()))
            } else {
                Ok(crate::vec![])
            }
        },
        |_name, _variants| Ok(crate::vec![]),
        |_name, _inner| Ok(crate::vec![]),
    );
    crate::check_test!(
        matches!(result, Err(CodegenError::Schema(msg)) if msg == "record too weird"),
        "on_record errors must propagate as CodegenError"
    );
    Ok(())
}

fn generate_types_from_schema_empty_types() -> Result<(), &'static str> {
    let schema = Schema::new();
    let out = generate_types_from_schema(
        &schema,
        crate::vec!["only-headers".into()],
        |_name, _fields| Ok(crate::vec![]),
        |_name, _variants| Ok(crate::vec![]),
        |_name, _inner| Ok(crate::vec![]),
    )
    .map_err(|_| "empty schema must succeed")?;
    crate::check_test!(
        out == "only-headers",
        "an empty type map must yield just the header"
    );
    Ok(())
}
