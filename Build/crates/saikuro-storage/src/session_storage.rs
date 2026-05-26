#[cfg(all(target_arch = "wasm32", feature = "wasm-storage"))]
use crate::impl_web_storage;

#[cfg(all(target_arch = "wasm32", feature = "wasm-storage"))]
impl_web_storage!(SessionStorage, session_storage);

#[cfg(not(all(target_arch = "wasm32", feature = "wasm-storage")))]
pub use crate::InMemoryStorage as SessionStorage;
