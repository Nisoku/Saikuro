use saikuro_exec::ChannelCapacity;
use saikuro_runtime::RuntimeConfig;

#[test]
fn runtime_config_rejects_channel_capacity_below_minimum() {
    let error = serde_json::from_str::<RuntimeConfig>(r#"{"stream_buffer_capacity":0}"#)
        .expect_err("zero capacity must be rejected");

    assert!(error.to_string().contains("outside the range 1..=256"));
}

#[test]
fn runtime_config_rejects_channel_capacity_above_maximum() {
    let error = serde_json::from_str::<RuntimeConfig>(r#"{"stream_buffer_capacity":257}"#)
        .expect_err("capacity above 256 must be rejected");

    assert!(error.to_string().contains("outside the range 1..=256"));
}

#[test]
fn runtime_config_preserves_valid_channel_capacity() {
    let config = serde_json::from_str::<RuntimeConfig>(r#"{"stream_buffer_capacity":64}"#)
        .expect("valid capacity must deserialize");

    assert_eq!(
        config.stream_buffer_capacity,
        ChannelCapacity::try_from(64).expect("64 is a valid channel capacity")
    );
    assert_eq!(
        config.router_config().stream_channel_capacity,
        config.stream_buffer_capacity
    );
    assert_eq!(
        config.router_config().channel_capacity,
        config.stream_buffer_capacity
    );
}
