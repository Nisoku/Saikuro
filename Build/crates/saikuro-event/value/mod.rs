/// The dynamically-typed [`Value`] and [`ValueMap`] types.
#[allow(clippy::module_inception)]
pub mod value;

pub use value::{Value, ValueMap, core_to_json, json_to_core};
