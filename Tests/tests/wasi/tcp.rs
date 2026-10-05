use alloc::format;

use crate::TestSuite;
use bytes::Bytes;
use core::future::Future;
use core::pin::Pin;
use saikuro_exec::spawn;
use saikuro_tests::shared_test_async;
use saikuro_transport::{
    LocalTransport, LocalTransportConnector, LocalTransportListener, TransportReceiver,
    TransportSender, WasiTcpConnector, WasiTcpListener,
};

const PAYLOAD: &[u8] = &[0x5A; 64];

pub fn register(suite: &mut TestSuite) {
    shared_test_async!(
        suite,
        "wasi::tcp_close_half_closes_connection",
        tcp_close_half_closes_connection,
    );
}

/// A provider reading one frame then half-closing
fn tcp_close_half_closes_connection() -> Pin<Box<dyn Future<Output = Result<(), &'static str>>>> {
    Box::pin(async {
        let mut listener = WasiTcpListener::new("127.0.0.1:0").map_err(|_| "bind tcp listener")?;
        let port = listener.local_port().map_err(|_| "resolve tcp port")?;
        let addr = format!("127.0.0.1:{port}");

        // Accept on its own task (accept blocks)
        let serving = spawn(async move {
            let transport = match listener.accept().await {
                Ok(Some(transport)) => transport,
                Ok(None) => return Err("tcp listener closed"),
                Err(_) => return Err("tcp accept"),
            };
            let (mut sender, mut receiver) = transport.split();
            let received = match receiver.recv().await {
                Ok(Some(frame)) => frame,
                Ok(None) => return Err("provider saw EOF before a frame"),
                Err(_) => return Err("provider recv"),
            };
            if received.as_ref() != PAYLOAD {
                return Err("provider received a different payload");
            }

            sender.close().await.map_err(|_| "provider close")?;

            // Half-close must leave the receive half readable
            match receiver.recv().await {
                Ok(None) => Ok(()),
                Ok(Some(_)) => Err("provider received a frame after its own close"),
                Err(_) => Err("provider recv after close"),
            }
        });

        let client = WasiTcpConnector::new(addr)
            .connect()
            .await
            .map_err(|_| "tcp client connect")?;
        let (mut sender, mut receiver) = client.split();

        sender
            .send(Bytes::from_static(PAYLOAD))
            .await
            .map_err(|_| "client send")?;
        sender.close().await.map_err(|_| "client close")?;

        // The client half-closed, so the provider's FIN must arrive as EOF.
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
