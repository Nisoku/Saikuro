//! `RuntimeConfig` channel-capacity validation through serde_json.

use crate::check_test;
use crate::shared_test;
use crate::String;
use crate::TestSuite;
use crate::ToString;
use saikuro_exec::ChannelCapacity;
use saikuro_runtime::RuntimeConfig;

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "runtime::config_rejects_capacity_below_minimum",
        config_rejects_capacity_below_minimum,
    );
    shared_test!(
        suite,
        "runtime::config_rejects_capacity_above_maximum",
        config_rejects_capacity_above_maximum,
    );
    shared_test!(
        suite,
        "runtime::config_preserves_valid_capacity",
        config_preserves_valid_capacity,
    );
}

fn capacity_error_message(raw: &str) -> String {
    serde_json::from_str::<RuntimeConfig>(raw)
        .map(|_| "<no error>".into())
        .unwrap_or_else(|e| e.to_string())
}

fn config_rejects_capacity_below_minimum() -> Result<(), &'static str> {
    let error = capacity_error_message(r#"{"stream_buffer_capacity":0}"#);
    check_test!(
        error.contains("outside the range 1..=256"),
        "zero capacity must be rejected"
    );
    Ok(())
}

fn config_rejects_capacity_above_maximum() -> Result<(), &'static str> {
    let error = capacity_error_message(r#"{"stream_buffer_capacity":257}"#);
    check_test!(
        error.contains("outside the range 1..=256"),
        "capacity above 256 must be rejected"
    );
    Ok(())
}

fn config_preserves_valid_capacity() -> Result<(), &'static str> {
    let config = serde_json::from_str::<RuntimeConfig>(r#"{"stream_buffer_capacity":64}"#)
        .map_err(|_| "valid capacity must deserialize")?;
    assert_eq!(
        config.stream_buffer_capacity,
        ChannelCapacity::try_from(64).map_err(|_| "64 is valid")?
    );
    assert_eq!(
        config.router_config().stream_channel_capacity,
        config.stream_buffer_capacity
    );
    assert_eq!(
        config.router_config().channel_capacity,
        config.stream_buffer_capacity
    );
    Ok(())
}
