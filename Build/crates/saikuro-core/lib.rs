#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;

mod protocol;
pub use protocol::*;

mod value;
pub use value::*;

mod codec;
pub use codec::*;

// Engine selection guard
#[cfg(all(
    feature = "native",
    any(feature = "wasm", feature = "embedded", feature = "no_std")
))]
compile_error!(
    "saikuro-core: only one engine feature (native/wasm/embedded/no_std) may be enabled"
);

#[cfg(all(feature = "wasm", any(feature = "embedded", feature = "no_std")))]
compile_error!(
    "saikuro-core: only one engine feature (native/wasm/embedded/no_std) may be enabled"
);

#[cfg(all(feature = "embedded", feature = "no_std"))]
compile_error!(
    "saikuro-core: only one engine feature (native/wasm/embedded/no_std) may be enabled"
);

#[cfg(not(any(
    feature = "native",
    feature = "wasm",
    feature = "embedded",
    feature = "no_std"
)))]
compile_error!("saikuro-core: an engine feature (native/wasm/embedded/no_std) must be enabled");

#[cfg(all(feature = "std", feature = "no_std"))]
compile_error!("saikuro-core: the `std` toolchain flag is incompatible with the `no_std` engine");

/// Wire-level protocol version. All envelopes carry this; the runtime
/// rejects messages with an incompatible version.
pub const PROTOCOL_VERSION: u32 = 1;
