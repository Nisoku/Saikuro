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
pub mod typescript;

pub use error::CodegenError;
pub use generator::{BindingGenerator, GeneratedFile, GeneratorOutput};
