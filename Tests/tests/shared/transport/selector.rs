//! `TransportSelector` pick logic and `TransportConfig` serialization.

use crate::check_test;
use crate::shared_test;
use crate::TestSuite;
use saikuro_transport::selector::{TransportConfig, TransportKind, TransportSelector};

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "transport::selector_no_address_returns_memory",
        selector_no_address_returns_memory,
    );
    shared_test!(
        suite,
        "transport::selector_memory_address_returns_memory",
        selector_memory_address_returns_memory,
    );
    shared_test!(
        suite,
        "transport::selector_override_config_wins",
        selector_override_config_wins,
    );
    shared_test!(
        suite,
        "transport::selector_websocket_address",
        selector_websocket_address,
    );
    shared_test!(
        suite,
        "transport::selector_unix_path_prefers_unix",
        selector_unix_path_prefers_unix,
    );
    shared_test!(
        suite,
        "transport::selector_tcp_fallback",
        selector_tcp_fallback,
    );
    shared_test!(
        suite,
        "transport::selector_wasm_host_route",
        selector_wasm_host_route,
    );
    shared_test!(
        suite,
        "transport::transport_config_serde_roundtrip",
        transport_config_serde_roundtrip,
    );
    shared_test!(
        suite,
        "transport::transport_kind_deserializes_snake_case",
        transport_kind_deserializes_snake_case,
    );
}

fn selector_no_address_returns_memory() -> Result<(), &'static str> {
    let (kind, address) = TransportSelector::select(None, None);
    check_test!(kind == TransportKind::Memory, "no address must pick Memory");
    check_test!(address.is_none(), "no address must resolve to None");
    Ok(())
}

fn selector_memory_address_returns_memory() -> Result<(), &'static str> {
    let (kind, _address) = TransportSelector::select(Some("memory"), None);
    check_test!(
        kind == TransportKind::Memory,
        "the literal `memory` address must pick Memory"
    );
    let (kind, address) = TransportSelector::select(Some(""), None);
    check_test!(
        kind == TransportKind::Memory,
        "empty address must pick Memory"
    );
    check_test!(address.is_none(), "empty address must resolve to None");
    Ok(())
}

fn selector_override_config_wins() -> Result<(), &'static str> {
    let override_config = TransportConfig {
        kind: TransportKind::Memory,
        address: Some("ignored".into()),
        max_message_size: 4096,
        send_buffer: 8,
    };
    let (kind, address) = TransportSelector::select(Some("/tmp/socket"), Some(&override_config));
    check_test!(
        kind == TransportKind::Memory,
        "explicit override must beat address heuristics"
    );
    check_test!(
        address.as_deref() == Some("ignored"),
        "override address must win"
    );
    Ok(())
}

fn selector_websocket_address() -> Result<(), &'static str> {
    let (kind, address) = TransportSelector::select(Some("ws://relay.dev/sock"), None);
    check_test!(
        kind == TransportKind::WebSocket,
        "ws:// address must pick WebSocket"
    );
    check_test!(
        address.as_deref() == Some("ws://relay.dev/sock"),
        "ws URL must be preserved verbatim"
    );
    Ok(())
}

fn selector_unix_path_prefers_unix() -> Result<(), &'static str> {
    #[cfg(all(target_family = "unix", not(target_arch = "wasm32")))]
    {
        let (kind, address) = TransportSelector::select(Some("/tmp/foo.sock"), None);
        check_test!(
            kind == TransportKind::Unix,
            "a path-like address must prefer Unix"
        );
        check_test!(
            address.as_deref() == Some("/tmp/foo.sock"),
            "unix address must be preserved"
        );
        let (kind, _) = TransportSelector::select(Some("./relative.sock"), None);
        check_test!(
            kind == TransportKind::Unix,
            "a relative dot path must prefer Unix"
        );
        let (kind, address) = TransportSelector::select(Some("unix:///tmp/foo.sock"), None);
        check_test!(
            kind == TransportKind::Unix,
            "unix:// scheme must prefer Unix"
        );
        check_test!(
            address.as_deref() == Some("/tmp/foo.sock"),
            "unix:// address must be stripped of its scheme"
        );
    }
    Ok(())
}

fn selector_tcp_fallback() -> Result<(), &'static str> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (kind, address) = TransportSelector::select(Some("host:8080"), None);
        check_test!(
            kind == TransportKind::Tcp,
            "a bare host:port must fall back to Tcp"
        );
        check_test!(
            address.as_deref() == Some("host:8080"),
            "tcp address must be preserved"
        );
    }
    Ok(())
}

fn selector_wasm_host_route() -> Result<(), &'static str> {
    #[cfg(target_arch = "wasm32")]
    {
        let (kind, address) = TransportSelector::select(Some("wasm-host://inst"), None);
        check_test!(
            kind == TransportKind::WasmHost,
            "wasm-host:// address must pick WasmHost"
        );
        check_test!(
            address.as_deref() == Some("wasm-host://inst"),
            "wasm-host address must be preserved"
        );
    }
    Ok(())
}

fn transport_config_serde_roundtrip() -> Result<(), &'static str> {
    let config = TransportConfig {
        kind: TransportKind::WebSocket,
        address: Some("wss://example.dev/x".into()),
        max_message_size: 2048,
        send_buffer: 32,
    };
    let json = serde_json::to_string(&config).map_err(|_| "serialize")?;
    let parsed = serde_json::from_str::<TransportConfig>(&json).map_err(|_| "deserialize")?;
    check_test!(
        parsed.kind == TransportKind::WebSocket,
        "kind must survive the serde roundtrip"
    );
    check_test!(
        parsed.address.as_deref() == Some("wss://example.dev/x"),
        "address must survive the serde roundtrip"
    );
    check_test!(
        parsed.max_message_size == 2048,
        "max_message_size must survive"
    );
    check_test!(parsed.send_buffer == 32, "send_buffer must survive");
    Ok(())
}

fn transport_kind_deserializes_snake_case() -> Result<(), &'static str> {
    let memory: TransportKind = serde_json::from_str("\"memory\"").map_err(|_| "memory kind")?;
    check_test!(
        memory == TransportKind::Memory,
        "snake_case memory must parse"
    );
    let websocket: TransportKind =
        serde_json::from_str("\"web_socket\"").map_err(|_| "websocket kind")?;
    check_test!(
        websocket == TransportKind::WebSocket,
        "snake_case web_socket must parse"
    );
    Ok(())
}
