
use crate::shared_test;
use crate::TestSuite;
use crate::ToOwned;
use saikuro_codegen::language::{c::CGenerator, cpp::CppGenerator};
use saikuro_codegen::{BindingGenerator, GeneratorOutput};
use saikuro_core::schema::{
    FunctionSchema, NamespaceSchema, PrimitiveType, Schema, TypeDescriptor, Visibility,
};

fn sample_schema() -> Schema {
    let mut schema = Schema::new();

    let mut ns = NamespaceSchema {
        functions: crate::Box::default(),
        doc: Some("Math functions".to_owned()),
    };

    ns.functions.insert(
        "add".to_owned(),
        FunctionSchema {
            args: crate::vec![],
            returns: TypeDescriptor::primitive(PrimitiveType::I64),
            visibility: Visibility::Public,
            capabilities: crate::vec![],
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

pub fn register(suite: &mut TestSuite) {
    shared_test!(suite,
        "codegen::c_generator_emits_expected_files",
        c_generator_emits_expected_files,
    );
    shared_test!(suite,
        "codegen::cpp_generator_emits_expected_files",
        cpp_generator_emits_expected_files,
    );
}

fn c_generator_emits_expected_files() -> Result<(), &'static str> {
    let schema = sample_schema();
    let output = CGenerator
        .generate(&schema)
        .map_err(|_| "C generation should succeed")?;

    assert!(has_file(&output, "saikuro_types.h"));
    assert!(has_file(&output, "saikuro_generated.h"));
    assert!(has_file(&output, "math_client.h"));

    let math = output
        .files
        .iter()
        .find(|f| f.path == "math_client.h")
        .ok_or("math client file must exist")?;

    assert!(math.content.contains("saikuro_client_call_json"));
    assert!(math.content.contains("math.add"));
    Ok(())
}

fn cpp_generator_emits_expected_files() -> Result<(), &'static str> {
    let schema = sample_schema();
    let output = CppGenerator
        .generate(&schema)
        .map_err(|_| "C++ generation should succeed")?;

    assert!(has_file(&output, "saikuro_generated.hpp"));
    assert!(has_file(&output, "MathClient.hpp"));

    let class_file = output
        .files
        .iter()
        .find(|f| f.path == "MathClient.hpp")
        .ok_or("Math client file must exist")?;

    assert!(class_file.content.contains("class MathClient"));
    assert!(class_file.content.contains("client_.call_json"));
    assert!(class_file.content.contains("math.add"));
    Ok(())
}
