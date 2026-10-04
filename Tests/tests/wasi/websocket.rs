//! WASI WebSocket transport: server upgrade plus a loopback round-trip.

use alloc::format;

use crate::TestSuite;
use bytes::Bytes;
use core::future::Future;
use core::pin::Pin;
use saikuro_exec::spawn;
use saikuro_tests::shared_test_async;
use saikuro_transport::{
    LocalTransport, LocalTransportListener, TransportReceiver, TransportSender, WasiWsListener,
    WebSocketTransport,
};

const PAYLOAD: &[u8] = &[0xA5; 10_000];

pub fn register(suite: &mut TestSuite) {
    shared_test_async!(
        suite,
        "wasi::websocket_server_round_trip",
        websocket_server_round_trip,
    );
}

/// A WASI provider upgrading an inbound socket and echoing one frame, with the
/// in-guest client dialing it over loopback.
fn websocket_server_round_trip() -> Pin<Box<dyn Future<Output = Result<(), &'static str>>>> {
    Box::pin(async {
        let mut listener = WasiWsListener::new("127.0.0.1:0").map_err(|_| "bind ws listener")?;
        let port = listener.local_port().map_err(|_| "resolve ws port")?;
        let url = format!("ws://127.0.0.1:{port}");

        // Accept on its own task (accept blocks)
        let serving = spawn(async move {
            let transport = match listener.accept().await {
                Ok(Some(transport)) => transport,
                Ok(None) => return Err("ws listener closed"),
                Err(_) => return Err("ws accept"),
            };
            let (mut sender, mut receiver) = transport.split();
            let received = match receiver.recv().await {
                Ok(Some(frame)) => frame,
                Ok(None) => return Err("provider saw EOF before a frame"),
                Err(_) => return Err("provider recv"),
            };
            sender.send(received).await.map_err(|_| "provider send")?;
            sender.close().await.map_err(|_| "provider close")
        });

        let client = WebSocketTransport::connect(url)
            .await
            .map_err(|_| "ws client connect")?;
        let (mut sender, mut receiver) = client.split();

        sender
            .send(Bytes::from_static(PAYLOAD))
            .await
            .map_err(|_| "client send")?;

        let echoed = match receiver.recv().await {
            Ok(Some(frame)) => frame,
            Ok(None) => return Err("client saw EOF before the echo"),
            Err(_) => return Err("client recv"),
        };
        if echoed.as_ref() != PAYLOAD {
            return Err("echoed payload differs from the sent payload");
        }

        // The provider closes after echoing, so the client must observe EOF
        // rather than hanging.
        match receiver.recv().await {
            Ok(None) => {}
            Ok(Some(_)) => return Err("client received a frame after close"),
            Err(_) => return Err("client recv after close"),
        }

        match serving.await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err("provider task did not complete"),
        }
        Ok(())
    })
}
