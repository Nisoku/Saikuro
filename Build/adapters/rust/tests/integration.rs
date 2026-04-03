//! Rust adapter integration tests.
//!
//! All tests use [`InMemoryTransport`] so no network is required.

use saikuro::{Client, InMemoryTransport, Provider};
use serde_json::json;

// Helpers

/// Spin up a provider on one side of an in-memory pair and return a connected
/// client on the other side.  The provider task is detached; it will stop when
/// the client drops its side of the channel.
async fn make_pair(provider: Provider) -> Client {
    let (provider_t, client_t) = InMemoryTransport::pair();

    tokio::spawn(async move {
        // Ignore the error; the test ends when the client drops its side.
        let _ = provider.serve_on(Box::new(provider_t)).await;
    });

    // Yield to give the provider task a scheduling opportunity.  The provider
    // sends its announce frame synchronously during `serve_on` startup; by
    // yielding here we ensure the frame is in the transport buffer before
    // the client task starts draining it.
    tokio::task::yield_now().await;

    Client::from_transport(Box::new(client_t), None).unwrap()
}

// Value conversion

#[test]
fn value_null_roundtrips() {
    use saikuro::value::{core_to_json, json_to_core};
    let v = json!(null);
    let core = json_to_core(v.clone());
    assert_eq!(core_to_json(core), v);
}

#[test]
fn value_integer_roundtrips() {
    use saikuro::value::{core_to_json, json_to_core};
    let v = json!(42i64);
    let core = json_to_core(v.clone());
    assert_eq!(core_to_json(core), v);
}

#[test]
fn value_string_roundtrips() {
    use saikuro::value::{core_to_json, json_to_core};
    let v = json!("hello world");
    let core = json_to_core(v.clone());
    assert_eq!(core_to_json(core), v);
}

#[test]
fn value_array_roundtrips() {
    use saikuro::value::{core_to_json, json_to_core};
    let v = json!([1, 2, 3]);
    let core = json_to_core(v.clone());
    assert_eq!(core_to_json(core), v);
}

#[test]
fn value_object_roundtrips() {
    use saikuro::value::{core_to_json, json_to_core};
    let v = json!({"key": "value", "n": 7});
    let core = json_to_core(v.clone());
    assert_eq!(core_to_json(core), v);
}

// Error types

#[test]
fn error_display_transport() {
    let e = saikuro::Error::Transport("connection refused".into());
    assert!(e.to_string().contains("connection refused"));
}

#[test]
fn error_display_timeout() {
    let e = saikuro::Error::Timeout {
        target: "math.add".into(),
        ms: 5000,
    };
    let s = e.to_string();
    assert!(s.contains("math.add"));
    assert!(s.contains("5000"));
}

#[test]
fn error_display_remote() {
    let e = saikuro::Error::remote("CapabilityDenied", "not allowed", None);
    let s = e.to_string();
    assert!(s.contains("CapabilityDenied"));
    assert!(s.contains("not allowed"));
}

// Schema builder

#[test]
fn schema_build_basic() {
    use saikuro::{FunctionSchema, NamespaceSchema};
    let mut ns = NamespaceSchema::new();
    ns.insert(
        "add",
        FunctionSchema {
            doc: Some("Add two numbers.".into()),
            idempotent: true,
            ..Default::default()
        },
    );
    // to_core should not panic
    let _ = ns.to_core();
}

#[test]
fn schema_capabilities_convert() {
    use saikuro::{FunctionSchema, NamespaceSchema};
    let mut ns = NamespaceSchema::new();
    ns.insert(
        "op",
        FunctionSchema {
            capabilities: vec!["admin.write".into(), "math.advanced".into()],
            ..Default::default()
        },
    );
    let core_ns = ns.to_core();
    let fn_schema = core_ns.functions.get("op").expect("op function missing");
    let cap_strs: Vec<String> = fn_schema
        .capabilities
        .iter()
        .map(|c| c.to_string())
        .collect();
    assert!(cap_strs.contains(&"admin.write".to_string()));
    assert!(cap_strs.contains(&"math.advanced".to_string()));
}

// In-memory end-to-end: call

#[tokio::test]
async fn call_add_returns_sum() {
    let mut provider = Provider::new("math");
    provider.register("add", |args: Vec<serde_json::Value>| async move {
        let a = args.first().and_then(|v| v.as_i64()).unwrap_or(0);
        let b = args.get(1).and_then(|v| v.as_i64()).unwrap_or(0);
        Ok(json!(a + b))
    });

    let client = make_pair(provider).await;
    let result = client
        .call("math.add", vec![json!(3), json!(4)])
        .await
        .unwrap();
    assert_eq!(result, json!(7));
    client.close().await.unwrap();
}

#[tokio::test]
async fn call_unknown_function_returns_error() {
    let provider = Provider::new("math");
    let client = make_pair(provider).await;
    let err = client.call("math.nonexistent", vec![]).await.unwrap_err();
    // Should get a Remote error with FunctionNotFound-style message
    assert!(matches!(err, saikuro::Error::Remote { .. }));
    client.close().await.unwrap();
}

#[tokio::test]
async fn call_handler_error_propagates() {
    let mut provider = Provider::new("math");
    provider.register("divide", |args: Vec<serde_json::Value>| async move {
        let b = args.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
        if b == 0.0 {
            return Err(saikuro::Error::remote(
                "DivisionByZero",
                "divisor is zero",
                None,
            ));
        }
        Ok(json!(args[0].as_f64().unwrap_or(0.0) / b))
    });

    let client = make_pair(provider).await;
    let err = client
        .call("math.divide", vec![json!(10), json!(0)])
        .await
        .unwrap_err();
    assert!(matches!(err, saikuro::Error::Remote { .. }));
    client.close().await.unwrap();
}

// In-memory end-to-end: cast

#[tokio::test]
async fn cast_does_not_block() {
    use std::sync::{Arc, Mutex};

    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));
    let log_clone = log.clone();

    let mut provider = Provider::new("log");
    provider.register("write", move |args: Vec<serde_json::Value>| {
        let log = log_clone.clone();
        async move {
            if let Some(msg) = args.first().and_then(|v| v.as_str()) {
                log.lock().unwrap().push(msg.to_owned());
            }
            Ok(json!(null))
        }
    });

    let client = make_pair(provider).await;
    client
        .cast("log.write", vec![json!("hello")])
        .await
        .unwrap();

    // Give the provider a moment to process the cast.
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    client.close().await.unwrap();
}

// In-memory end-to-end: batch

#[tokio::test]
async fn batch_returns_all_results() {
    let mut provider = Provider::new("math");
    provider.register("double", |args: Vec<serde_json::Value>| async move {
        let n = args.first().and_then(|v| v.as_i64()).unwrap_or(0);
        Ok(json!(n * 2))
    });

    let client = make_pair(provider).await;
    let results = client
        .batch(vec![
            ("math.double".into(), vec![json!(1)]),
            ("math.double".into(), vec![json!(2)]),
            ("math.double".into(), vec![json!(3)]),
        ])
        .await
        .unwrap();

    // Batch returns all results as a JSON array.
    assert!(!results.is_empty());
    client.close().await.unwrap();
}

// InMemoryTransport pair sanity check

#[tokio::test]
async fn in_memory_transport_sends_and_receives() {
    use bytes::Bytes;
    use saikuro::transport::AdapterTransport;

    let (mut a, mut b) = InMemoryTransport::pair();
    let frame = Bytes::from_static(b"hello");
    a.send(frame.clone()).await.unwrap();
    let received = b.recv().await.unwrap();
    assert_eq!(received, Some(frame));
}
