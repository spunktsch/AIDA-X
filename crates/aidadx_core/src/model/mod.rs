//! Neural amp model loading and inference (port of `model_variant.hpp` and the
//! model handling in `aidadsp-plugin.cpp`).
//!
//! Parsing and validation are always available. `DynamicModel` /
//! `AtomicModel` need an inference backend: `rtneural` (default, real-time,
//! C++ RTNeural via FFI) or `candle` (offline). See [`loader`] for details.

#[cfg(feature = "candle")]
pub mod inference;
pub mod loader;
pub mod parser;
pub mod types;

#[cfg(any(feature = "rtneural", feature = "candle"))]
pub use loader::{create_model, custom_model_creator, AtomicModel, DynamicModel, BACKEND};
pub use parser::parse_model;
pub use types::{ModelArch, ModelConfig, ModelError, ModelResult};

#[cfg(test)]
mod tests;
