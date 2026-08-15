//! Saikuro Runtime Server (native binary)
//!
//! Standalone process that accepts connections from Saikuro adapters over TCP,
//! WebSocket, and Unix domain sockets. It acts as the central message broker:
//! adapters announce their capabilities and the runtime routes invocations
//! among them.
//!
//! The engine-agnostic orchestration lives in the `saikuro-runtime` library;
//! this binary only handles native concerns (CLI, `std::fs` schema loading,
//! OS signal handling) and drives [`SaikuroRuntime::serve`].
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
//!   ```

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use saikuro_exec::{signal, spawn, timeout, watch};
use saikuro_runtime::config::RuntimeMode;
use saikuro_runtime::SaikuroRuntime;
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
    #[arg(long, value_name = "PATH")]
    schema: Option<std::path::PathBuf>,

    /// Port to listen for raw TCP connections.
    #[arg(long, value_name = "PORT", default_value = "7700")]
    tcp_port: u16,

    /// Port to listen for WebSocket connections.
    #[arg(long, value_name = "PORT", default_value = "7701")]
    ws_port: u16,

    /// Path to a Unix domain socket to create and listen on.
    #[arg(long, value_name = "PATH")]
    unix: Option<std::path::PathBuf>,

    /// Bind address for TCP and WebSocket listeners.
    #[arg(long, value_name = "ADDR", default_value = "127.0.0.1")]
    bind: IpAddr,

    /// Runtime mode.
    #[arg(long, value_name = "MODE", default_value = "development")]
    mode: CliMode,

    /// Minimum log level to emit.
    #[arg(long, value_name = "LEVEL", default_value = "info", env = "SAIKURO_LOG")]
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

fn main() -> Result<()> {
    saikuro_exec::block_on(async_main())
}

async fn async_main() -> Result<()> {
    let args = Args::parse();

    init_logging(&args.log_level, args.json_logs);

    info!(
        version = env!("CARGO_PKG_VERSION"),
        mode = ?args.mode,
        "saikuro-runtime starting"
    );

    // Build the runtime.
    let mut builder = SaikuroRuntime::builder()
        .mode(args.mode.into())
        .json_logs(args.json_logs);

    // Load a baked-in schema from disk (native only).
    if let Some(schema_path) = &args.schema {
        let raw = std::fs::read(schema_path)
            .with_context(|| format!("reading schema file {}", schema_path.display()))?;
        let bytes: &'static [u8] = Box::leak(raw.into_boxed_slice());
        builder = builder.schema_bytes(bytes);
        info!(path = %schema_path.display(), "loaded static schema");
    }

    let runtime = Arc::new(builder.build());

    // Set up graceful shutdown channel.
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Each enabled listener type is driven by its own `serve` task.
    let mut serve_tasks: Vec<_> = Vec::new();

    // TCP listener.
    #[cfg(feature = "tcp")]
    if !args.no_tcp {
        use saikuro_transport::tcp::TcpTransportListener;
        let addr = SocketAddr::new(args.bind, args.tcp_port);
        match TcpTransportListener::bind(addr).await {
            Ok(listener) => {
                info!(addr = %listener.local_addr(), "TCP listener ready");
                let rt = runtime.clone();
                let mut rx = shutdown_rx.clone();
                serve_tasks.push(spawn(async move { rt.serve(vec![listener], rx).await; }));
            }
            Err(e) => {
                error!(addr = %addr, error = %e, "failed to bind TCP listener");
                return Err(anyhow::anyhow!("TCP bind failed: {e}"));
            }
        }
    }

    // WebSocket listener.
    #[cfg(feature = "ws")]
    if !args.no_ws {
        use saikuro_transport::websocket::WsTransportListener;
        let addr = SocketAddr::new(args.bind, args.ws_port);
        match WsTransportListener::bind(addr).await {
            Ok(listener) => {
                info!(addr = %listener.local_addr(), "WebSocket listener ready");
                let rt = runtime.clone();
                let mut rx = shutdown_rx.clone();
                serve_tasks.push(spawn(async move { rt.serve(vec![listener], rx).await; }));
            }
            Err(e) => {
                error!(addr = %addr, error = %e, "failed to bind WebSocket listener");
                return Err(anyhow::anyhow!("WebSocket bind failed: {e}"));
            }
        }
    }

    // Unix domain socket listener (Unix-only).
    #[cfg(all(feature = "unix", target_family = "unix"))]
    if let Some(unix_path) = &args.unix {
        use saikuro_transport::unix::UnixTransportListener;
        match UnixTransportListener::bind(unix_path).await {
            Ok(listener) => {
                info!(path = %unix_path.display(), "Unix socket listener ready");
                let rt = runtime.clone();
                let mut rx = shutdown_rx.clone();
                serve_tasks.push(spawn(async move { rt.serve(vec![listener], rx).await; }));
            }
            Err(e) => {
                error!(path = %unix_path.display(), error = %e, "failed to bind Unix listener");
                return Err(anyhow::anyhow!("Unix socket bind failed: {e}"));
            }
        }
    }

    if serve_tasks.is_empty() {
        warn!("no listeners are active; all transports were disabled");
    }

    // Wait for Ctrl-C or SIGTERM.
    wait_for_shutdown_signal().await;
    info!("shutdown signal received; stopping listeners");

    let _ = shutdown_tx.send(true);
    runtime.shutdown();

    // Allow the listener tasks to exit cleanly.
    for task in serve_tasks {
        let _ = timeout(std::time::Duration::from_secs(5), task).await;
    }

    info!("saikuro-runtime stopped");
    Ok(())
}

/// Wait for Ctrl-C (SIGINT) or SIGTERM.
async fn wait_for_shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = signal::ctrl_c().await {
            error!("failed to install Ctrl-C handler: {e}");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => {
                error!("failed to install SIGTERM handler: {e}");
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    saikuro_exec::select! {
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
