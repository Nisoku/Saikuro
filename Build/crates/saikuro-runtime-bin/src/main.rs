//! Saikuro Runtime Server
//!
//! Standalone process that accepts connections from Saikuro adapters over TCP,
//! WebSocket, and Unix domain sockets. It acts as the central message broker:
//! adapters announce their capabilities and the runtime routes invocations
//! among them.
//!
//! # Usage
//!
//! ```text
//! saikuro-runtime [OPTIONS]
//!
//! Options:
//!   --schema <PATH>       Load a frozen schema JSON at startup
//!   --tcp-port <PORT>     Listen for TCP connections (default: 7700)
//!   --ws-port <PORT>      Listen for WebSocket connections (default: 7701)
//!   --unix <PATH>         Listen on a Unix domain socket
//!   --mode <MODE>         Runtime mode: development | production (default: development)
//!   --log-level <LEVEL>   Log level: error | warn | info | debug | trace (default: info)
//!   --json-logs           Emit logs as JSON (useful for log aggregation)
//!   --no-tcp              Disable TCP listener
//!   --no-ws               Disable WebSocket listener
//! ```

use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};

use anyhow::{Context, Result};
use clap::Parser;
use saikuro_core::capability::CapabilitySet;
use saikuro_runtime_lib::{config::RuntimeMode, RuntimeConfig, SaikuroRuntime};
use saikuro_transport::tcp::{TcpTransport, TcpTransportListener};
use saikuro_transport::traits::TransportListener;
use tokio::{signal, sync::watch};
use tracing::{error, info, warn};

// CLI

#[derive(Debug, Parser)]
#[command(
    name = "saikuro-runtime",
    about = "Saikuro runtime server: routes invocations between adapters",
    version
)]
struct Args {
    /// Path to a schema JSON file to load at startup.
    ///
    /// When provided the schema is merged into the registry before any
    /// adapter connects. Useful for production deployments where the schema
    /// is known ahead of time.
    #[arg(long, value_name = "PATH")]
    schema: Option<PathBuf>,

    /// Port to listen for raw TCP connections.
    ///
    /// Set to 0 to let the OS assign a port, or use --no-tcp to disable.
    #[arg(long, value_name = "PORT", default_value = "7700")]
    tcp_port: u16,

    /// Port to listen for WebSocket connections.
    ///
    /// Set to 0 to let the OS assign a port, or use --no-ws to disable.
    #[arg(long, value_name = "PORT", default_value = "7701")]
    ws_port: u16,

    /// Path to a Unix domain socket to create and listen on.
    ///
    /// Only available on Unix platforms. Ignored on Windows.
    #[arg(long, value_name = "PATH")]
    unix: Option<PathBuf>,

    /// Bind address for TCP and WebSocket listeners.
    ///
    /// Defaults to 127.0.0.1 (loopback). Use 0.0.0.0 to listen on all
    /// interfaces.
    #[arg(long, value_name = "ADDR", default_value = "127.0.0.1")]
    bind: IpAddr,

    /// Runtime mode.
    ///
    /// In production mode the schema registry is frozen: adapters cannot
    /// announce new functions after startup.
    #[arg(long, value_name = "MODE", default_value = "development")]
    mode: CliMode,

    /// Minimum log level to emit.
    #[arg(
        long,
        value_name = "LEVEL",
        default_value = "info",
        env = "SAIKURO_LOG"
    )]
    log_level: String,

    /// Emit logs as newline-delimited JSON instead of human-readable text.
    #[arg(long)]
    json_logs: bool,

    /// Disable the TCP listener.
    #[arg(long)]
    no_tcp: bool,

    /// Disable the WebSocket listener.
    #[arg(long)]
    no_ws: bool,
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum CliMode {
    Development,
    Production,
}

impl From<CliMode> for RuntimeMode {
    fn from(m: CliMode) -> Self {
        match m {
            CliMode::Development => RuntimeMode::Development,
            CliMode::Production => RuntimeMode::Production,
        }
    }
}

// Main

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    init_logging(&args.log_level, args.json_logs);

    info!(
        version = env!("CARGO_PKG_VERSION"),
        mode = ?args.mode,
        "saikuro-runtime starting"
    );

    // Build the runtime.
    let mode: RuntimeMode = args.mode.into();
    let config = RuntimeConfig {
        mode,
        json_logs: args.json_logs,
        ..Default::default()
    };
    let runtime = Arc::new(SaikuroRuntime::builder().config(config).build());
    let handle = runtime.handle();

    // Load schema from disk if requested.
    if let Some(schema_path) = &args.schema {
        let raw = std::fs::read_to_string(schema_path)
            .with_context(|| format!("reading schema file {}", schema_path.display()))?;
        let schema: saikuro_core::schema::Schema = serde_json::from_str(&raw)
            .with_context(|| format!("parsing schema file {}", schema_path.display()))?;
        handle
            .register_schema(schema, "static")
            .context("registering static schema")?;
        info!(path = %schema_path.display(), "loaded static schema");
    }

    // Set up graceful shutdown channel.
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Spawn transport listeners.
    let mut listener_tasks = Vec::new();

    // TCP listener.
    if !args.no_tcp {
        let addr = SocketAddr::new(args.bind, args.tcp_port);
        match TcpTransportListener::bind(addr).await {
            Ok(mut listener) => {
                info!(addr = %listener.local_addr(), "TCP listener ready");
                let h = handle.clone();
                let mut rx = shutdown_rx.clone();
                listener_tasks.push(tokio::spawn(async move {
                    run_tcp_listener(&mut listener, h, &mut rx).await;
                }));
            }
            Err(e) => {
                error!(addr = %addr, error = %e, "failed to bind TCP listener");
                return Err(anyhow::anyhow!("TCP bind failed: {e}"));
            }
        }
    }

    // WebSocket listener.
    #[cfg(feature = "ws-transport")]
    if !args.no_ws {
        let addr = SocketAddr::new(args.bind, args.ws_port);
        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => {
                let actual_addr = listener.local_addr()?;
                info!(addr = %actual_addr, "WebSocket listener ready");
                let h = handle.clone();
                let mut rx = shutdown_rx.clone();
                listener_tasks.push(tokio::spawn(async move {
                    run_ws_listener(listener, h, &mut rx).await;
                }));
            }
            Err(e) => {
                error!(addr = %addr, error = %e, "failed to bind WebSocket listener");
                return Err(anyhow::anyhow!("WebSocket bind failed: {e}"));
            }
        }
    }

    // Unix domain socket listener (Unix-only).
    #[cfg(all(feature = "native-transport", target_family = "unix"))]
    if let Some(unix_path) = &args.unix {
        use saikuro_transport::unix::UnixTransportListener;
        match UnixTransportListener::bind(unix_path) {
            Ok(mut listener) => {
                info!(path = %unix_path.display(), "Unix socket listener ready");
                let h = handle.clone();
                let mut rx = shutdown_rx.clone();
                listener_tasks.push(tokio::spawn(async move {
                    run_unix_listener(&mut listener, h, &mut rx).await;
                }));
            }
            Err(e) => {
                error!(path = %unix_path.display(), error = %e, "failed to bind Unix listener");
                return Err(anyhow::anyhow!("Unix socket bind failed: {e}"));
            }
        }
    }

    if listener_tasks.is_empty() {
        warn!("no listeners are active; all transports were disabled");
    }

    // Wait for Ctrl-C or SIGTERM.
    wait_for_shutdown_signal().await;
    info!("shutdown signal received; stopping listeners");

    // Broadcast shutdown to all listener loops.
    let _ = shutdown_tx.send(true);
    runtime.shutdown();

    // Give listeners a moment to exit cleanly.
    for task in listener_tasks {
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), task).await;
    }

    info!("saikuro-runtime stopped");
    Ok(())
}

// Transport accept loops

/// Accept loop for the raw TCP listener.
async fn run_tcp_listener(
    listener: &mut TcpTransportListener,
    handle: saikuro_runtime_lib::RuntimeHandle,
    shutdown: &mut watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok(Some(transport)) => {
                        let peer_id = peer_id_from_tcp(&transport);
                        info!(peer = %peer_id, "TCP connection accepted");
                        handle.accept_transport(transport, peer_id, CapabilitySet::default());
                    }
                    Ok(None) => {
                        info!("TCP listener closed");
                        break;
                    }
                    Err(e) => {
                        error!(error = %e, "TCP accept error");
                        // Brief back-off to avoid a tight error loop.
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }
                }
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    info!("TCP listener shutting down");
                    break;
                }
            }
        }
    }
}

/// Accept loop for the WebSocket listener.
///
/// We own a raw `tokio::net::TcpListener` here and perform the WebSocket
/// upgrade ourselves because `saikuro-transport` does not provide a
/// `WsTransportListener` type (the caller is expected to do the upgrade).
#[cfg(feature = "ws-transport")]
async fn run_ws_listener(
    listener: tokio::net::TcpListener,
    handle: saikuro_runtime_lib::RuntimeHandle,
    shutdown: &mut watch::Receiver<bool>,
) {
    use saikuro_transport::websocket::WebSocketTransport;

    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, peer_addr)) => {
                        let url = format!("ws://{peer_addr}");
                        info!(peer = %peer_addr, "WebSocket TCP stream accepted; upgrading");
                        let h = handle.clone();
                        tokio::spawn(async move {
                            // Wrap the plain TcpStream in MaybeTlsStream::Plain so
                            // the resulting WebSocketStream type matches what
                            // WebSocketTransport::from_stream expects.
                            use tokio_tungstenite::MaybeTlsStream;
                            let maybe_tls = MaybeTlsStream::Plain(stream);
                            match tokio_tungstenite::accept_async(maybe_tls).await {
                                Ok(ws_stream) => {
                                    let transport = WebSocketTransport::from_stream(
                                        ws_stream,
                                        url.clone(),
                                    );
                                    let peer_id = format!("ws-{peer_addr}");
                                    info!(peer = %peer_id, "WebSocket connection ready");
                                    h.accept_transport(transport, peer_id, CapabilitySet::default());
                                }
                                Err(e) => {
                                    warn!(peer = %peer_addr, error = %e, "WebSocket upgrade failed");
                                }
                            }
                        });
                    }
                    Err(e) => {
                        error!(error = %e, "WebSocket TCP accept error");
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }
                }
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    info!("WebSocket listener shutting down");
                    break;
                }
            }
        }
    }
}

/// Accept loop for the Unix domain socket listener.
#[cfg(all(feature = "native-transport", target_family = "unix"))]
async fn run_unix_listener(
    listener: &mut saikuro_transport::unix::UnixTransportListener,
    handle: saikuro_runtime_lib::RuntimeHandle,
    shutdown: &mut watch::Receiver<bool>,
) {
    use saikuro_transport::traits::TransportListener as _;

    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok(Some(transport)) => {
                        let peer_id = format!("unix-{}", uuid_short());
                        info!(peer = %peer_id, path = %listener.path().display(), "Unix connection accepted");
                        handle.accept_transport(transport, peer_id, CapabilitySet::default());
                    }
                    Ok(None) => {
                        info!("Unix listener closed");
                        break;
                    }
                    Err(e) => {
                        error!(error = %e, "Unix accept error");
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }
                }
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    info!("Unix listener shutting down");
                    break;
                }
            }
        }
    }
}

// Helpers

/// Build a stable peer ID string from a TCP transport's peer address.
fn peer_id_from_tcp(_t: &TcpTransport) -> String {
    // TcpTransport does not expose peer_addr publicly, so we use the
    // description plus a short unique suffix.
    format!("tcp-{}", uuid_short())
}

/// Return a short (8-char) random hex string for peer IDs.
fn uuid_short() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    // Simple approach: use current nanoseconds XOR a thread-local counter.
    // Good enough for log correlation; not a security primitive.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("{nanos:08x}")
}

/// Wait for Ctrl-C (SIGINT) or SIGTERM.
async fn wait_for_shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

// Logging initialisation

fn init_logging(level: &str, json: bool) {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let filter = EnvFilter::try_new(level).unwrap_or_else(|_| EnvFilter::new("info"));

    if json {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().json())
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer())
            .init();
    }
}
