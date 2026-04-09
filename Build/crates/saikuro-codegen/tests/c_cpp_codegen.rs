use saikuro_codegen::{
    c::CGenerator, cpp::CppGenerator, generator::BindingGenerator, GeneratorOutput,
};
use saikuro_core::schema::{
    FunctionSchema, NamespaceSchema, PrimitiveType, Schema, TypeDescriptor, Visibility,
};

fn sample_schema() -> Schema {
    let mut schema = Schema::new();

    let mut ns = NamespaceSchema {
        functions: Default::default(),
        doc: Some("Math functions".to_owned()),
    };

    ns.functions.insert(
        "add".to_owned(),
        FunctionSchema {
            args: vec![],
            returns: TypeDescriptor::primitive(PrimitiveType::I64),
            visibility: Visibility::Public,
            capabilities: vec![],
            idempotent: true,
            doc: Some("Add two values".to_owned()),
        },
    );

    schema.namespaces.insert("math".to_owned(), ns);

    schema
}

fn has_file(output: &GeneratorOutput, name: &str) -> bool {
    output.files.iter().any(|f| f.path == name)
}

#[test]
fn c_generator_emits_expected_files() {
    let schema = sample_schema();
    let output = CGenerator
        .generate(&schema)
        .expect("C generation should succeed");

    assert!(has_file(&output, "saikuro_types.h"));
    assert!(has_file(&output, "saikuro_generated.h"));
    assert!(has_file(&output, "math_client.h"));

    let math = output
        .files
        .iter()
        .find(|f| f.path == "math_client.h")
        .expect("math client file must exist");

    assert!(math.content.contains("saikuro_client_call_json"));
    assert!(math.content.contains("math.add"));
}

#[test]
fn cpp_generator_emits_expected_files() {
    let schema = sample_schema();
    let output = CppGenerator
        .generate(&schema)
        .expect("C++ generation should succeed");

    assert!(has_file(&output, "saikuro_generated.hpp"));
    assert!(has_file(&output, "MathClient.hpp"));

    let class_file = output
        .files
        .iter()
        .find(|f| f.path == "MathClient.hpp")
        .expect("Math client file must exist");

    assert!(class_file.content.contains("class MathClient"));
    assert!(class_file.content.contains("client_.call_json"));
    assert!(class_file.content.contains("math.add"));
}
