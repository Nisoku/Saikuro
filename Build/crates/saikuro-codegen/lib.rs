pub mod shared;
pub mod language;

pub use shared::error::CodegenError;
pub use shared::generator::{
    convert_type, generate_types_and_namespace_clients, generate_types_from_schema,
    namespace_public_functions, to_camel_case, to_pascal_case, BindingGenerator, GeneratedFile,
    GeneratorOutput, TypeConverter,
};
