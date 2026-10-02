//! Math example: one provider and one client over a selectable transport.
//!
//! Run with:
//!   cargo run -p math                                  # in-memory (default)
//!   cargo run -p math -- --transport tcp               # loopback TCP
//!   cargo run -p math -- --transport tcp --addr 127.0.0.1:9000
//!
//! The TCP mode hosts a provider and dials it in the same process. Pass
//! `--serve-only` to listen without a local client instead
//!   cargo run -p math -- --transport tcp --addr 127.0.0.1:9000 --serve-only

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use math_core::{run_in_memory, Options, TransportChoice};
use saikuro::event::{LogSink, NullSink};
use saikuro::transport::{TcpTransportListener, Transport, TransportListener};
use saikuro::{from_halves, Client, Result};

fn main() -> Result<()> {
    saikuro_exec::block_on(async_main())
}

async fn async_main() -> Result<()> {
    let (serve_only, args) = split_serve_only(std::env::args().skip(1));
    let options = Options::parse(args.into_iter())?;
    match (options.transport, serve_only) {
        (TransportChoice::Memory, true) => Err(saikuro::Error::ProviderError(
            "--serve-only needs --transport tcp".into(),
        )),
        (TransportChoice::Memory, false) => run_in_memory().await,
        (TransportChoice::Tcp, true) => serve_tcp(options.addr).await,
        (TransportChoice::Tcp, false) => run_tcp(options.addr).await,
    }
}

fn split_serve_only<I: Iterator<Item = String>>(args: I) -> (bool, Vec<String>) {
    let mut serve_only = false;
    let rest = args
        .filter(|a| {
            if a == "--serve-only" {
                serve_only = true;
                false
            } else {
                true
            }
        })
        .collect();
    (serve_only, rest)
}

/// Listen on `addr` and serve the provider until the peer hangs up.
async fn serve_tcp(addr: SocketAddr) -> Result<()> {
    let sink: Box<dyn LogSink> = Box::new(NullSink);
    let mut listener = TcpTransportListener::bind(addr, Arc::from(sink)).await?;
    let bound = listener.local_addr();
    println!("transport: tcp (provider listening on {bound}, serve-only)");

    while let Some(transport) = listener.accept().await? {
        // `connect` only dials, so the accepted side is wrapped here.
        let (sender, receiver) = transport.split();
        // One bad connection must not take the listener down; report and keep
        // accepting.
        if let Err(e) = math_core::math_provider()
            .serve_on(from_halves(sender, receiver))
            .await
        {
            println!("provider: connection failed: {e}");
        }
    }
    Ok(())
}

/// Bind a loopback TCP listener, serve the provider on the accepted connection,
/// and have the client dial it.
async fn run_tcp(addr: SocketAddr) -> Result<()> {
    let sink: Box<dyn LogSink> = Box::new(NullSink);
    let mut listener = TcpTransportListener::bind(addr, Arc::from(sink)).await?;
    let bound = listener.local_addr();
    println!("transport: tcp (provider listening on {bound})");

    // Accept first so the listener is bound and the client cannot race it.
    let accepting = saikuro_exec::spawn(async move {
        match listener.accept().await {
            Ok(Some(transport)) => {
                // `connect` only dials, so the accepted side is wrapped here.
                let (sender, receiver) = transport.split();
                let _ = math_core::math_provider()
                    .serve_on(from_halves(sender, receiver))
                    .await;
            }
            Ok(None) => {}
            Err(e) => println!("provider: accept failed: {e}"),
        }
    });

    let client = Client::connect(format!("tcp://{bound}")).await?;
    let result = math_core::run_demo(client).await;

    // Let the provider notice the hang-up rather than leaving the task behind.
    let _ = saikuro_exec::timeout(Duration::from_millis(200), accepting).await;

    result
}
