#[cfg(feature = "wasi-preview1")]
mod preview1;
#[cfg(feature = "wasi-component")]
mod preview2;

#[cfg(feature = "wasi-preview1")]
pub use preview1::{WasiFileStore, WasiKvStore};

#[cfg(feature = "wasi-component")]
pub use preview2::{WasiFileStore, WasiKvStore};
