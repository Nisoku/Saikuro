#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
pub mod fs_access;

#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
pub mod indexeddb;

#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
pub mod webstorage;

#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
pub mod opfs;

#[cfg(feature = "wasm")]
pub mod local_storage;

#[cfg(feature = "wasm")]
pub mod session_storage;
