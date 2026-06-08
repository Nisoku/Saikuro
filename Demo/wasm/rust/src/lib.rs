use wasm_bindgen::prelude::*;

use saikuro::{Provider, RegisterOptions};
use saikuro::schema::{FunctionSchema, ArgDescriptor};
use saikuro::{PrimitiveType, TypeDescriptor};
use serde_json::Value as JsonValue;

#[wasm_bindgen]
pub async fn start_rust_provider(channel: String) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    let mut provider = Provider::new("rust");
    provider.register_with_options(
        "sentiment",
        |args: Vec<JsonValue>| async move {
            Ok(sentiment_score(args))
        },
        RegisterOptions {
            schema: Some(FunctionSchema {
                doc: Some("Simple keyword-based sentiment analysis (Rust WASM)".into()),
                args: vec![ArgDescriptor {
                    name: "text".into(),
                    r#type: TypeDescriptor::Primitive {
                        r#type: PrimitiveType::String,
                    },
                    optional: false,
                    doc: Some("Input text to analyze".into()),
                }],
                returns: Some(TypeDescriptor::Primitive {
                    r#type: PrimitiveType::Any,
                }),
                ..Default::default()
            }),
        },
    );

    provider
        .serve(format!("wasm-host://{channel}"))
        .await
        .map_err(|e| JsValue::from_str(&format!("rust provider error: {e}")))?;

    Ok(())
}

fn sentiment_score(args: Vec<JsonValue>) -> JsonValue {
    let text = args
        .get(0)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_lowercase();

    let positives = ["great", "good", "fast", "clean", "love", "light", "sharp"];
    let negatives = ["bad", "slow", "broken", "hard", "confusing"];

    let mut score: f64 = 0.0;
    for word in positives {
        if text.contains(word) {
            score += 0.15;
        }
    }
    for word in negatives {
        if text.contains(word) {
            score -= 0.2;
        }
    }

    let label = if score > 0.2 {
        "positive"
    } else if score < -0.2 {
        "negative"
    } else {
        "neutral"
    };

    let tags = if label == "positive" {
        vec!["energetic", "optimistic"]
    } else if label == "negative" {
        vec!["cautious", "sharp"]
    } else {
        vec!["balanced"]
    };

    serde_json::json!({
        "label": label,
        "score": score,
        "confidence": (score.abs() + 0.35).min(0.95),
        "tags": tags
    })
}
