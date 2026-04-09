//! Code generation output tests

use saikuro_codegen::{
    csharp::CSharpGenerator, generator::BindingGenerator, python::PythonGenerator,
    rust::RustGenerator, typescript::TypeScriptGenerator,
};
use saikuro_core::schema::{
    ArgumentDescriptor, FieldDescriptor, FunctionSchema, NamespaceSchema, PrimitiveType, Schema,
    TypeDefinition, TypeDescriptor, Visibility,
};
use std::collections::{BTreeMap, HashMap};

// Schema builders

fn simple_fn(vis: Visibility) -> FunctionSchema {
    FunctionSchema {
        args: vec![ArgumentDescriptor {
            name: "x".into(),
            r#type: TypeDescriptor::primitive(PrimitiveType::I64),
            optional: false,
            default: None,
            doc: None,
        }],
        returns: TypeDescriptor::primitive(PrimitiveType::I64),
        visibility: vis,
        capabilities: vec![],
        idempotent: false,
        doc: Some("doc comment".into()),
    }
}

fn make_schema_with_math() -> Schema {
    let mut schema = Schema::new();
    let mut functions = HashMap::new();
    functions.insert("add".into(), simple_fn(Visibility::Public));
    functions.insert("sub".into(), {
        let mut f = simple_fn(Visibility::Internal);
        f.doc = None;
        f
    });
    functions.insert("secret".into(), simple_fn(Visibility::Private));
    schema.namespaces.insert(
        "math".into(),
        NamespaceSchema {
            functions,
            doc: Some("Math namespace".into()),
        },
    );
    schema
}

fn make_schema_with_types() -> Schema {
    let mut schema = Schema::new();

    // Record type.
    let mut fields = BTreeMap::new();
    fields.insert(
        "name".into(),
        FieldDescriptor {
            r#type: TypeDescriptor::primitive(PrimitiveType::String),
            optional: false,
            doc: None,
        },
    );
    fields.insert(
        "age".into(),
        FieldDescriptor {
            r#type: TypeDescriptor::primitive(PrimitiveType::I64),
            optional: true,
            doc: None,
        },
    );
    schema
        .types
        .insert("Person".into(), TypeDefinition::Record { fields });

    // Enum type.
    schema.types.insert(
        "Color".into(),
        TypeDefinition::Enum {
            variants: vec!["Red".into(), "Green".into(), "Blue".into()],
        },
    );

    // Alias type.
    schema.types.insert(
        "UserId".into(),
        TypeDefinition::Alias {
            inner: TypeDescriptor::primitive(PrimitiveType::String),
        },
    );

    schema
}

// Python generator tests

#[test]
fn python_empty_schema_produces_required_files() {
    let schema = Schema::new();
    let gen = PythonGenerator;
    let output = gen.generate(&schema).expect("generate");

    let paths: Vec<_> = output.files.iter().map(|f| f.path.as_str()).collect();
    assert!(paths.contains(&"types.py"), "types.py missing");
    assert!(paths.contains(&"__init__.py"), "__init__.py missing");
}

#[test]
fn python_generates_client_per_namespace() {
    let schema = make_schema_with_math();
    let gen = PythonGenerator;
    let output = gen.generate(&schema).expect("generate");

    let paths: Vec<_> = output.files.iter().map(|f| f.path.as_str()).collect();
    assert!(paths.contains(&"math_client.py"), "math_client.py missing");
}

#[test]
fn python_public_and_internal_functions_are_generated() {
    let schema = make_schema_with_math();
    let gen = PythonGenerator;
    let output = gen.generate(&schema).expect("generate");

    let client = output
        .files
        .iter()
        .find(|f| f.path == "math_client.py")
        .expect("math_client.py");

    assert!(
        client.content.contains("async def add"),
        "add should appear"
    );
    assert!(
        client.content.contains("async def sub"),
        "sub should appear"
    );
}

#[test]
fn python_private_functions_are_omitted() {
    let schema = make_schema_with_math();
    let gen = PythonGenerator;
    let output = gen.generate(&schema).expect("generate");

    let client = output
        .files
        .iter()
        .find(|f| f.path == "math_client.py")
        .expect("math_client.py");

    assert!(
        !client.content.contains("async def secret"),
        "private fn 'secret' should not be generated"
    );
}

#[test]
fn python_init_imports_all_clients() {
    let schema = make_schema_with_math();
    let gen = PythonGenerator;
    let output = gen.generate(&schema).expect("generate");

    let init = output
        .files
        .iter()
        .find(|f| f.path == "__init__.py")
        .expect("__init__.py");

    assert!(
        init.content.contains("from .math_client"),
        "__init__.py should import math_client"
    );
}

#[test]
fn python_types_file_contains_record() {
    let schema = make_schema_with_types();
    let gen = PythonGenerator;
    let output = gen.generate(&schema).expect("generate");

    let types = output
        .files
        .iter()
        .find(|f| f.path == "types.py")
        .expect("types.py");

    assert!(
        types.content.contains("class Person"),
        "Person class missing"
    );
    assert!(types.content.contains("name: str"), "name field missing");
}

#[test]
fn python_types_file_contains_enum() {
    let schema = make_schema_with_types();
    let gen = PythonGenerator;
    let output = gen.generate(&schema).expect("generate");

    let types = output
        .files
        .iter()
        .find(|f| f.path == "types.py")
        .expect("types.py");

    assert!(types.content.contains("Color"), "Color enum missing");
}

#[test]
fn python_types_file_contains_alias() {
    let schema = make_schema_with_types();
    let gen = PythonGenerator;
    let output = gen.generate(&schema).expect("generate");

    let types = output
        .files
        .iter()
        .find(|f| f.path == "types.py")
        .expect("types.py");

    assert!(types.content.contains("UserId"), "UserId alias missing");
}

#[test]
fn python_all_primitive_types_map_correctly() {
    let primitive_cases: &[(&str, PrimitiveType, &str)] = &[
        ("b_fn", PrimitiveType::Bool, "bool"),
        ("i_fn", PrimitiveType::I64, "int"),
        ("f_fn", PrimitiveType::F64, "float"),
        ("s_fn", PrimitiveType::String, "str"),
        ("by_fn", PrimitiveType::Bytes, "bytes"),
        ("any_fn", PrimitiveType::Any, "Any"),
        ("u_fn", PrimitiveType::Unit, "None"),
    ];

    let mut schema = Schema::new();
    let mut functions = HashMap::new();
    for (name, prim, _) in primitive_cases {
        let f = FunctionSchema {
            args: vec![],
            returns: TypeDescriptor::primitive(prim.clone()),
            visibility: Visibility::Public,
            capabilities: vec![],
            idempotent: false,
            doc: None,
        };
        functions.insert((*name).to_owned(), f);
    }
    schema.namespaces.insert(
        "types_ns".into(),
        NamespaceSchema {
            functions,
            doc: None,
        },
    );

    let gen = PythonGenerator;
    let output = gen.generate(&schema).expect("generate");
    let client = output
        .files
        .iter()
        .find(|f| f.path == "types_ns_client.py")
        .expect("types_ns_client.py");

    for (fn_name, _, expected_type) in primitive_cases {
        assert!(
            client.content.contains(expected_type),
            "Python type '{}' not found in client for fn '{}'\nContent:\n{}",
            expected_type,
            fn_name,
            client.content
        );
    }
}

// TypeScript generator tests

#[test]
fn typescript_empty_schema_produces_required_files() {
    let schema = Schema::new();
    let gen = TypeScriptGenerator;
    let output = gen.generate(&schema).expect("generate");

    let paths: Vec<_> = output.files.iter().map(|f| f.path.as_str()).collect();
    assert!(paths.contains(&"types.ts"), "types.ts missing");
    assert!(paths.contains(&"index.ts"), "index.ts missing");
}

#[test]
fn typescript_generates_client_per_namespace() {
    let schema = make_schema_with_math();
    let gen = TypeScriptGenerator;
    let output = gen.generate(&schema).expect("generate");

    let paths: Vec<_> = output.files.iter().map(|f| f.path.as_str()).collect();
    assert!(paths.contains(&"MathClient.ts"), "MathClient.ts missing");
}

#[test]
fn typescript_public_and_internal_functions_are_generated() {
    let schema = make_schema_with_math();
    let gen = TypeScriptGenerator;
    let output = gen.generate(&schema).expect("generate");

    let client = output
        .files
        .iter()
        .find(|f| f.path == "MathClient.ts")
        .expect("MathClient.ts");

    assert!(client.content.contains("async add"), "add missing");
    assert!(client.content.contains("async sub"), "sub missing");
}

#[test]
fn typescript_private_functions_are_omitted() {
    let schema = make_schema_with_math();
    let gen = TypeScriptGenerator;
    let output = gen.generate(&schema).expect("generate");

    let client = output
        .files
        .iter()
        .find(|f| f.path == "MathClient.ts")
        .expect("MathClient.ts");

    assert!(
        !client.content.contains("async secret"),
        "private fn 'secret' should not be generated"
    );
}

#[test]
fn typescript_index_exports_all_clients() {
    let schema = make_schema_with_math();
    let gen = TypeScriptGenerator;
    let output = gen.generate(&schema).expect("generate");

    let index = output
        .files
        .iter()
        .find(|f| f.path == "index.ts")
        .expect("index.ts");

    assert!(
        index.content.contains("MathClient"),
        "index.ts should export MathClient"
    );
}

#[test]
fn typescript_types_file_contains_interface() {
    let schema = make_schema_with_types();
    let gen = TypeScriptGenerator;
    let output = gen.generate(&schema).expect("generate");

    let types = output
        .files
        .iter()
        .find(|f| f.path == "types.ts")
        .expect("types.ts");

    assert!(
        types.content.contains("interface Person"),
        "Person interface missing"
    );
    assert!(types.content.contains("name: string"), "name field missing");
}

#[test]
fn typescript_types_file_contains_enum_union() {
    let schema = make_schema_with_types();
    let gen = TypeScriptGenerator;
    let output = gen.generate(&schema).expect("generate");

    let types = output
        .files
        .iter()
        .find(|f| f.path == "types.ts")
        .expect("types.ts");

    assert!(types.content.contains("Color"), "Color type missing");
    assert!(types.content.contains("\"Red\""), "Red variant missing");
}

#[test]
fn typescript_all_primitive_types_map_correctly() {
    let primitive_cases: &[(&str, PrimitiveType, &str)] = &[
        ("b_fn", PrimitiveType::Bool, "boolean"),
        ("i_fn", PrimitiveType::I64, "number"),
        ("f_fn", PrimitiveType::F64, "number"),
        ("s_fn", PrimitiveType::String, "string"),
        ("by_fn", PrimitiveType::Bytes, "Uint8Array"),
        ("any_fn", PrimitiveType::Any, "unknown"),
        ("u_fn", PrimitiveType::Unit, "void"),
    ];

    let mut schema = Schema::new();
    let mut functions = HashMap::new();
    for (name, prim, _) in primitive_cases {
        let f = FunctionSchema {
            args: vec![],
            returns: TypeDescriptor::primitive(prim.clone()),
            visibility: Visibility::Public,
            capabilities: vec![],
            idempotent: false,
            doc: None,
        };
        functions.insert((*name).to_owned(), f);
    }
    schema.namespaces.insert(
        "types_ns".into(),
        NamespaceSchema {
            functions,
            doc: None,
        },
    );

    let gen = TypeScriptGenerator;
    let output = gen.generate(&schema).expect("generate");
    let client = output
        .files
        .iter()
        .find(|f| f.path == "TypesNsClient.ts")
        .expect("TypesNsClient.ts");

    for (fn_name, _, expected_type) in primitive_cases {
        assert!(
            client.content.contains(expected_type),
            "TS type '{}' not found for fn '{}'\nContent:\n{}",
            expected_type,
            fn_name,
            client.content
        );
    }
}

#[test]
fn typescript_optional_arg_has_question_mark() {
    let mut schema = Schema::new();
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
                    default: None,
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
    schema.namespaces.insert(
        "greeter".into(),
        NamespaceSchema {
            functions,
            doc: None,
        },
    );

    let gen = TypeScriptGenerator;
    let output = gen.generate(&schema).expect("generate");
    let client = output
        .files
        .iter()
        .find(|f| f.path == "GreeterClient.ts")
        .expect("GreeterClient.ts");

    assert!(
        client.content.contains("greeting?:"),
        "optional arg 'greeting' should have '?' in TS signature"
    );
}

// Stream/Channel dispatch tests

fn make_schema_with_stream_and_channel() -> Schema {
    let mut schema = Schema::new();
    let mut functions = HashMap::new();

    // Stream-returning function.
    functions.insert(
        "subscribe".into(),
        FunctionSchema {
            args: vec![ArgumentDescriptor {
                name: "topic".into(),
                r#type: TypeDescriptor::primitive(PrimitiveType::String),
                optional: false,
                default: None,
                doc: None,
            }],
            returns: TypeDescriptor::Stream {
                item: Box::new(TypeDescriptor::primitive(PrimitiveType::String)),
            },
            visibility: Visibility::Public,
            capabilities: vec![],
            idempotent: false,
            doc: Some("Subscribe to a topic and receive a stream of messages.".into()),
        },
    );

    // Channel-returning function.
    functions.insert(
        "chat".into(),
        FunctionSchema {
            args: vec![],
            returns: TypeDescriptor::Channel {
                inbound: Box::new(TypeDescriptor::primitive(PrimitiveType::String)),
                outbound: Box::new(TypeDescriptor::primitive(PrimitiveType::String)),
            },
            visibility: Visibility::Public,
            capabilities: vec![],
            idempotent: false,
            doc: Some("Open a bidirectional chat channel.".into()),
        },
    );

    // Regular call-returning function for contrast.
    functions.insert(
        "ping".into(),
        FunctionSchema {
            args: vec![],
            returns: TypeDescriptor::primitive(PrimitiveType::String),
            visibility: Visibility::Public,
            capabilities: vec![],
            idempotent: false,
            doc: None,
        },
    );

    schema.namespaces.insert(
        "events".into(),
        NamespaceSchema {
            functions,
            doc: None,
        },
    );
    schema
}

#[test]
fn python_stream_method_calls_stream_not_call() {
    let schema = make_schema_with_stream_and_channel();
    let gen = PythonGenerator;
    let output = gen.generate(&schema).expect("generate");

    let client = output
        .files
        .iter()
        .find(|f| f.path == "events_client.py")
        .expect("events_client.py");

    // The subscribe method must call .stream(), not .call().
    assert!(
        client
            .content
            .contains("self._client.stream(\"events.subscribe\""),
        "subscribe should call stream(), not call(). Content:\n{}",
        client.content
    );
    // And return an AsyncIterator type.
    assert!(
        client.content.contains("AsyncIterator[str]"),
        "subscribe return type should be AsyncIterator[str]. Content:\n{}",
        client.content
    );
    // ping still uses call().
    assert!(
        client.content.contains("self._client.call(\"events.ping\""),
        "ping should still call call(). Content:\n{}",
        client.content
    );
}

#[test]
fn python_channel_method_calls_channel_not_call() {
    let schema = make_schema_with_stream_and_channel();
    let gen = PythonGenerator;
    let output = gen.generate(&schema).expect("generate");

    let client = output
        .files
        .iter()
        .find(|f| f.path == "events_client.py")
        .expect("events_client.py");

    // The chat method must call .channel(), not .call().
    assert!(
        client
            .content
            .contains("self._client.channel(\"events.chat\""),
        "chat should call channel(), not call(). Content:\n{}",
        client.content
    );
    // The signature should reference SaikuroChannel.
    assert!(
        client.content.contains("SaikuroChannel"),
        "channel method should reference SaikuroChannel. Content:\n{}",
        client.content
    );
    // The comment hint for the import should be present.
    assert!(
        client.content.contains("# from saikuro import"),
        "stream/channel import hint comment should appear. Content:\n{}",
        client.content
    );
}

#[test]
fn typescript_stream_method_calls_stream_not_call() {
    let schema = make_schema_with_stream_and_channel();
    let gen = TypeScriptGenerator;
    let output = gen.generate(&schema).expect("generate");

    let client = output
        .files
        .iter()
        .find(|f| f.path == "EventsClient.ts")
        .expect("EventsClient.ts");

    // subscribe must call .stream<...>(), not .call().
    assert!(
        client
            .content
            .contains("this.client.stream<string>('events.subscribe'"),
        "subscribe should call stream(), not call(). Content:\n{}",
        client.content
    );
    // Return type should be Promise<AsyncIterable<string>>.
    assert!(
        client.content.contains("Promise<AsyncIterable<string>>"),
        "subscribe return type should be Promise<AsyncIterable<string>>. Content:\n{}",
        client.content
    );
    // ping still uses call().
    assert!(
        client.content.contains("this.client.call('events.ping'"),
        "ping should still call call(). Content:\n{}",
        client.content
    );
}

#[test]
fn typescript_channel_method_calls_channel_not_call() {
    let schema = make_schema_with_stream_and_channel();
    let gen = TypeScriptGenerator;
    let output = gen.generate(&schema).expect("generate");

    let client = output
        .files
        .iter()
        .find(|f| f.path == "EventsClient.ts")
        .expect("EventsClient.ts");

    // chat must call .channel<...>(), not .call().
    assert!(
        client
            .content
            .contains("this.client.channel<string, string>('events.chat'"),
        "chat should call channel(), not call(). Content:\n{}",
        client.content
    );
    // Return type should be Promise<SaikuroChannel<string, string>>.
    assert!(
        client
            .content
            .contains("Promise<SaikuroChannel<string, string>>"),
        "chat return type should be Promise<SaikuroChannel<...>>. Content:\n{}",
        client.content
    );
    // SaikuroChannel import should be present when channel functions exist.
    assert!(
        client.content.contains("import type { SaikuroChannel }"),
        "SaikuroChannel import should appear when channel functions exist. Content:\n{}",
        client.content
    );
}

// C# generator tests

#[test]
fn csharp_empty_schema_produces_required_files() {
    let schema = Schema::new();
    let gen = CSharpGenerator;
    let output = gen.generate(&schema).expect("generate");

    let paths: Vec<_> = output.files.iter().map(|f| f.path.as_str()).collect();
    assert!(paths.contains(&"Types.cs"), "Types.cs missing");
    assert!(paths.contains(&"Generated.cs"), "Generated.cs missing");
}

#[test]
fn csharp_generates_client_per_namespace() {
    let schema = make_schema_with_math();
    let gen = CSharpGenerator;
    let output = gen.generate(&schema).expect("generate");

    let paths: Vec<_> = output.files.iter().map(|f| f.path.as_str()).collect();
    assert!(paths.contains(&"MathClient.cs"), "MathClient.cs missing");
}

#[test]
fn csharp_public_and_internal_functions_are_generated() {
    let schema = make_schema_with_math();
    let gen = CSharpGenerator;
    let output = gen.generate(&schema).expect("generate");

    let client = output
        .files
        .iter()
        .find(|f| f.path == "MathClient.cs")
        .expect("MathClient.cs");

    assert!(
        client.content.contains("AddAsync"),
        "add should appear as AddAsync"
    );
    assert!(
        client.content.contains("SubAsync"),
        "sub should appear as SubAsync"
    );
}

#[test]
fn csharp_private_functions_are_omitted() {
    let schema = make_schema_with_math();
    let gen = CSharpGenerator;
    let output = gen.generate(&schema).expect("generate");

    let client = output
        .files
        .iter()
        .find(|f| f.path == "MathClient.cs")
        .expect("MathClient.cs");

    assert!(
        !client.content.contains("SecretAsync"),
        "private fn 'secret' should not be generated"
    );
}

#[test]
fn csharp_generated_file_lists_clients() {
    let schema = make_schema_with_math();
    let gen = CSharpGenerator;
    let output = gen.generate(&schema).expect("generate");

    let index = output
        .files
        .iter()
        .find(|f| f.path == "Generated.cs")
        .expect("Generated.cs");

    assert!(
        index.content.contains("MathClient"),
        "Generated.cs should mention MathClient"
    );
}

#[test]
fn csharp_types_file_contains_record() {
    let schema = make_schema_with_types();
    let gen = CSharpGenerator;
    let output = gen.generate(&schema).expect("generate");

    let types = output
        .files
        .iter()
        .find(|f| f.path == "Types.cs")
        .expect("Types.cs");

    assert!(types.content.contains("Person"), "Person record missing");
    assert!(
        types.content.contains("string"),
        "string field type missing"
    );
}

#[test]
fn csharp_types_file_contains_enum() {
    let schema = make_schema_with_types();
    let gen = CSharpGenerator;
    let output = gen.generate(&schema).expect("generate");

    let types = output
        .files
        .iter()
        .find(|f| f.path == "Types.cs")
        .expect("Types.cs");

    assert!(types.content.contains("Color"), "Color enum missing");
    assert!(types.content.contains("Red"), "Red variant missing");
}

#[test]
fn csharp_types_file_contains_alias() {
    let schema = make_schema_with_types();
    let gen = CSharpGenerator;
    let output = gen.generate(&schema).expect("generate");

    let types = output
        .files
        .iter()
        .find(|f| f.path == "Types.cs")
        .expect("Types.cs");

    assert!(types.content.contains("UserId"), "UserId alias missing");
}

#[test]
fn csharp_all_primitive_types_map_correctly() {
    let primitive_cases: &[(&str, PrimitiveType, &str)] = &[
        ("b_fn", PrimitiveType::Bool, "bool"),
        ("i_fn", PrimitiveType::I64, "long"),
        ("f_fn", PrimitiveType::F64, "double"),
        ("s_fn", PrimitiveType::String, "string"),
        ("by_fn", PrimitiveType::Bytes, "byte[]"),
        ("any_fn", PrimitiveType::Any, "object?"),
    ];

    let mut schema = Schema::new();
    let mut functions = HashMap::new();
    for (name, prim, _) in primitive_cases {
        let f = FunctionSchema {
            args: vec![],
            returns: TypeDescriptor::primitive(prim.clone()),
            visibility: Visibility::Public,
            capabilities: vec![],
            idempotent: false,
            doc: None,
        };
        functions.insert((*name).to_owned(), f);
    }
    schema.namespaces.insert(
        "types_ns".into(),
        NamespaceSchema {
            functions,
            doc: None,
        },
    );

    let gen = CSharpGenerator;
    let output = gen.generate(&schema).expect("generate");
    let client = output
        .files
        .iter()
        .find(|f| f.path == "TypesNsClient.cs")
        .expect("TypesNsClient.cs");

    for (fn_name, _, expected_type) in primitive_cases {
        assert!(
            client.content.contains(expected_type),
            "C# type '{}' not found for fn '{}'\nContent:\n{}",
            expected_type,
            fn_name,
            client.content
        );
    }
}

#[test]
fn csharp_stream_method_uses_stream_async() {
    let schema = make_schema_with_stream_and_channel();
    let gen = CSharpGenerator;
    let output = gen.generate(&schema).expect("generate");

    let client = output
        .files
        .iter()
        .find(|f| f.path == "EventsClient.cs")
        .expect("EventsClient.cs");

    assert!(
        client.content.contains("StreamAsync"),
        "subscribe should use StreamAsync. Content:\n{}",
        client.content
    );
    assert!(
        client.content.contains("SaikuroStream<string>"),
        "subscribe return type should be SaikuroStream<string>. Content:\n{}",
        client.content
    );
}

#[test]
fn csharp_channel_method_uses_channel_async() {
    let schema = make_schema_with_stream_and_channel();
    let gen = CSharpGenerator;
    let output = gen.generate(&schema).expect("generate");

    let client = output
        .files
        .iter()
        .find(|f| f.path == "EventsClient.cs")
        .expect("EventsClient.cs");

    assert!(
        client.content.contains("ChannelAsync"),
        "chat should use ChannelAsync. Content:\n{}",
        client.content
    );
    assert!(
        client.content.contains("SaikuroChannel<string, string>"),
        "chat return type should be SaikuroChannel<string, string>. Content:\n{}",
        client.content
    );
}

// Rust generator tests

#[test]
fn rust_empty_schema_produces_required_files() {
    let schema = Schema::new();
    let gen = RustGenerator;
    let output = gen.generate(&schema).expect("generate");

    let paths: Vec<_> = output.files.iter().map(|f| f.path.as_str()).collect();
    assert!(paths.contains(&"types.rs"), "types.rs missing");
    assert!(paths.contains(&"mod.rs"), "mod.rs missing");
}

#[test]
fn rust_generates_client_per_namespace() {
    let schema = make_schema_with_math();
    let gen = RustGenerator;
    let output = gen.generate(&schema).expect("generate");

    let paths: Vec<_> = output.files.iter().map(|f| f.path.as_str()).collect();
    assert!(paths.contains(&"math_client.rs"), "math_client.rs missing");
}

#[test]
fn rust_private_functions_are_omitted() {
    let schema = make_schema_with_math();
    let gen = RustGenerator;
    let output = gen.generate(&schema).expect("generate");

    let client = output
        .files
        .iter()
        .find(|f| f.path == "math_client.rs")
        .expect("math_client.rs");

    assert!(client.content.contains("pub async fn add"), "add missing");
    assert!(client.content.contains("pub async fn sub"), "sub missing");
    assert!(
        !client.content.contains("pub async fn secret"),
        "private fn 'secret' should not be generated"
    );
}

#[test]
fn rust_stream_and_channel_methods_use_adapter_primitives() {
    let schema = make_schema_with_stream_and_channel();
    let gen = RustGenerator;
    let output = gen.generate(&schema).expect("generate");

    let client = output
        .files
        .iter()
        .find(|f| f.path == "events_client.rs")
        .expect("events_client.rs");

    assert!(
        client.content.contains("Result<saikuro::SaikuroStream>"),
        "stream method should return SaikuroStream"
    );
    assert!(
        client.content.contains("Result<saikuro::SaikuroChannel>"),
        "channel method should return SaikuroChannel"
    );
    assert!(
        client
            .content
            .contains("self.client.stream(\"events.subscribe\""),
        "stream method should call client.stream"
    );
    assert!(
        client
            .content
            .contains("self.client.channel(\"events.chat\""),
        "channel method should call client.channel"
    );
}
