use crate::TestSuite;

mod binding_output {
    use crate::shared_test;
    use crate::TestSuite;
    use crate::ToOwned;
    use saikuro_codegen::language::{
        csharp::CSharpGenerator, python::PythonGenerator, rust::RustGenerator,
        typescript::TypeScriptGenerator,
    };
    use saikuro_codegen::BindingGenerator;
    use saikuro_core::schema::{
        ArgumentDescriptor, FieldDescriptor, FieldMap, FunctionMap, FunctionSchema,
        NamespaceSchema, PrimitiveType, Schema, TypeDefinition, TypeDescriptor, Visibility,
    };

    fn simple_fn(vis: Visibility) -> FunctionSchema {
        FunctionSchema {
            args: crate::vec![ArgumentDescriptor {
                name: "x".into(),
                r#type: TypeDescriptor::primitive(PrimitiveType::I64),
                optional: false,
                default: None,
                doc: None,
            }],
            returns: TypeDescriptor::primitive(PrimitiveType::I64),
            visibility: vis,
            capabilities: crate::vec![],
            idempotent: false,
            doc: Some("doc comment".into()),
        }
    }

    fn make_schema_with_math() -> Schema {
        let mut schema = Schema::new();
        let mut functions = FunctionMap::new();
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
                functions: crate::Box::new(functions),
                doc: Some("Math namespace".into()),
            },
        );
        schema
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
        fields.insert(
            "age".into(),
            FieldDescriptor {
                r#type: TypeDescriptor::primitive(PrimitiveType::I64),
                optional: true,
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

    fn make_schema_with_stream_and_channel() -> Schema {
        let mut schema = Schema::new();
        let mut functions = FunctionMap::new();

        functions.insert(
            "subscribe".into(),
            FunctionSchema {
                args: crate::vec![ArgumentDescriptor {
                    name: "topic".into(),
                    r#type: TypeDescriptor::primitive(PrimitiveType::String),
                    optional: false,
                    default: None,
                    doc: None,
                }],
                returns: TypeDescriptor::Stream {
                    item: crate::Box::new(TypeDescriptor::primitive(PrimitiveType::String)),
                },
                visibility: Visibility::Public,
                capabilities: crate::vec![],
                idempotent: false,
                doc: Some("Subscribe to a topic and receive a stream of messages.".into()),
            },
        );

        functions.insert(
            "chat".into(),
            FunctionSchema {
                args: crate::vec![],
                returns: TypeDescriptor::Channel {
                    inbound: crate::Box::new(TypeDescriptor::primitive(PrimitiveType::String)),
                    outbound: crate::Box::new(TypeDescriptor::primitive(PrimitiveType::String)),
                },
                visibility: Visibility::Public,
                capabilities: crate::vec![],
                idempotent: false,
                doc: Some("Open a bidirectional chat channel.".into()),
            },
        );

        functions.insert(
            "ping".into(),
            FunctionSchema {
                args: crate::vec![],
                returns: TypeDescriptor::primitive(PrimitiveType::String),
                visibility: Visibility::Public,
                capabilities: crate::vec![],
                idempotent: false,
                doc: None,
            },
        );

        schema.namespaces.insert(
            "events".into(),
            NamespaceSchema {
                functions: crate::Box::new(functions),
                doc: None,
            },
        );
        schema
    }

    pub fn register(suite: &mut TestSuite) {
        shared_test!(suite,
            "codegen::python_empty_schema_produces_required_files",
            python_empty_schema_produces_required_files,
        );
        shared_test!(suite,
            "codegen::python_generates_client_per_namespace",
            python_generates_client_per_namespace,
        );
        shared_test!(suite,
            "codegen::python_public_and_internal_functions_are_generated",
            python_public_and_internal_functions_are_generated,
        );
        shared_test!(suite,
            "codegen::python_private_functions_are_omitted",
            python_private_functions_are_omitted,
        );
        shared_test!(suite,
            "codegen::python_init_imports_all_clients",
            python_init_imports_all_clients,
        );
        shared_test!(suite,
            "codegen::python_types_file_contains_record",
            python_types_file_contains_record,
        );
        shared_test!(suite,
            "codegen::python_types_file_contains_enum",
            python_types_file_contains_enum,
        );
        shared_test!(suite,
            "codegen::python_types_file_contains_alias",
            python_types_file_contains_alias,
        );
        shared_test!(suite,
            "codegen::python_all_primitive_types_map_correctly",
            python_all_primitive_types_map_correctly,
        );
        shared_test!(suite,
            "codegen::typescript_empty_schema_produces_required_files",
            typescript_empty_schema_produces_required_files,
        );
        shared_test!(suite,
            "codegen::typescript_generates_client_per_namespace",
            typescript_generates_client_per_namespace,
        );
        shared_test!(suite,
            "codegen::typescript_public_and_internal_functions_are_generated",
            typescript_public_and_internal_functions_are_generated,
        );
        shared_test!(suite,
            "codegen::typescript_private_functions_are_omitted",
            typescript_private_functions_are_omitted,
        );
        shared_test!(suite,
            "codegen::typescript_index_exports_all_clients",
            typescript_index_exports_all_clients,
        );
        shared_test!(suite,
            "codegen::typescript_types_file_contains_interface",
            typescript_types_file_contains_interface,
        );
        shared_test!(suite,
            "codegen::typescript_types_file_contains_enum_union",
            typescript_types_file_contains_enum_union,
        );
        shared_test!(suite,
            "codegen::typescript_all_primitive_types_map_correctly",
            typescript_all_primitive_types_map_correctly,
        );
        shared_test!(suite,
            "codegen::typescript_optional_arg_has_question_mark",
            typescript_optional_arg_has_question_mark,
        );
        shared_test!(suite,
            "codegen::python_stream_method_calls_stream_not_call",
            python_stream_method_calls_stream_not_call,
        );
        shared_test!(suite,
            "codegen::python_channel_method_calls_channel_not_call",
            python_channel_method_calls_channel_not_call,
        );
        shared_test!(suite,
            "codegen::typescript_stream_method_calls_stream_not_call",
            typescript_stream_method_calls_stream_not_call,
        );
        shared_test!(suite,
            "codegen::typescript_channel_method_calls_channel_not_call",
            typescript_channel_method_calls_channel_not_call,
        );
        shared_test!(suite,
            "codegen::csharp_empty_schema_produces_required_files",
            csharp_empty_schema_produces_required_files,
        );
        shared_test!(suite,
            "codegen::csharp_generates_client_per_namespace",
            csharp_generates_client_per_namespace,
        );
        shared_test!(suite,
            "codegen::csharp_public_and_internal_functions_are_generated",
            csharp_public_and_internal_functions_are_generated,
        );
        shared_test!(suite,
            "codegen::csharp_private_functions_are_omitted",
            csharp_private_functions_are_omitted,
        );
        shared_test!(suite,
            "codegen::csharp_generated_file_lists_clients",
            csharp_generated_file_lists_clients,
        );
        shared_test!(suite,
            "codegen::csharp_types_file_contains_record",
            csharp_types_file_contains_record,
        );
        shared_test!(suite,
            "codegen::csharp_types_file_contains_enum",
            csharp_types_file_contains_enum,
        );
        shared_test!(suite,
            "codegen::csharp_types_file_contains_alias",
            csharp_types_file_contains_alias,
        );
        shared_test!(suite,
            "codegen::csharp_all_primitive_types_map_correctly",
            csharp_all_primitive_types_map_correctly,
        );
        shared_test!(suite,
            "codegen::csharp_stream_method_uses_stream_async",
            csharp_stream_method_uses_stream_async,
        );
        shared_test!(suite,
            "codegen::csharp_channel_method_uses_channel_async",
            csharp_channel_method_uses_channel_async,
        );
        shared_test!(suite,
            "codegen::rust_empty_schema_produces_required_files",
            rust_empty_schema_produces_required_files,
        );
        shared_test!(suite,
            "codegen::rust_generates_client_per_namespace",
            rust_generates_client_per_namespace,
        );
        shared_test!(suite,
            "codegen::rust_private_functions_are_omitted",
            rust_private_functions_are_omitted,
        );
        shared_test!(suite,
            "codegen::rust_stream_and_channel_methods_use_adapter_primitives",
            rust_stream_and_channel_methods_use_adapter_primitives,
        );
    }

    fn python_empty_schema_produces_required_files() -> Result<(), &'static str> {
        let schema = Schema::new();
        let gen = PythonGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let paths: crate::Vec<_> = output.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"types.py"), "types.py missing");
        assert!(paths.contains(&"__init__.py"), "__init__.py missing");
        Ok(())
    }

    fn python_generates_client_per_namespace() -> Result<(), &'static str> {
        let schema = make_schema_with_math();
        let gen = PythonGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let paths: crate::Vec<_> = output.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"math_client.py"), "math_client.py missing");
        Ok(())
    }

    fn python_public_and_internal_functions_are_generated() -> Result<(), &'static str> {
        let schema = make_schema_with_math();
        let gen = PythonGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let client = output
            .files
            .iter()
            .find(|f| f.path == "math_client.py")
            .ok_or("math_client.py")?;

        assert!(
            client.content.contains("async def add"),
            "add should appear"
        );
        assert!(
            client.content.contains("async def sub"),
            "sub should appear"
        );
        Ok(())
    }

    fn python_private_functions_are_omitted() -> Result<(), &'static str> {
        let schema = make_schema_with_math();
        let gen = PythonGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let client = output
            .files
            .iter()
            .find(|f| f.path == "math_client.py")
            .ok_or("math_client.py")?;

        assert!(
            !client.content.contains("async def secret"),
            "private fn 'secret' should not be generated"
        );
        Ok(())
    }

    fn python_init_imports_all_clients() -> Result<(), &'static str> {
        let schema = make_schema_with_math();
        let gen = PythonGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let init = output
            .files
            .iter()
            .find(|f| f.path == "__init__.py")
            .ok_or("__init__.py")?;

        assert!(
            init.content.contains("from .math_client"),
            "__init__.py should import math_client"
        );
        Ok(())
    }

    fn python_types_file_contains_record() -> Result<(), &'static str> {
        let schema = make_schema_with_types();
        let gen = PythonGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let types = output
            .files
            .iter()
            .find(|f| f.path == "types.py")
            .ok_or("types.py")?;

        assert!(
            types.content.contains("class Person"),
            "Person class missing"
        );
        assert!(types.content.contains("name: str"), "name field missing");
        Ok(())
    }

    fn python_types_file_contains_enum() -> Result<(), &'static str> {
        let schema = make_schema_with_types();
        let gen = PythonGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let types = output
            .files
            .iter()
            .find(|f| f.path == "types.py")
            .ok_or("types.py")?;

        assert!(types.content.contains("Color"), "Color enum missing");
        Ok(())
    }

    fn python_types_file_contains_alias() -> Result<(), &'static str> {
        let schema = make_schema_with_types();
        let gen = PythonGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let types = output
            .files
            .iter()
            .find(|f| f.path == "types.py")
            .ok_or("types.py")?;

        assert!(types.content.contains("UserId"), "UserId alias missing");
        Ok(())
    }

    fn python_all_primitive_types_map_correctly() -> Result<(), &'static str> {
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
        let mut functions = FunctionMap::new();
        for (name, prim, _) in primitive_cases {
            let f = FunctionSchema {
                args: crate::vec![],
                returns: TypeDescriptor::primitive(prim.clone()),
                visibility: Visibility::Public,
                capabilities: crate::vec![],
                idempotent: false,
                doc: None,
            };
            functions.insert((*name).to_owned(), f);
        }
        schema.namespaces.insert(
            "types_ns".into(),
            NamespaceSchema {
                functions: crate::Box::new(functions),
                doc: None,
            },
        );

        let gen = PythonGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;
        let client = output
            .files
            .iter()
            .find(|f| f.path == "types_ns_client.py")
            .ok_or("types_ns_client.py")?;

        for (fn_name, _, expected_type) in primitive_cases {
            assert!(
                client.content.contains(expected_type),
                "Python type '{}' not found in client for fn '{}'\nContent:\n{}",
                expected_type,
                fn_name,
                client.content
            );
        }
        Ok(())
    }

    fn typescript_empty_schema_produces_required_files() -> Result<(), &'static str> {
        let schema = Schema::new();
        let gen = TypeScriptGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let paths: crate::Vec<_> = output.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"types.ts"), "types.ts missing");
        assert!(paths.contains(&"index.ts"), "index.ts missing");
        Ok(())
    }

    fn typescript_generates_client_per_namespace() -> Result<(), &'static str> {
        let schema = make_schema_with_math();
        let gen = TypeScriptGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let paths: crate::Vec<_> = output.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"MathClient.ts"), "MathClient.ts missing");
        Ok(())
    }

    fn typescript_public_and_internal_functions_are_generated() -> Result<(), &'static str> {
        let schema = make_schema_with_math();
        let gen = TypeScriptGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let client = output
            .files
            .iter()
            .find(|f| f.path == "MathClient.ts")
            .ok_or("MathClient.ts")?;

        assert!(client.content.contains("async add"), "add missing");
        assert!(client.content.contains("async sub"), "sub missing");
        Ok(())
    }

    fn typescript_private_functions_are_omitted() -> Result<(), &'static str> {
        let schema = make_schema_with_math();
        let gen = TypeScriptGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let client = output
            .files
            .iter()
            .find(|f| f.path == "MathClient.ts")
            .ok_or("MathClient.ts")?;

        assert!(
            !client.content.contains("async secret"),
            "private fn 'secret' should not be generated"
        );
        Ok(())
    }

    fn typescript_index_exports_all_clients() -> Result<(), &'static str> {
        let schema = make_schema_with_math();
        let gen = TypeScriptGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let index = output
            .files
            .iter()
            .find(|f| f.path == "index.ts")
            .ok_or("index.ts")?;

        assert!(
            index.content.contains("MathClient"),
            "index.ts should export MathClient"
        );
        Ok(())
    }

    fn typescript_types_file_contains_interface() -> Result<(), &'static str> {
        let schema = make_schema_with_types();
        let gen = TypeScriptGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let types = output
            .files
            .iter()
            .find(|f| f.path == "types.ts")
            .ok_or("types.ts")?;

        assert!(
            types.content.contains("interface Person"),
            "Person interface missing"
        );
        assert!(types.content.contains("name: string"), "name field missing");
        Ok(())
    }

    fn typescript_types_file_contains_enum_union() -> Result<(), &'static str> {
        let schema = make_schema_with_types();
        let gen = TypeScriptGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let types = output
            .files
            .iter()
            .find(|f| f.path == "types.ts")
            .ok_or("types.ts")?;

        assert!(types.content.contains("Color"), "Color type missing");
        assert!(types.content.contains("\"Red\""), "Red variant missing");
        Ok(())
    }

    fn typescript_all_primitive_types_map_correctly() -> Result<(), &'static str> {
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
        let mut functions = FunctionMap::new();
        for (name, prim, _) in primitive_cases {
            let f = FunctionSchema {
                args: crate::vec![],
                returns: TypeDescriptor::primitive(prim.clone()),
                visibility: Visibility::Public,
                capabilities: crate::vec![],
                idempotent: false,
                doc: None,
            };
            functions.insert((*name).to_owned(), f);
        }
        schema.namespaces.insert(
            "types_ns".into(),
            NamespaceSchema {
                functions: crate::Box::new(functions),
                doc: None,
            },
        );

        let gen = TypeScriptGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;
        let client = output
            .files
            .iter()
            .find(|f| f.path == "TypesNsClient.ts")
            .ok_or("TypesNsClient.ts")?;

        for (fn_name, _, expected_type) in primitive_cases {
            assert!(
                client.content.contains(expected_type),
                "TS type '{}' not found for fn '{}'\nContent:\n{}",
                expected_type,
                fn_name,
                client.content
            );
        }
        Ok(())
    }

    fn typescript_optional_arg_has_question_mark() -> Result<(), &'static str> {
        let mut schema = Schema::new();
        let mut functions = FunctionMap::new();
        functions.insert(
            "greet".into(),
            FunctionSchema {
                args: crate::vec![
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
                capabilities: crate::vec![],
                idempotent: false,
                doc: None,
            },
        );
        schema.namespaces.insert(
            "greeter".into(),
            NamespaceSchema {
                functions: crate::Box::new(functions),
                doc: None,
            },
        );

        let gen = TypeScriptGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;
        let client = output
            .files
            .iter()
            .find(|f| f.path == "GreeterClient.ts")
            .ok_or("GreeterClient.ts")?;

        assert!(
            client.content.contains("greeting?:"),
            "optional arg 'greeting' should have '?' in TS signature"
        );
        Ok(())
    }

    fn python_stream_method_calls_stream_not_call() -> Result<(), &'static str> {
        let schema = make_schema_with_stream_and_channel();
        let gen = PythonGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let client = output
            .files
            .iter()
            .find(|f| f.path == "events_client.py")
            .ok_or("events_client.py")?;

        assert!(
            client
                .content
                .contains("self._client.stream(\"events.subscribe\""),
            "subscribe should call stream(), not call(). Content:\n{}",
            client.content
        );
        assert!(
            client.content.contains("AsyncIterator[str]"),
            "subscribe return type should be AsyncIterator[str]. Content:\n{}",
            client.content
        );
        assert!(
            client.content.contains("self._client.call(\"events.ping\""),
            "ping should still call call(). Content:\n{}",
            client.content
        );
        Ok(())
    }

    fn python_channel_method_calls_channel_not_call() -> Result<(), &'static str> {
        let schema = make_schema_with_stream_and_channel();
        let gen = PythonGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let client = output
            .files
            .iter()
            .find(|f| f.path == "events_client.py")
            .ok_or("events_client.py")?;

        assert!(
            client
                .content
                .contains("self._client.channel(\"events.chat\""),
            "chat should call channel(), not call(). Content:\n{}",
            client.content
        );
        assert!(
            client.content.contains("SaikuroChannel"),
            "channel method should reference SaikuroChannel. Content:\n{}",
            client.content
        );
        assert!(
            client.content.contains("# from saikuro import"),
            "stream/channel import hint comment should appear. Content:\n{}",
            client.content
        );
        Ok(())
    }

    fn typescript_stream_method_calls_stream_not_call() -> Result<(), &'static str> {
        let schema = make_schema_with_stream_and_channel();
        let gen = TypeScriptGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let client = output
            .files
            .iter()
            .find(|f| f.path == "EventsClient.ts")
            .ok_or("EventsClient.ts")?;

        assert!(
            client
                .content
                .contains("this.client.stream<string>('events.subscribe'"),
            "subscribe should call stream(), not call(). Content:\n{}",
            client.content
        );
        assert!(
            client.content.contains("Promise<AsyncIterable<string>>"),
            "subscribe return type should be Promise<AsyncIterable<string>>. Content:\n{}",
            client.content
        );
        assert!(
            client.content.contains("this.client.call('events.ping'"),
            "ping should still call call(). Content:\n{}",
            client.content
        );
        Ok(())
    }

    fn typescript_channel_method_calls_channel_not_call() -> Result<(), &'static str> {
        let schema = make_schema_with_stream_and_channel();
        let gen = TypeScriptGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let client = output
            .files
            .iter()
            .find(|f| f.path == "EventsClient.ts")
            .ok_or("EventsClient.ts")?;

        assert!(
            client
                .content
                .contains("this.client.channel<string, string>('events.chat'"),
            "chat should call channel(), not call(). Content:\n{}",
            client.content
        );
        assert!(
            client
                .content
                .contains("Promise<SaikuroChannel<string, string>>"),
            "chat return type should be Promise<SaikuroChannel<...>>. Content:\n{}",
            client.content
        );
        assert!(
            client.content.contains("import type { SaikuroChannel }"),
            "SaikuroChannel import should appear when channel functions exist. Content:\n{}",
            client.content
        );
        Ok(())
    }

    fn csharp_empty_schema_produces_required_files() -> Result<(), &'static str> {
        let schema = Schema::new();
        let gen = CSharpGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let paths: crate::Vec<_> = output.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"Types.cs"), "Types.cs missing");
        assert!(paths.contains(&"Generated.cs"), "Generated.cs missing");
        Ok(())
    }

    fn csharp_generates_client_per_namespace() -> Result<(), &'static str> {
        let schema = make_schema_with_math();
        let gen = CSharpGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let paths: crate::Vec<_> = output.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"MathClient.cs"), "MathClient.cs missing");
        Ok(())
    }

    fn csharp_public_and_internal_functions_are_generated() -> Result<(), &'static str> {
        let schema = make_schema_with_math();
        let gen = CSharpGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let client = output
            .files
            .iter()
            .find(|f| f.path == "MathClient.cs")
            .ok_or("MathClient.cs")?;

        assert!(
            client.content.contains("AddAsync"),
            "add should appear as AddAsync"
        );
        assert!(
            client.content.contains("SubAsync"),
            "sub should appear as SubAsync"
        );
        Ok(())
    }

    fn csharp_private_functions_are_omitted() -> Result<(), &'static str> {
        let schema = make_schema_with_math();
        let gen = CSharpGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let client = output
            .files
            .iter()
            .find(|f| f.path == "MathClient.cs")
            .ok_or("MathClient.cs")?;

        assert!(
            !client.content.contains("SecretAsync"),
            "private fn 'secret' should not be generated"
        );
        Ok(())
    }

    fn csharp_generated_file_lists_clients() -> Result<(), &'static str> {
        let schema = make_schema_with_math();
        let gen = CSharpGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let index = output
            .files
            .iter()
            .find(|f| f.path == "Generated.cs")
            .ok_or("Generated.cs")?;

        assert!(
            index.content.contains("MathClient"),
            "Generated.cs should mention MathClient"
        );
        Ok(())
    }

    fn csharp_types_file_contains_record() -> Result<(), &'static str> {
        let schema = make_schema_with_types();
        let gen = CSharpGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let types = output
            .files
            .iter()
            .find(|f| f.path == "Types.cs")
            .ok_or("Types.cs")?;

        assert!(types.content.contains("Person"), "Person record missing");
        assert!(
            types.content.contains("string"),
            "string field type missing"
        );
        Ok(())
    }

    fn csharp_types_file_contains_enum() -> Result<(), &'static str> {
        let schema = make_schema_with_types();
        let gen = CSharpGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let types = output
            .files
            .iter()
            .find(|f| f.path == "Types.cs")
            .ok_or("Types.cs")?;

        assert!(types.content.contains("Color"), "Color enum missing");
        assert!(types.content.contains("Red"), "Red variant missing");
        Ok(())
    }

    fn csharp_types_file_contains_alias() -> Result<(), &'static str> {
        let schema = make_schema_with_types();
        let gen = CSharpGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let types = output
            .files
            .iter()
            .find(|f| f.path == "Types.cs")
            .ok_or("Types.cs")?;

        assert!(types.content.contains("UserId"), "UserId alias missing");
        Ok(())
    }

    fn csharp_all_primitive_types_map_correctly() -> Result<(), &'static str> {
        let primitive_cases: &[(&str, PrimitiveType, &str)] = &[
            ("b_fn", PrimitiveType::Bool, "bool"),
            ("i_fn", PrimitiveType::I64, "long"),
            ("f_fn", PrimitiveType::F64, "double"),
            ("s_fn", PrimitiveType::String, "string"),
            ("by_fn", PrimitiveType::Bytes, "byte[]"),
            ("any_fn", PrimitiveType::Any, "object?"),
        ];

        let mut schema = Schema::new();
        let mut functions = FunctionMap::new();
        for (name, prim, _) in primitive_cases {
            let f = FunctionSchema {
                args: crate::vec![],
                returns: TypeDescriptor::primitive(prim.clone()),
                visibility: Visibility::Public,
                capabilities: crate::vec![],
                idempotent: false,
                doc: None,
            };
            functions.insert((*name).to_owned(), f);
        }
        schema.namespaces.insert(
            "types_ns".into(),
            NamespaceSchema {
                functions: crate::Box::new(functions),
                doc: None,
            },
        );

        let gen = CSharpGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;
        let client = output
            .files
            .iter()
            .find(|f| f.path == "TypesNsClient.cs")
            .ok_or("TypesNsClient.cs")?;

        for (fn_name, _, expected_type) in primitive_cases {
            assert!(
                client.content.contains(expected_type),
                "C# type '{}' not found for fn '{}'\nContent:\n{}",
                expected_type,
                fn_name,
                client.content
            );
        }
        Ok(())
    }

    fn csharp_stream_method_uses_stream_async() -> Result<(), &'static str> {
        let schema = make_schema_with_stream_and_channel();
        let gen = CSharpGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let client = output
            .files
            .iter()
            .find(|f| f.path == "EventsClient.cs")
            .ok_or("EventsClient.cs")?;

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
        Ok(())
    }

    fn csharp_channel_method_uses_channel_async() -> Result<(), &'static str> {
        let schema = make_schema_with_stream_and_channel();
        let gen = CSharpGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let client = output
            .files
            .iter()
            .find(|f| f.path == "EventsClient.cs")
            .ok_or("EventsClient.cs")?;

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
        Ok(())
    }

    fn rust_empty_schema_produces_required_files() -> Result<(), &'static str> {
        let schema = Schema::new();
        let gen = RustGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let paths: crate::Vec<_> = output.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"types.rs"), "types.rs missing");
        assert!(paths.contains(&"mod.rs"), "mod.rs missing");
        Ok(())
    }

    fn rust_generates_client_per_namespace() -> Result<(), &'static str> {
        let schema = make_schema_with_math();
        let gen = RustGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let paths: crate::Vec<_> = output.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"math_client.rs"), "math_client.rs missing");
        Ok(())
    }

    fn rust_private_functions_are_omitted() -> Result<(), &'static str> {
        let schema = make_schema_with_math();
        let gen = RustGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let client = output
            .files
            .iter()
            .find(|f| f.path == "math_client.rs")
            .ok_or("math_client.rs")?;

        assert!(client.content.contains("pub async fn add"), "add missing");
        assert!(client.content.contains("pub async fn sub"), "sub missing");
        assert!(
            !client.content.contains("pub async fn secret"),
            "private fn 'secret' should not be generated"
        );
        Ok(())
    }

    fn rust_stream_and_channel_methods_use_adapter_primitives() -> Result<(), &'static str> {
        let schema = make_schema_with_stream_and_channel();
        let gen = RustGenerator;
        let output = gen.generate(&schema).map_err(|_| "generate")?;

        let client = output
            .files
            .iter()
            .find(|f| f.path == "events_client.rs")
            .ok_or("events_client.rs")?;

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
        Ok(())
    }
}

pub mod c_cpp;

pub fn register(suite: &mut TestSuite) {
    binding_output::register(suite);
    c_cpp::register(suite);
}
