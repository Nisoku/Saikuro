use saikuro_core::envelope::{Envelope, InvocationType};
use saikuro_event::SaikuroError;
use saikuro_schema::registry::SchemaRegistry;
use saikuro_schema::validator::InvocationValidator;

#[test]
fn batch_with_empty_items_returns_empty_batch_error() {
    let registry = SchemaRegistry::new();
    let validator = InvocationValidator::new(registry);

    let mut batch = Envelope::call("", vec![]).expect("entropy available");
    batch.invocation_type = InvocationType::Batch;
    batch.target = String::new();
    batch.batch_items = Some(vec![]);

    let result = validator.validate(&batch);
    assert!(matches!(result, Err(SaikuroError::EmptyBatch)));
}
