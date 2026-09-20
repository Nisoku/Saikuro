//! `RuntimeConfig` serde round-trips, defaults, and engine mapping.

use core::time::Duration;

use crate::check_test;
use crate::format;
use crate::shared_test;
use crate::TestSuite;
use saikuro_exec::ChannelCapacity;
use saikuro_runtime::RuntimeConfig;
use saikuro_transport::selector::{TransportConfig, TransportKind};

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "runtime::config_defaults_match_struct_default",
        config_defaults_match_struct_default,
    );
    shared_test!(
        suite,
        "runtime::config_full_roundtrip_preserves_fields",
        config_full_roundtrip_preserves_fields,
    );
    shared_test!(
        suite,
        "runtime::config_mode_parses_lowercase",
        config_mode_parses_lowercase,
    );
    shared_test!(
        suite,
        "runtime::config_call_timeout_serializes_millis",
        config_call_timeout_serializes_millis,
    );
    shared_test!(
        suite,
        "runtime::config_router_config_propagates_call_timeout",
        config_router_config_propagates_call_timeout,
    );
    shared_test!(
        suite,
        "runtime::config_transport_config_roundtrip",
        config_transport_config_roundtrip,
    );
    shared_test!(
        suite,
        "runtime::config_omits_absent_transport",
        config_omits_absent_transport,
    );
}

fn config_defaults_match_struct_default() -> Result<(), &'static str> {
    let from_json =
        serde_json::from_str::<RuntimeConfig>("{}").map_err(|_| "empty config must parse")?;
    check_test!(
        format!("{:?}", from_json.mode) == "Development",
        "mode must default to Development"
    );
    check_test!(
        from_json.max_in_flight_calls == 1024,
        "max_in_flight_calls must default to 1024"
    );
    check_test!(
        from_json.call_timeout == core::time::Duration::from_secs(30),
        "call_timeout must default to 30s"
    );
    check_test!(
        from_json.transport.is_none(),
        "transport must default to None"
    );
    check_test!(
        from_json.max_message_size == 16 * 1024 * 1024,
        "max_message_size must default to 16 MiB"
    );
    check_test!(
        from_json.stream_buffer_capacity == ChannelCapacity::DEFAULT,
        "stream_buffer_capacity must default to DEFAULT"
    );
    check_test!(!from_json.json_logs, "json_logs must default to false");
    Ok(())
}

fn rich_config() -> RuntimeConfig {
    RuntimeConfig {
        max_in_flight_calls: 7,
        call_timeout: core::time::Duration::from_millis(1234),
        transport: Some(TransportConfig {
            kind: TransportKind::WebSocket,
            address: Some("ws://example.dev/sock".into()),
            max_message_size: 4096,
            send_buffer: 8,
        }),
        max_message_size: 65536,
        stream_buffer_capacity: ChannelCapacity::try_from(64)
            .expect("64 is a valid channel capacity"),
        json_logs: true,
        ..RuntimeConfig::default()
    }
}

fn config_full_roundtrip_preserves_fields() -> Result<(), &'static str> {
    let original = rich_config();
    let json = serde_json::to_string(&original).map_err(|_| "serialize")?;
    let parsed = serde_json::from_str::<RuntimeConfig>(&json).map_err(|_| "deserialize")?;

    check_test!(
        format!("{:?}", parsed.mode) == "Development",
        "mode must survive the roundtrip"
    );
    check_test!(
        parsed.max_in_flight_calls == original.max_in_flight_calls,
        "max_in_flight_calls must roundtrip"
    );
    check_test!(
        parsed.call_timeout == original.call_timeout,
        "call_timeout must roundtrip"
    );
    check_test!(
        parsed.max_message_size == original.max_message_size,
        "max_message_size must roundtrip"
    );
    check_test!(
        parsed.stream_buffer_capacity == original.stream_buffer_capacity,
        "stream_buffer_capacity must roundtrip"
    );
    check_test!(
        parsed.json_logs == original.json_logs,
        "json_logs must roundtrip"
    );

    let transport = parsed.transport.as_ref().ok_or("transport must survive")?;
    check_test!(
        transport.kind == TransportKind::WebSocket,
        "kind must roundtrip"
    );
    check_test!(
        transport.address.as_deref() == Some("ws://example.dev/sock"),
        "address must roundtrip"
    );
    check_test!(
        transport.max_message_size == 4096,
        "transport max_message_size must roundtrip"
    );
    check_test!(
        transport.send_buffer == 8,
        "transport send_buffer must roundtrip"
    );
    Ok(())
}

fn config_mode_parses_lowercase() -> Result<(), &'static str> {
    let dev = serde_json::from_str::<RuntimeConfig>(r#"{"mode":"development"}"#)
        .map_err(|_| "dev mode must parse")?;
    check_test!(
        format!("{:?}", dev.mode) == "Development",
        "development must parse"
    );
    let prod = serde_json::from_str::<RuntimeConfig>(r#"{"mode":"production"}"#)
        .map_err(|_| "prod mode must parse")?;
    check_test!(
        format!("{:?}", prod.mode) == "Production",
        "production must parse"
    );
    Ok(())
}

fn config_call_timeout_serializes_millis() -> Result<(), &'static str> {
    let config = RuntimeConfig {
        call_timeout: Duration::from_millis(1234),
        ..RuntimeConfig::default()
    };
    let json = serde_json::to_string(&config).map_err(|_| "serialize")?;
    check_test!(
        json.contains("\"call_timeout\":1234"),
        "call_timeout must serialize as integer milliseconds: {json}"
    );
    Ok(())
}

fn config_router_config_propagates_call_timeout() -> Result<(), &'static str> {
    let config = RuntimeConfig {
        call_timeout: Duration::from_millis(4321),
        ..RuntimeConfig::default()
    };
    check_test!(
        config.router_config().call_timeout == Duration::from_millis(4321),
        "router call timeout must mirror runtime call timeout"
    );
    Ok(())
}

fn config_transport_config_roundtrip() -> Result<(), &'static str> {
    let transport = TransportConfig {
        kind: TransportKind::Memory,
        address: None,
        max_message_size: 1024,
        send_buffer: 16,
    };
    let json = serde_json::to_string(&transport).map_err(|_| "serialize")?;
    let parsed = serde_json::from_str::<TransportConfig>(&json).map_err(|_| "deserialize")?;
    check_test!(
        parsed.kind == TransportKind::Memory,
        "kind must roundtrip through snake_case"
    );
    check_test!(
        parsed.max_message_size == 1024,
        "max_message_size must roundtrip"
    );
    check_test!(parsed.send_buffer == 16, "send_buffer must roundtrip");
    Ok(())
}

fn config_omits_absent_transport() -> Result<(), &'static str> {
    let config = RuntimeConfig::default();
    let json = serde_json::to_string(&config).map_err(|_| "serialize")?;
    check_test!(
        !json.contains("\"transport\""),
        "absent transport must be skipped on the wire: {json}"
    );
    Ok(())
}
