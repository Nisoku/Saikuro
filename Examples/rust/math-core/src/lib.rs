use std::net::SocketAddr;
use std::sync::Mutex;

use saikuro::{Client, Error, MemoryAdapterTransport, Provider, Result};
use serde_json::Value as JsonValue;

/// Sink for the demo's output lines.
type LogSink = Box<dyn Fn(&str) + Send + Sync>;

/// Optional redirect installed by a target that has no stdout.
static LOG_SINK: Mutex<Option<LogSink>> = Mutex::new(None);

/// Redirect the demo's output to `sink` instead of stdout.
pub fn set_log_sink(sink: Option<impl Fn(&str) + Send + Sync + 'static>) {
    let mut guard = LOG_SINK.lock().unwrap_or_else(|e| e.into_inner());
    *guard = sink.map(|f| Box::new(f) as LogSink);
}

/// Emit one line of demo output.
pub fn log_line(line: &str) {
    let guard = LOG_SINK.lock().unwrap_or_else(|e| e.into_inner());
    match guard.as_ref() {
        Some(sink) => sink(line),
        None => println!("{line}"),
    }
}

/// Format and emit one line of demo output.
#[macro_export]
macro_rules! demo_log {
    ($($arg:tt)*) => {
        $crate::log_line(&format!($($arg)*))
    };
}

/// Install the embassy time driver and critical-section impl the WASI engines
/// need.
#[cfg(feature = "wasi-runtime-support")]
pub fn install_wasi_runtime_support() {
    use core::task::Waker;
    use std::sync::OnceLock;
    use std::time::Instant;

    struct WasiTimeDriver;

    impl embassy_time_driver::Driver for WasiTimeDriver {
        fn now(&self) -> u64 {
            static START: OnceLock<Instant> = OnceLock::new();
            START.get_or_init(Instant::now).elapsed().as_micros() as u64
        }

        fn schedule_wake(&self, _at: u64, waker: &Waker) {
            // The demo never actually sleeps, so anything waiting is ready now.
            waker.wake_by_ref();
        }
    }

    embassy_time_driver::time_driver_impl!(static WASI_TIME_DRIVER: WasiTimeDriver = WasiTimeDriver);

    struct NoopCriticalSection;

    critical_section::set_impl!(NoopCriticalSection);

    // SAFETY: wasm is single-threaded, so no other code can be inside this
    // critical section and there is no interrupt that could preempt it.
    unsafe impl critical_section::Impl for NoopCriticalSection {
        unsafe fn acquire() -> critical_section::RawRestoreState {
            Default::default()
        }

        unsafe fn release(_token: critical_section::RawRestoreState) {}
    }
}

/// Which transport the example wires the provider and client over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportChoice {
    /// Paired in-process channels. Always available.
    Memory,
    /// TCP, via whichever listener the target provides.
    Tcp,
}

impl TransportChoice {
    /// Parse the `--transport` value.
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "memory" => Ok(Self::Memory),
            "tcp" => Ok(Self::Tcp),
            other => Err(Error::ProviderError(format!(
                "unknown transport '{other}': expected 'memory' or 'tcp'"
            ))),
        }
    }

    /// The name this choice is selected by on the command line.
    pub fn name(self) -> &'static str {
        match self {
            Self::Memory => "memory",
            Self::Tcp => "tcp",
        }
    }
}

/// Parsed command line.
pub struct Options {
    /// Requested transport.
    pub transport: TransportChoice,
    /// Address the provider listens on for `--transport tcp`.
    pub addr: SocketAddr,
}

impl Options {
    /// Parse the command line, defaulting to in-memory on loopback.
    pub fn parse<I: Iterator<Item = String>>(args: I) -> Result<Self> {
        let mut transport = TransportChoice::Memory;
        let mut addr: SocketAddr = "127.0.0.1:0".parse().expect("valid default address");

        let mut args = args.peekable();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--transport" => {
                    let value = args
                        .next()
                        .ok_or_else(|| Error::ProviderError("--transport needs a value".into()))?;
                    transport = TransportChoice::parse(&value)?;
                }
                "--addr" => {
                    let value = args
                        .next()
                        .ok_or_else(|| Error::ProviderError("--addr needs a value".into()))?;
                    addr = value.parse().map_err(|e: std::net::AddrParseError| {
                        Error::ProviderError(format!("invalid --addr '{value}': {e}"))
                    })?;
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                other => {
                    return Err(Error::ProviderError(format!(
                        "unrecognised argument '{other}'"
                    )));
                }
            }
        }

        Ok(Self { transport, addr })
    }
}

/// Print the command line this example accepts.
pub fn print_usage() {
    demo_log!("usage: math [--transport memory|tcp] [--addr HOST:PORT]");
}

/// Read the two operands a math handler expects, defaulting to zero.
fn extract_two_floats(args: &[JsonValue]) -> (f64, f64) {
    let a = args.first().and_then(JsonValue::as_f64).unwrap_or(0.0);
    let b = args.get(1).and_then(JsonValue::as_f64).unwrap_or(0.0);
    (a, b)
}

/// The shared schema: every handler is registered here, for every transport.
pub fn math_provider() -> Provider {
    let mut provider = Provider::new("math");

    provider.register("add", |args: Vec<JsonValue>| async move {
        let (a, b) = extract_two_floats(&args);
        Ok(serde_json::json!(a + b))
    });

    provider.register("subtract", |args: Vec<JsonValue>| async move {
        let (a, b) = extract_two_floats(&args);
        Ok(serde_json::json!(a - b))
    });

    provider.register("multiply", |args: Vec<JsonValue>| async move {
        let (a, b) = extract_two_floats(&args);
        Ok(serde_json::json!(a * b))
    });

    provider.register("divide", |args: Vec<JsonValue>| async move {
        let (a, b) = extract_two_floats(&args);
        if b == 0.0 {
            return Err(Error::ProviderError("division by zero".into()));
        }
        Ok(serde_json::json!(a / b))
    });

    provider
}

/// Wire provider and client directly over a paired in-memory transport.
pub async fn run_in_memory() -> Result<()> {
    demo_log!("transport: in-memory");

    let (provider_transport, client_transport) = MemoryAdapterTransport::pair();
    let _provider = saikuro_exec::spawn(async move {
        // The provider announces its schema and serves until the client hangs up.
        let _ = math_provider().serve_on(Box::new(provider_transport)).await;
    });

    let client = Client::from_transport(Box::new(client_transport), None)?;
    run_demo(client).await
}

/// The shared client demo: call, cast, batch, and error handling.
pub async fn run_demo(client: Client) -> Result<()> {
    // call

    let sum = client
        .call(
            "math.add",
            vec![serde_json::json!(10), serde_json::json!(32)],
        )
        .await?;
    demo_log!("math.add(10, 32) = {sum}");
    assert_eq!(sum, serde_json::json!(42.0));

    let diff = client
        .call(
            "math.subtract",
            vec![serde_json::json!(100), serde_json::json!(58)],
        )
        .await?;
    demo_log!("math.subtract(100, 58) = {diff}");
    assert_eq!(diff, serde_json::json!(42.0));

    let product = client
        .call(
            "math.multiply",
            vec![serde_json::json!(6), serde_json::json!(7)],
        )
        .await?;
    demo_log!("math.multiply(6, 7) = {product}");
    assert_eq!(product, serde_json::json!(42.0));

    let quotient = client
        .call(
            "math.divide",
            vec![serde_json::json!(84.0), serde_json::json!(2.0)],
        )
        .await?;
    demo_log!("math.divide(84, 2) = {quotient}");
    assert_eq!(quotient, serde_json::json!(42.0));

    // cast (fire-and-forget)

    client
        .cast("math.add", vec![serde_json::json!(1), serde_json::json!(1)])
        .await?;
    demo_log!("cast sent (no response expected)");

    // batch

    let results = client
        .batch(vec![
            (
                "math.add".into(),
                vec![serde_json::json!(1), serde_json::json!(2)],
            ),
            (
                "math.multiply".into(),
                vec![serde_json::json!(3), serde_json::json!(4)],
            ),
        ])
        .await?;
    demo_log!("batch [add(1,2), multiply(3,4)] = {results:?}");

    // error handling

    let err = client
        .call(
            "math.divide",
            vec![serde_json::json!(1), serde_json::json!(0)],
        )
        .await
        .unwrap_err();
    demo_log!("divide by zero caught: {err}");
    assert!(matches!(
        err,
        Error::Remote { .. } | Error::ProviderError(_)
    ));

    client.close().await?;
    demo_log!("all examples passed");
    Ok(())
}
