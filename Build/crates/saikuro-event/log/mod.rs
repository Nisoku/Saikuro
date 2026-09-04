/// Log severity levels.
pub mod level;
/// A single structured log record and its fields.
pub mod record;
/// The [`LogSink`](sink::LogSink) trait and built-in sink implementations.
pub mod sink;

#[cfg(feature = "collector")]
pub mod ring;

pub use level::*;
pub use record::*;
#[cfg(feature = "collector")]
pub use ring::*;
pub use sink::*;

#[cfg(
    any(
        all(feature = "native", any(feature = "no_std", feature = "wasm", feature = "embedded")),
        all(feature = "no_std", any(feature = "native", feature = "wasm", feature = "embedded")),
        all(feature = "wasm", any(feature = "native", feature = "no_std", feature = "embedded")),
        all(feature = "embedded", any(feature = "native", feature = "no_std", feature = "wasm"))
    )
)]
compile_error!("exactly one engine must be enabled: native | no_std | wasm | embedded");

#[cfg(all(feature = "std", feature = "no_std", not(target_os = "wasi")))]
compile_error!("the no_std engine cannot be combined with the std toolchain");

#[cfg(not(any(feature = "native", feature = "no_std", feature = "wasm", feature = "embedded")))]
compile_error!("exactly one engine must be selected: native | no_std | wasm | embedded");

#[cfg(feature = "native")]
mod native;
#[cfg(feature = "native")]
pub use native::*;

#[cfg(feature = "wasm")]
mod wasm;
#[cfg(feature = "wasm")]
pub use wasm::*;

#[cfg(feature = "embedded")]
mod embedded;
#[cfg(feature = "embedded")]
pub use embedded::*;
