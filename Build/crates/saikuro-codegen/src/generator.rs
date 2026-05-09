//! Common generator traits and output types.

use std::collections::BTreeMap;

use saikuro_core::schema::{FieldDescriptor, Schema, TypeDefinition, TypeDescriptor};

use crate::error::Result;

/// A single generated source file.
#[derive(Debug, Clone)]
pub struct GeneratedFile {
    /// Relative path where this file should be written (e.g. `"math.py"`).
    pub path: String,
    /// The source code content.
    pub content: String,
}

/// All files produced by a single code generation run.
#[derive(Debug, Default)]
pub struct GeneratorOutput {
    pub files: Vec<GeneratedFile>,
}

impl GeneratorOutput {
    pub fn add(&mut self, path: impl Into<String>, content: impl Into<String>) {
        self.files.push(GeneratedFile {
            path: path.into(),
            content: content.into(),
        });
    }
}

/// A language-specific binding generator.
pub trait BindingGenerator {
    /// Generate bindings from the given schema.
    fn generate(&self, schema: &Schema) -> Result<GeneratorOutput>;
}

// Shared utilities

/// Convert a snake_case string to PascalCase.
pub fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|part| {
            let mut c = part.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect()
}

/// Convert a snake_case string to camelCase.
pub fn to_camel_case(s: &str) -> String {
    let pascal = to_pascal_case(s);
    let mut c = pascal.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
    }
}

/// Shared iteration + match over all schema types.
///
/// Every codegen backend iterates `schema.types` and dispatches on
/// `TypeDefinition::{Record, Enum, Alias}`. This function captures that
/// common skeleton; each backend provides language-specific generation
/// for each variant via the three closures.
pub fn generate_types_from_schema(
    schema: &Schema,
    header: Vec<String>,
    mut on_record: impl FnMut(&str, &BTreeMap<String, FieldDescriptor>) -> Result<Vec<String>>,
    mut on_enum: impl FnMut(&str, &[String]) -> Result<Vec<String>>,
    mut on_alias: impl FnMut(&str, &TypeDescriptor) -> Result<Vec<String>>,
) -> Result<String> {
    let mut lines = header;
    for (type_name, type_def) in &schema.types {
        match type_def {
            TypeDefinition::Record { fields } => {
                lines.extend(on_record(type_name, fields)?);
            }
            TypeDefinition::Enum { variants } => {
                lines.extend(on_enum(type_name, variants)?);
            }
            TypeDefinition::Alias { inner } => {
                lines.extend(on_alias(type_name, inner)?);
            }
        }
    }
    Ok(lines.join("\n"))
}
