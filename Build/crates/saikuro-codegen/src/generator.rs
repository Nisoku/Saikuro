//! Common generator traits and output types.

use saikuro_core::schema::Schema;

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
