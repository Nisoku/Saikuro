//! Saikuro Runtime
//!
//! This is the top-level orchestrator that wires together every component:

pub mod config;
pub mod connection;
pub mod error;
pub mod handle;
pub mod runtime;

pub use config::RuntimeConfig;
pub use error::RuntimeError;
pub use handle::RuntimeHandle;
pub use runtime::SaikuroRuntime;

// A small number of doc-test-only inline tests live here
#[cfg(test)]
mod tests {
    use crate::runtime::SaikuroRuntime;
    use saikuro_core::schema::{
        FunctionSchema, NamespaceSchema, PrimitiveType, Schema, TypeDescriptor, Visibility,
    };
    use std::collections::HashMap;

    /// Smoke test: build a runtime, register a schema, verify lookup works.
    #[test]
    fn schema_registration_roundtrip() {
        let rt = SaikuroRuntime::builder().build();

        let mut functions = HashMap::new();
        functions.insert(
            "ping".to_owned(),
            FunctionSchema {
                args: vec![],
                returns: TypeDescriptor::primitive(PrimitiveType::String),
                visibility: Visibility::Public,
                capabilities: vec![],
                idempotent: true,
                doc: Some("Returns 'pong'".to_owned()),
            },
        );

        let ns = NamespaceSchema {
            functions,
            doc: None,
        };

        let mut schema = Schema::new();
        schema.namespaces.insert("health".to_owned(), ns);

        rt.schema_registry()
            .merge_schema(schema, "test-provider")
            .expect("merge failed");

        let func_ref = rt
            .schema_registry()
            .lookup_function("health.ping")
            .expect("lookup failed");

        assert_eq!(func_ref.function, "ping");
        assert_eq!(func_ref.provider_id, "test-provider");
    }
}
