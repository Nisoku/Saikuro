#![cfg(all(feature = "native", feature = "quic", not(target_arch = "wasm32")))]

use bytes::Bytes;
use saikuro_transport::shared::framing::{read_frame, write_frame};
use saikuro_transport::{connect, connect_with_options, ConnectOptions, TransportError};
use std::{net::SocketAddr, time::Duration};

// Synthetic loopback-only certificate and key; never use these outside tests.
const CERT: &[u8] = include_bytes!("fixtures/quic-cert.pem");
const KEY: &[u8] = include_bytes!("fixtures/quic-key.pem");

fn roundtrip(addr: SocketAddr) {
    saikuro_exec::block_on(async {
        saikuro_exec::timeout(Duration::from_secs(10), async {
            let mut listener = s2n_quic::Server::builder()
                .with_tls((
                    std::str::from_utf8(CERT).unwrap(),
                    std::str::from_utf8(KEY).unwrap(),
                ))
                .unwrap()
                .with_io(addr)
                .unwrap()
                .start()
                .unwrap();
            let address = format!("quic://{}", listener.local_addr().unwrap());
            let server = async {
                let mut connection = listener.accept().await.unwrap();
                let stream = connection
                    .accept_bidirectional_stream()
                    .await
                    .unwrap()
                    .unwrap();
                let (mut receiver, mut sender) = stream.split();
                let frame = read_frame(&mut receiver, 1024).await.unwrap().unwrap();
                write_frame(&mut sender, &frame).await.unwrap();
                // Keep the streams alive until the client receives the echo.
                (sender, receiver)
            };
            let client = async {
                let mut adapter = connect_with_options(
                    &address,
                    ConnectOptions {
                        quic_server_cert_pem: Some(CERT),
                    },
                )
                .await
                .unwrap();
                adapter
                    .send(Bytes::from_static(b"hello QUIC"))
                    .await
                    .unwrap();
                assert_eq!(
                    adapter.recv().await.unwrap(),
                    Some(Bytes::from_static(b"hello QUIC"))
                );
                adapter
            };
            futures::join!(server, client);
        })
        .await
        .expect("QUIC roundtrip timed out");
    });
}

#[test]
fn adapter_custom_trust_ipv4() {
    roundtrip("127.0.0.1:0".parse().unwrap());
}

#[test]
fn adapter_custom_trust_ipv6() {
    roundtrip("[::1]:0".parse().unwrap());
}

#[test]
fn adapter_rejects_invalid_custom_trust() {
    saikuro_exec::block_on(async {
        let result = connect_with_options(
            "quic://127.0.0.1:443",
            ConnectOptions {
                quic_server_cert_pem: Some(b"invalid certificate"),
            },
        )
        .await;
        assert!(matches!(result, Err(TransportError::Io(_))));
    });
}

#[test]
fn adapter_default_trust_reports_provider_error_without_panicking() {
    // The configured rustls provider has no default root store. Preserve that
    // policy and return its configuration error instead of bypassing verification.
    saikuro_exec::block_on(async {
        let result = connect("quic://127.0.0.1:443").await;
        assert!(matches!(result, Err(TransportError::Io(_))));
    });
}
