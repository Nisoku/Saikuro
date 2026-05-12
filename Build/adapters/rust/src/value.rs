//! Value type alias and helpers.
//!
//! The Saikuro wire format uses MessagePack. Internally this adapter works
//! with `serde_json::Value` for ergonomic Rust use, converting to/from
//! the `saikuro_core::value::Value` at the transport boundary.

/// The value type used throughout the Saikuro Rust adapter.
///
/// Arguments and return values are represented as JSON-compatible values.
/// This matches the ergonomics of the other adapters (TypeScript/Python/C#)
/// and gives you `.as_i64()`, `.as_str()`, `json!()`, etc. for free.
pub type Value = serde_json::Value;

/// Convert a `saikuro_core::value::Value` into a [`Value`] (JSON).
pub fn core_to_json(v: saikuro_core::value::Value) -> Value {
    match serde_json::to_value(&v) {
        Ok(j) => j,
        Err(e) => {
            tracing::warn!(error = %e, "core_to_json serialization failed");
            Value::Null
        }
    }
}

/// Convert a [`Value`] (JSON) into `saikuro_core::value::Value`.
pub fn json_to_core(v: Value) -> saikuro_core::value::Value {
    match serde_json::from_value(v) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "json_to_core deserialization failed");
            saikuro_core::value::Value::Null
        }
    }
}
