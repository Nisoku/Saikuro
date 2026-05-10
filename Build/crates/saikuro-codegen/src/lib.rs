//! Saikuro Codegen
//!
//! Generates typed language bindings from a frozen [`Schema`].
//!
//! Currently supported targets:
//! - [`python`] :  Python 3 dataclasses + async client stubs
//! - [`typescript`] :  TypeScript interfaces + async client stubs
//! - [`csharp`] :  C# records + async client stubs
//! - [`c`] :  C headers with namespace client helpers over the C adapter ABI
//! - [`cpp`] :  C++ wrappers with typed class stubs over the C adapter ABI
//! - [`rust`] :  Rust bindings with async client and type-safe wrappers
//!
//! The codegen pipeline is:
//! 1. Load a [`Schema`] (from JSON file or in-process snapshot).
//! 2. Pass it through a [`BindingGenerator`] for the target language.
//! 3. Write the output files.

pub mod c;
pub mod cpp;
pub mod csharp;
pub mod error;
pub mod generator;
pub mod python;
pub mod rust;
pub mod typescript;

pub use error::CodegenError;
pub use generator::{
    convert_type, generate_types_and_namespace_clients, generate_types_from_schema,
    namespace_public_functions, to_camel_case, to_pascal_case, BindingGenerator, GeneratedFile,
    GeneratorOutput, TypeConverter,
};
