#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
use crate::impl_web_storage;

#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
impl_web_storage!(LocalStorage, local_storage);

#[cfg(not(all(target_arch = "wasm32", feature = "wasm")))]
pub use crate::InMemoryStorage as LocalStorage;
