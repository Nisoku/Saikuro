//! Math example
//!
//! Wires a math provider and a client together over an in-memory transport,
//! then exercises call, cast, batch, and error-handling paths.
//!
//! Run with:
//!   cargo run -p math

use saikuro::{Client, Error, InMemoryTransport, Provider, Result};
use serde_json::Value as JsonValue;

fn extract_two_floats(args: &[JsonValue]) -> (f64, f64) {
    let a = args.first().and_then(JsonValue::as_f64).unwrap_or(0.0);
    let b = args.get(1).and_then(JsonValue::as_f64).unwrap_or(0.0);
    (a, b)
}

fn main() -> Result<()> {
    saikuro_exec::block_on(async_main())
}

async fn async_main() -> Result<()> {
    // provider setup

    let mut provider = Provider::new("math");

    provider.register("add", |args: Vec<JsonValue>| async move {
        let (a, b) = extract_two_floats(&args);
        Ok(serde_json::json!(a + b))
    });

    provider.register("subtract", |args: Vec<JsonValue>| async move {
        let (a, b) = extract_two_floats(&args);
        Ok(serde_json::json!(a - b))
    });

    provider.register("multiply", |args: Vec<JsonValue>| async move {
        let (a, b) = extract_two_floats(&args);
        Ok(serde_json::json!(a * b))
    });

    provider.register("divide", |args: Vec<JsonValue>| async move {
        let (a, b) = extract_two_floats(&args);
        if b == 0.0 {
            return Err(Error::InvalidState("division by zero".into()));
        }
        Ok(serde_json::json!(a / b))
    });

    // wire provider + client over in-memory transport

    let (provider_transport, client_transport) = InMemoryTransport::pair();

    saikuro_exec::spawn(async move {
        let _ = provider.serve_on(Box::new(provider_transport)).await;
    });

    // Give the provider task a chance to start and send its announce frame.
    saikuro_exec::yield_now().await;

    let client = Client::from_transport(Box::new(client_transport), None)?;

    // call

    let sum = client
        .call(
            "math.add",
            vec![serde_json::json!(10), serde_json::json!(32)],
        )
        .await?;
    println!("math.add(10, 32) = {sum}");
    assert_eq!(sum, serde_json::json!(42.0));

    let diff = client
        .call(
            "math.subtract",
            vec![serde_json::json!(100), serde_json::json!(58)],
        )
        .await?;
    println!("math.subtract(100, 58) = {diff}");
    assert_eq!(diff, serde_json::json!(42.0));

    let product = client
        .call(
            "math.multiply",
            vec![serde_json::json!(6), serde_json::json!(7)],
        )
        .await?;
    println!("math.multiply(6, 7) = {product}");
    assert_eq!(product, serde_json::json!(42.0));

    let quotient = client
        .call(
            "math.divide",
            vec![serde_json::json!(84.0), serde_json::json!(2.0)],
        )
        .await?;
    println!("math.divide(84, 2) = {quotient}");
    assert_eq!(quotient, serde_json::json!(42.0));

    // cast (fire-and-forget)

    client
        .cast("math.add", vec![serde_json::json!(1), serde_json::json!(1)])
        .await?;
    println!("cast sent (no response expected)");

    // batch

    let results = client
        .batch(vec![
            (
                "math.add".into(),
                vec![serde_json::json!(1), serde_json::json!(2)],
            ),
            (
                "math.multiply".into(),
                vec![serde_json::json!(3), serde_json::json!(4)],
            ),
        ])
        .await?;
    println!("batch [add(1,2), multiply(3,4)] = {results:?}");

    // error handling

    let err = client
        .call(
            "math.divide",
            vec![serde_json::json!(1), serde_json::json!(0)],
        )
        .await
        .unwrap_err();
    println!("divide by zero caught: {err}");
    assert!(matches!(err, Error::Remote { .. } | Error::InvalidState(_)));

    client.close().await?;
    println!("all examples passed");
    Ok(())
}
