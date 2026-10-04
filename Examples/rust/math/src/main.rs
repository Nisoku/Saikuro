//! Math example: one provider and one client over a selectable transport.
//!
//! Run with:
//!   cargo run -p math                                  # in-memory (default)
//!   cargo run -p math -- --transport tcp               # loopback TCP
//!   cargo run -p math -- --transport tcp --addr 127.0.0.1:9000
//!   cargo run -p math -- --transport unix              # loopback Unix socket
//!   cargo run -p math -- --transport unix --addr /tmp/math.sock
//!   cargo run -p math -- --transport ws                # loopback WebSocket
//!   cargo run -p math -- --transport ws --addr 127.0.0.1:9000
//!
//! The socket modes host a provider and dial it in the same process. Use
//! `--serve-only` to listen without a local client, or `--client-only` to dial
//! an existing provider without listening:
//!   cargo run -p math -- --transport tcp --addr 127.0.0.1:9000 --serve-only
//!   cargo run -p math -- --transport tcp --addr 127.0.0.1:9000 --client-only

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use math_core::{run_client, run_in_memory, split_mode, Mode, Options, TransportChoice};
use saikuro::event::{LogSink, NullSink};
use saikuro::transport::unix::UnixTransportListener;
use saikuro::transport::{TcpTransportListener, Transport, TransportListener, WsTransportListener};
use saikuro::{from_halves, Client, Error, Result};

fn main() -> Result<()> {
    saikuro_exec::block_on(async_main())
}

async fn async_main() -> Result<()> {
    let (mode, args) = split_mode(std::env::args().skip(1))?;
    let options = Options::parse(args.into_iter())?;

    if options.transport == TransportChoice::Memory {
        if mode != Mode::Both {
            return Err(Error::ProviderError(
                "--serve-only/--client-only need a socket transport".into(),
            ));
        }
        return run_in_memory().await;
    }

    if mode == Mode::ClientOnly {
        return run_client(&options).await;
    }

    let log = null_log();
    let serve_suffix = if mode == Mode::ServeOnly {
        ", serve-only"
    } else {
        ""
    };

    match options.transport {
        TransportChoice::Memory => unreachable!("memory handled above"),
        TransportChoice::Tcp => {
            let addr = parse_socket_addr(&options.addr, "tcp")?;
            let listener = TcpTransportListener::bind(addr, log).await?;
            let bound = listener.local_addr();
            math_core::demo_log!("transport: tcp (provider listening on {bound}{serve_suffix})");
            serve(listener, mode, format!("tcp://{bound}")).await
        }
        TransportChoice::Unix => {
            let listener = UnixTransportListener::bind(&options.addr, log).await?;
            let path = listener.path().display().to_string();
            math_core::demo_log!("transport: unix (provider listening on {path}{serve_suffix})");
            serve(listener, mode, format!("unix://{path}")).await
        }
        TransportChoice::WebSocket => {
            let addr = parse_socket_addr(&options.addr, "ws")?;
            let listener = WsTransportListener::bind(addr, log).await?;
            let bound = listener.local_addr();
            math_core::demo_log!("transport: ws (provider listening on {bound}{serve_suffix})");
            serve(listener, mode, format!("ws://{bound}")).await
        }
    }
}

/// A logging sink that discards every record.
fn null_log() -> Arc<dyn LogSink> {
    Arc::from(Box::new(NullSink) as Box<dyn LogSink>)
}

/// Parse a `HOST:PORT` argument, naming the transport in any error.
fn parse_socket_addr(value: &str, transport: &str) -> Result<SocketAddr> {
    value.parse().map_err(|e: std::net::AddrParseError| {
        Error::ProviderError(format!("invalid --addr '{value}' for {transport}: {e}"))
    })
}

/// Serve the provider on `listener`.
async fn serve<L>(mut listener: L, mode: Mode, client_url: String) -> Result<()>
where
    L: TransportListener,
    L::Output: Transport,
{
    if mode == Mode::ServeOnly {
        while let Some(transport) = listener.accept().await? {
            // `connect` only dials, so the accepted side is wrapped here.
            let (sender, receiver) = transport.split();
            // One bad connection must not take the listener down; report and
            // keep accepting.
            if let Err(e) = math_core::math_provider()
                .serve_on(from_halves(sender, receiver))
                .await
            {
                math_core::demo_log!("provider: connection failed: {e}");
            }
        }
        return Ok(());
    }

    // Accept first so the listener is bound and the client cannot race it.
    let accepting = saikuro_exec::spawn(async move {
        match listener.accept().await {
            Ok(Some(transport)) => {
                let (sender, receiver) = transport.split();
                let _ = math_core::math_provider()
                    .serve_on(from_halves(sender, receiver))
                    .await;
            }
            Ok(None) => {}
            Err(e) => math_core::demo_log!("provider: accept failed: {e}"),
        }
    });

    let client = Client::connect(&client_url).await?;
    let result = math_core::run_demo(client).await;

    // Let the provider notice the hang-up rather than leaving the task behind.
    let _ = saikuro_exec::timeout(Duration::from_millis(200), accepting).await;

    result
}
