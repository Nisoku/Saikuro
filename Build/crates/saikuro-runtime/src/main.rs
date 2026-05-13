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
    sync::atomic::{AtomicU64, Ordering},
    sync::Arc,
};

use anyhow::{Context, Result};
use clap::Parser;
use saikuro_core::capability::CapabilitySet;
use saikuro_exec::{signal, sleep, spawn, timeout, watch};

/// Milliseconds to wait before retrying after an accept error.
const ACCEPT_BACKOFF_MS: u64 = 50;
use saikuro_runtime::{config::RuntimeMode, RuntimeConfig, SaikuroRuntime};
use saikuro_transport::tcp::TcpTransportListener;
use saikuro_transport::traits::{Transport, TransportListener};
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
                listener_tasks.push(spawn(async move {
                    run_listener("TCP", &mut listener, h, &mut rx, |_| {
                        format!("tcp-{}", uuid_short())
                    })
                    .await;
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
        use saikuro_transport::websocket::WsTransportListener;
        let addr = SocketAddr::new(args.bind, args.ws_port);
        match WsTransportListener::bind(addr).await {
            Ok(mut listener) => {
                info!(addr = %listener.local_addr(), "WebSocket listener ready");
                let h = handle.clone();
                let mut rx = shutdown_rx.clone();
                listener_tasks.push(spawn(async move {
                    run_listener("WebSocket", &mut listener, h, &mut rx, |_| {
                        format!("ws-{}", uuid_short())
                    })
                    .await;
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
        match UnixTransportListener::bind(unix_path).await {
            Ok(mut listener) => {
                info!(path = %unix_path.display(), "Unix socket listener ready");
                let h = handle.clone();
                let mut rx = shutdown_rx.clone();
                listener_tasks.push(spawn(async move {
                    run_listener("Unix", &mut listener, h, &mut rx, |_t| {
                        format!("unix-{}", uuid_short())
                    })
                    .await;
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
        let _ = timeout(std::time::Duration::from_secs(5), task).await;
    }

    info!("saikuro-runtime stopped");
    Ok(())
}

// Transport accept loop

/// Generic accept loop for any [`TransportListener`].
///
/// Accepts connections in a loop until the listener is closed or a shutdown
/// signal is received. New connections are handed to the runtime via
/// [`RuntimeHandle::accept_transport`].
async fn run_listener<L>(
    name: &str,
    listener: &mut L,
    handle: saikuro_runtime::RuntimeHandle,
    shutdown: &mut watch::Receiver<bool>,
    peer_id: impl Fn(&L::Output) -> String,
) where
    L: TransportListener,
    L::Output: Transport,
{
    loop {
        saikuro_exec::select! {
            result = listener.accept() => {
                match result {
                    Ok(Some(transport)) => {
                        let id = peer_id(&transport);
                        info!(peer = %id, "{name} connection accepted");
                        handle.accept_transport(transport, id, CapabilitySet::default());
                    }
                    Ok(None) => {
                        info!("{name} listener closed");
                        break;
                    }
                    Err(e) => {
                        error!(error = %e, "{name} accept error");
                        sleep(std::time::Duration::from_millis(ACCEPT_BACKOFF_MS)).await;
                    }
                }
            }
            res = shutdown.changed() => {
                if res.is_err() || *shutdown.borrow() {
                    info!("{name} listener shutting down");
                    break;
                }
            }
        }
    }
}

// Helpers

/// Monotonically increasing counter for peer IDs.
static PEER_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Return a short (8-char) hex string for peer IDs.
///
/// Combines sub-second timestamp bits with a monotonic counter so IDs remain
/// unique even under high-frequency concurrent calls or a system clock
/// before Unix epoch.
fn uuid_short() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let count = PEER_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:016x}", millis.wrapping_add(count))
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
