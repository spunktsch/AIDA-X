//! Neural amp model loading and inference (port of `model_variant.hpp` and the
//! model handling in `aidadsp-plugin.cpp`).

#[cfg(feature = "candle")]
pub mod inference;
pub mod loader;
pub mod parser;
pub mod types;

#[cfg(feature = "candle")]
pub use loader::{custom_model_creator, AtomicModel, DynamicModel};
pub use parser::parse_model;
pub use types::{ModelArch, ModelConfig, ModelError, ModelResult};

#[cfg(all(test, feature = "candle"))]
mod tests;
