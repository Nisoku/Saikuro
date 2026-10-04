//! WASI preview 2 math example (`wasm32-wasip2`).
//!
//! Run with:
//!   cargo run -p math-wasi-preview2 --target wasm32-wasip2
//!   cargo run -p math-wasi-preview2 --target wasm32-wasip2 -- --transport tcp
//!   cargo run -p math-wasi-preview2 --target wasm32-wasip2 -- \
//!       --transport ws --addr 127.0.0.1:9000
//!
//! Supported transports: `memory` (in-process), `tcp`, and `ws`, each with
//! both a provider and a client role. Unix sockets are not available on WASI.

use std::time::Duration;

use math_core::{run_client, run_in_memory, split_mode, Mode, Options, TransportChoice};
use saikuro::transport::{
    from_halves, LocalTransport, LocalTransportListener, TransportReceiver, TransportSender,
    WasiTcpListener, WasiWsListener,
};
use saikuro::{Client, Error, Result};

fn main() -> Result<()> {
    math_core::install_wasi_runtime_support();
    saikuro_exec::block_on(async_main())
}

async fn async_main() -> Result<()> {
    let (mode, args) = split_mode(std::env::args().skip(1))?;
    let options = Options::parse(args.into_iter())?;

    match options.transport {
        TransportChoice::Memory => {
            if mode != Mode::Both {
                return Err(Error::ProviderError(
                    "--serve-only/--client-only need a socket transport".into(),
                ));
            }
            run_in_memory().await
        }
        TransportChoice::Tcp => run_tcp(&options, mode).await,
        TransportChoice::WebSocket => run_ws(&options, mode).await,
        TransportChoice::Unix => Err(Error::ProviderError(
            "wasm32-wasip2 has no Unix socket support".into(),
        )),
    }
}

/// Bind a TCP listener, resolve its port, and wire the provider and client.
async fn run_tcp(options: &Options, mode: Mode) -> Result<()> {
    if mode == Mode::ClientOnly {
        return run_client(options).await;
    }

    let listener = WasiTcpListener::new(options.addr.clone())?;
    let port = listener.local_port()?;
    let url = format!("tcp://127.0.0.1:{port}");
    let serve_suffix = if mode == Mode::ServeOnly {
        ", serve-only"
    } else {
        ""
    };
    math_core::demo_log!("transport: tcp (provider listening on 127.0.0.1:{port}{serve_suffix})");
    serve(listener, mode, url).await
}

/// Bind a WebSocket listener, resolve its port, and wire the provider and
/// client. The upgrade handshake runs inside `accept`.
async fn run_ws(options: &Options, mode: Mode) -> Result<()> {
    if mode == Mode::ClientOnly {
        return run_client(options).await;
    }

    let listener = WasiWsListener::new(options.addr.clone())?;
    let port = listener.local_port()?;
    let url = format!("ws://127.0.0.1:{port}");
    let serve_suffix = if mode == Mode::ServeOnly {
        ", serve-only"
    } else {
        ""
    };
    math_core::demo_log!("transport: ws (provider listening on 127.0.0.1:{port}{serve_suffix})");
    serve(listener, mode, url).await
}

/// Serve the provider on `listener`
async fn serve<L>(mut listener: L, mode: Mode, client_url: String) -> Result<()>
where
    L: LocalTransportListener,
    L::Output: LocalTransport,
    <L::Output as LocalTransport>::Sender: TransportSender + 'static,
    <L::Output as LocalTransport>::Receiver: TransportReceiver + 'static,
{
    if mode == Mode::ServeOnly {
        while let Some(transport) = listener.accept().await? {
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
