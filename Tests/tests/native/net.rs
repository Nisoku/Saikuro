//! saikuro-net facade tests against the native tokio engine.

use saikuro_net::io::{duplex, split, AsyncReadExt, AsyncWriteExt};
use saikuro_net::net::{TcpListener, TcpStream};
use saikuro_tests::TestSuite;

pub fn register(suite: &mut TestSuite) {
    suite.register("net::tcp_echo_roundtrip", tcp_echo_roundtrip);
    suite.register(
        "net::tcp_connection_refused_returns_error",
        tcp_connection_refused_returns_error,
    );
    suite.register("net::duplex_split_echo", duplex_split_echo);
    suite.register("net::duplex_bidirectional_flow", duplex_bidirectional_flow);
}

fn tcp_echo_roundtrip() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|_| "bind loopback")?;
        let addr = listener.local_addr().map_err(|_| "local addr")?;

        let server = async {
            let (mut socket, _peer) = listener.accept().await.map_err(|_| "accept")?;
            let mut buf = [0u8; 64];
            loop {
                let n = socket.read(&mut buf).await.map_err(|_| "server read")?;
                if n == 0 {
                    return Ok::<(), &'static str>(());
                }
                socket
                    .write_all(&buf[..n])
                    .await
                    .map_err(|_| "server write")?;
            }
        };

        let client = async {
            let mut stream = TcpStream::connect(addr).await.map_err(|_| "connect")?;
            stream
                .write_all(b"hello saikuro")
                .await
                .map_err(|_| "client write")?;
            let mut buf = [0u8; 64];
            let n = stream.read(&mut buf).await.map_err(|_| "client read")?;
            saikuro_tests::check_test!(
                &buf[..n] == b"hello saikuro",
                "the echoed bytes must match what was sent"
            );
            Ok::<(), &'static str>(())
        };

        let (s, c) = futures::join!(server, client);
        s?;
        c?;
        Ok(())
    })
}

fn tcp_connection_refused_returns_error() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|_| "bind loopback")?;
        let addr = listener.local_addr().map_err(|_| "local addr")?;
        drop(listener);
        let attempt = TcpStream::connect(addr).await;
        saikuro_tests::check_test!(attempt.is_err(), "connecting to a closed port must fail");
        Ok(())
    })
}

fn duplex_split_echo() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let (a, b) = duplex(64);
        let (mut a_read, mut a_write) = split(a);
        let (mut b_read, mut b_write) = split(b);

        a_write.write_all(b"ping").await.map_err(|_| "a write")?;
        let mut got = [0u8; 8];
        let n = b_read.read(&mut got).await.map_err(|_| "b read")?;
        saikuro_tests::check_test!(
            &got[..n] == b"ping",
            "data written on one half must arrive on the other"
        );

        b_write.write_all(b"pong").await.map_err(|_| "b write")?;
        let m = a_read.read(&mut got).await.map_err(|_| "a read")?;
        saikuro_tests::check_test!(&got[..m] == b"pong", "the reverse direction must flow too");
        Ok(())
    })
}

fn duplex_bidirectional_flow() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let (mut a, mut b) = duplex(64);
        let a_tx = async { a.write_all(b"1234").await.map_err(|_| "a write") };
        let b_rx = async {
            let mut buf = [0u8; 4];
            let n = b.read(&mut buf).await.map_err(|_| "b read")?;
            saikuro_tests::check_test!(
                &buf[..n] == b"1234",
                "the writer's bytes must be readable on the peer"
            );
            Ok::<(), &'static str>(())
        };
        let (a, b) = futures::join!(a_tx, b_rx);
        a?;
        b?;
        Ok(())
    })
}
