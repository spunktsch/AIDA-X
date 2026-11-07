//! Model creation (port of `custom_model_creator` / `loadModelFromStream`)
//! and the real-time model wrapper (`DynamicModel` + `applyModel`).
//!
//! Parsing, architecture detection and shape validation are pure Rust and
//! always available. Running a network needs an inference backend:
//!
//! | features                | `DynamicModel` runs on                         |
//! |-------------------------|------------------------------------------------|
//! | `rtneural` (default)    | C++ RTNeural via `aidadx_rtneural` (real-time) |
//! | `candle` only           | candle (`inference::ModelInference`, offline)  |
//! | neither                 | not available (parsing/validation only)        |
//!
//! With both features enabled, `DynamicModel` uses RTNeural and candle stays
//! available as `model::inference::ModelInference`.

use crate::model::types::{
    LayerType, ModelArch, ModelConfig, ModelError, ModelResult, WeightArray, MAX_INPUT_SIZE,
};

/// Maps (layer type, hidden size, input size) to one of the 54 supported architectures.
pub fn detect_arch(layer_type: LayerType, hidden_size: usize, input_size: usize) -> Option<ModelArch> {
    ModelArch::from_parts(layer_type, hidden_size, input_size)
}

/// `in_shape.back()`.
pub fn detect_input_size_from_config(config: &ModelConfig) -> usize {
    config.input_size()
}

/// `layers[0].shape.back()`.
pub fn detect_hidden_size_from_config(config: &ModelConfig) -> usize {
    config.hidden_size()
}

/// Runtime architecture detection, the equivalent of `custom_model_creator` in C++.
pub fn detect_arch_from_config(config: &ModelConfig) -> ModelResult<ModelArch> {
    let input_size = config.input_size();
    if input_size > MAX_INPUT_SIZE {
        return Err(ModelError::Unsupported(format!(
            "input size {input_size} (max {MAX_INPUT_SIZE})"
        )));
    }
    let layer_type = match config.layers.first().map(|l| l.layer_type) {
        Some(lt @ (LayerType::Gru | LayerType::Lstm)) => lt,
        Some(other) => {
            return Err(ModelError::Unsupported(format!(
                "first layer must be gru or lstm, got {other}"
            )))
        }
        None => return Err(ModelError::Parse("model has no layers".into())),
    };
    let hidden_size = config.hidden_size();
    detect_arch(layer_type, hidden_size, input_size).ok_or_else(|| {
        ModelError::Unsupported(format!(
            "no known architecture for {layer_type} hidden={hidden_size} input={input_size}"
        ))
    })
}

fn expect_shape(w: &WeightArray, expected: &[usize], what: &str) -> ModelResult<()> {
    if w.shape != expected {
        return Err(ModelError::Unsupported(format!(
            "{what}: expected weight shape {expected:?}, got {:?}",
            w.shape
        )));
    }
    Ok(())
}

/// Checks that `config` is exactly RNN(`arch`) + Dense(H → 1) with the weight
/// shapes RTNeural reads (Keras layout, see `inference.rs`). Shared by all backends.
pub fn validate_network(config: &ModelConfig, arch: ModelArch) -> ModelResult<()> {
    let input = arch.input_size();
    let hidden = arch.hidden_size();
    let gh = arch.layer_type().gate_count() * hidden;

    if config.layers.len() != 2 {
        return Err(ModelError::Unsupported(format!(
            "expected exactly 2 layers (RNN + Dense), got {}",
            config.layers.len()
        )));
    }
    let rnn = &config.layers[0];
    let dense = &config.layers[1];

    if rnn.layer_type != arch.layer_type() {
        return Err(ModelError::Unsupported(format!(
            "first layer is {}, expected {}",
            rnn.layer_type,
            arch.layer_type()
        )));
    }
    if dense.layer_type != LayerType::Dense || dense.units() != 1 {
        return Err(ModelError::Unsupported("second layer must be dense with 1 output".into()));
    }
    if rnn.weights.len() != 3 {
        return Err(ModelError::Unsupported(format!(
            "{} layer expects 3 weight arrays (kernel, recurrent, bias), got {}",
            rnn.layer_type,
            rnn.weights.len()
        )));
    }
    if dense.weights.len() != 2 {
        return Err(ModelError::Unsupported(format!(
            "dense layer expects 2 weight arrays, got {}",
            dense.weights.len()
        )));
    }

    expect_shape(&rnn.weights[0], &[input, gh], "kernel")?;
    expect_shape(&rnn.weights[1], &[hidden, gh], "recurrent kernel")?;
    match arch.layer_type() {
        LayerType::Gru => expect_shape(&rnn.weights[2], &[2, gh], "GRU bias")?,
        LayerType::Lstm => expect_shape(&rnn.weights[2], &[gh], "LSTM bias")?,
        LayerType::Dense => unreachable!("ModelArch is always recurrent"),
    }
    expect_shape(&dense.weights[0], &[hidden, 1], "dense kernel")?;
    expect_shape(&dense.weights[1], &[1], "dense bias")?;
    Ok(())
}

#[cfg(any(feature = "rtneural", feature = "candle"))]
pub use runtime::*;

/// Raw-network backend selected at compile time. API: `new`, `reset`, `run`,
/// `run_in_place`; `run*` must not allocate on the RTNeural path.
#[cfg(feature = "rtneural")]
mod net {
    use crate::model::types::{ModelArch, ModelConfig, ModelError, ModelResult, WeightArray};
    use serde_json::{json, Value};
    use std::sync::Arc;

    pub const BACKEND: &str = "rtneural";

    pub struct Net {
        model: aidadx_rtneural::Model,
        /// Canonical JSON the model was built from (used by `Clone`).
        json: Arc<str>,
    }

    fn weights_to_value(shape: &[usize], data: &[f32]) -> Value {
        match shape.split_first() {
            None => data.first().map_or(Value::Null, |&v| Value::from(f64::from(v))),
            Some((&n, rest)) => {
                let stride = rest.iter().product::<usize>();
                Value::Array(
                    (0..n)
                        .map(|i| weights_to_value(rest, &data[i * stride..(i + 1) * stride]))
                        .collect(),
                )
            }
        }
    }

    /// Re-serialises a validated config in the RTNeural JSON format (only the
    /// keys RTNeural reads; gains/skip are handled in Rust).
    fn config_to_json(config: &ModelConfig) -> String {
        let layers: Vec<Value> = config
            .layers
            .iter()
            .map(|l| {
                let w: Vec<Value> = l
                    .weights
                    .iter()
                    .map(|w: &WeightArray| weights_to_value(&w.shape, &w.data))
                    .collect();
                json!({
                    "type": l.layer_type.to_string(),
                    "activation": l.activation,
                    "shape": l.shape,
                    "weights": w,
                })
            })
            .collect();
        json!({ "in_shape": config.in_shape, "layers": layers }).to_string()
    }

    fn arch_index(arch: ModelArch) -> usize {
        ModelArch::ALL.iter().position(|&a| a == arch).expect("arch is in ALL")
    }

    impl Net {
        fn from_json(json: Arc<str>, arch: ModelArch) -> ModelResult<Self> {
            let model = aidadx_rtneural::Model::from_json(&json).map_err(|e| ModelError::Backend(e.0))?;
            // ModelVariantType = <NullModel, ModelArch::ALL...>
            if model.variant_index() != arch_index(arch) + 1 {
                return Err(ModelError::Backend(format!(
                    "RTNeural picked variant {} for {arch}",
                    model.variant_index()
                )));
            }
            Ok(Net { model, json })
        }

        pub fn new(config: &ModelConfig, arch: ModelArch) -> ModelResult<Self> {
            Self::from_json(config_to_json(config).into(), arch)
        }

        pub fn reset(&mut self) -> ModelResult<()> {
            self.model.reset();
            Ok(())
        }

        pub fn run(&mut self, input: &[f32], output: &mut [f32], p1: f32, p2: f32) -> ModelResult<()> {
            self.model.process(input, output, p1, p2);
            Ok(())
        }

        pub fn run_in_place(&mut self, buf: &mut [f32], p1: f32, p2: f32) -> ModelResult<()> {
            self.model.process_in_place(buf, p1, p2);
            Ok(())
        }
    }

    /// Builds a fresh copy of the same network. The recurrent state is **not**
    /// copied (RTNeural's `ModelT` is not safely copyable); the clone starts reset.
    impl Clone for Net {
        fn clone(&self) -> Self {
            let model = aidadx_rtneural::Model::from_json(&self.json)
                .expect("re-creating an already validated RTNeural model cannot fail");
            Net { model, json: Arc::clone(&self.json) }
        }
    }

    impl std::fmt::Debug for Net {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("Net").field("backend", &BACKEND).field("model", &self.model).finish()
        }
    }
}

#[cfg(all(feature = "candle", not(feature = "rtneural")))]
mod net {
    use crate::model::inference::ModelInference;
    use crate::model::types::{ModelArch, ModelConfig, ModelResult};

    pub const BACKEND: &str = "candle";

    #[derive(Clone, Debug)]
    pub struct Net(ModelInference);

    impl Net {
        pub fn new(config: &ModelConfig, arch: ModelArch) -> ModelResult<Self> {
            Ok(Net(ModelInference::from_config(config, arch)?))
        }

        pub fn reset(&mut self) -> ModelResult<()> {
            self.0.reset()
        }

        pub fn run(&mut self, input: &[f32], output: &mut [f32], p1: f32, p2: f32) -> ModelResult<()> {
            let n = self.0.input_size();
            for (x, y) in input.iter().zip(output.iter_mut()) {
                *y = self.0.forward(&[*x, p1, p2][..n])?;
            }
            Ok(())
        }

        pub fn run_in_place(&mut self, buf: &mut [f32], p1: f32, p2: f32) -> ModelResult<()> {
            let n = self.0.input_size();
            for s in buf.iter_mut() {
                *s = self.0.forward(&[*s, p1, p2][..n])?;
            }
            Ok(())
        }
    }
}

#[cfg(any(feature = "rtneural", feature = "candle"))]
mod runtime {
    use super::net::Net;
    use super::{detect_arch_from_config, validate_network};
    use crate::model::parser::parse_model;
    use crate::model::types::{db_to_coef, ModelArch, ModelConfig, ModelResult};

    use std::ptr;
    use std::sync::atomic::{AtomicPtr, Ordering};

    /// Name of the inference backend `DynamicModel` uses in this build
    /// (`"rtneural"` or `"candle"`).
    pub use super::net::BACKEND;

    /// Parses a JSON model and builds a ready-to-run [`DynamicModel`].
    pub fn custom_model_creator(json_str: &str) -> ModelResult<DynamicModel> {
        let config = parse_model(json_str)?;
        create_model(&config)
    }

    pub fn create_model(config: &ModelConfig) -> ModelResult<DynamicModel> {
        let arch = detect_arch_from_config(config)?;
        validate_network(config, arch)?;
        Ok(DynamicModel {
            arch,
            net: Net::new(config, arch)?,
            input_skip: config.in_skip != 0,
            input_gain: db_to_coef(config.in_gain),
            output_gain: db_to_coef(config.out_gain),
        })
    }

    /// Samples per inner block when the skip path needs a scratch buffer.
    const SCRATCH: usize = 128;

    /// A loaded model plus the extra info stored next to it in the C++ plugin.
    ///
    /// `Clone` yields the same network with a **reset** recurrent state on the
    /// RTNeural backend.
    #[derive(Clone, Debug)]
    pub struct DynamicModel {
        pub arch: ModelArch,
        net: Net,
        /// Model output is added to the (gained) input instead of replacing it.
        pub input_skip: bool,
        /// Linear input gain (from `in_gain` dB).
        pub input_gain: f32,
        /// Linear output gain (from `out_gain` dB).
        pub output_gain: f32,
    }

    impl DynamicModel {
        pub fn from_json(json_str: &str) -> ModelResult<Self> {
            custom_model_creator(json_str)
        }

        pub fn input_size(&self) -> usize {
            self.arch.input_size()
        }

        /// Clears the recurrent state. Real-time safe on the RTNeural backend.
        pub fn reset(&mut self) -> ModelResult<()> {
            self.net.reset()
        }

        /// One raw network step for a single audio sample (no gains, no skip).
        /// `param1`/`param2` are used only by 2- and 3-input models.
        pub fn forward_sample(&mut self, sample: f32, param1: f32, param2: f32) -> ModelResult<f32> {
            let mut x = [sample];
            self.net.run_in_place(&mut x, param1, param2)?;
            Ok(x[0])
        }

        /// Processes a mono buffer in place, mirroring `applyModel` in `aidadsp-plugin.cpp`:
        /// input gain on every sample, then either `out = (out + net(out)) * output_gain`
        /// (skip) or `out = net(out) * output_gain`.
        /// Parameters are held constant over the block (smoothing is the caller's job).
        /// Real-time safe (no allocation) on the RTNeural backend.
        pub fn process(&mut self, buffer: &mut [f32], param1: f32, param2: f32) -> ModelResult<()> {
            if self.input_gain != 1.0 {
                buffer.iter_mut().for_each(|s| *s *= self.input_gain);
            }
            let og = self.output_gain;
            if self.input_skip {
                let mut scratch = [0.0f32; SCRATCH];
                for chunk in buffer.chunks_mut(SCRATCH) {
                    let y = &mut scratch[..chunk.len()];
                    self.net.run(chunk, y, param1, param2)?;
                    for (s, &v) in chunk.iter_mut().zip(y.iter()) {
                        *s = (*s + v) * og;
                    }
                }
            } else {
                self.net.run_in_place(buffer, param1, param2)?;
                if og != 1.0 {
                    buffer.iter_mut().for_each(|s| *s *= og);
                }
            }
            Ok(())
        }

        /// Resets and runs 2048 zero samples through the model, as the C++ loader does
        /// before activating a new model, so the first real block does not click.
        pub fn prewarm(&mut self, param1: f32, param2: f32) -> ModelResult<()> {
            self.reset()?;
            let mut buf = [0.0f32; 2048];
            self.process(&mut buf, param1, param2)
        }
    }

    /// Lock-free hand-off of new models to the audio thread (the C++ plugin swaps a
    /// raw `DynamicModel*`).
    ///
    /// * Any non-real-time thread calls [`AtomicModel::publish`].
    /// * The single audio thread calls [`AtomicModel::take_pending`] at the start
    ///   of each block; it owns the active model and never blocks. The replaced model
    ///   is parked in a "retired" slot and freed later by [`AtomicModel::collect_garbage`]
    ///   (called automatically by `publish`), so deallocation stays off the audio thread
    ///   unless two swaps happen without any intermediate `publish`/`collect_garbage`.
    pub struct AtomicModel {
        pending: AtomicPtr<DynamicModel>,
        retired: AtomicPtr<DynamicModel>,
    }

    const _: () = {
        const fn assert_send<T: Send>() {}
        assert_send::<DynamicModel>();
    };

    impl Default for AtomicModel {
        fn default() -> Self {
            Self::new()
        }
    }

    impl std::fmt::Debug for AtomicModel {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("AtomicModel")
                .field("pending", &!self.pending.load(Ordering::Relaxed).is_null())
                .field("retired", &!self.retired.load(Ordering::Relaxed).is_null())
                .finish()
        }
    }

    fn free(p: *mut DynamicModel) {
        if !p.is_null() {
            // SAFETY: every non-null pointer stored in the slots came from
            // `Box::into_raw` and is owned exclusively by whoever swapped it out.
            drop(unsafe { Box::from_raw(p) });
        }
    }

    impl AtomicModel {
        pub fn new() -> Self {
            Self {
                pending: AtomicPtr::new(ptr::null_mut()),
                retired: AtomicPtr::new(ptr::null_mut()),
            }
        }

        /// Queues `model` for the audio thread. A previously queued model that was
        /// never picked up is dropped (latest wins). Call from non-real-time threads.
        pub fn publish(&self, model: DynamicModel) {
            self.collect_garbage();
            let new = Box::into_raw(Box::new(model));
            free(self.pending.swap(new, Ordering::AcqRel));
        }

        /// True if a model is waiting to be picked up.
        pub fn has_pending(&self) -> bool {
            !self.pending.load(Ordering::Acquire).is_null()
        }

        /// Audio thread: installs a pending model into `current` (if any) and retires
        /// the previous one. Returns `true` when the active model changed.
        /// Must only be called from a single consumer thread.
        pub fn take_pending(&self, current: &mut Option<Box<DynamicModel>>) -> bool {
            let p = self.pending.swap(ptr::null_mut(), Ordering::AcqRel);
            if p.is_null() {
                return false;
            }
            // SAFETY: pointer came from `Box::into_raw` in `publish`; swapping it out
            // transferred ownership to us.
            let new = unsafe { Box::from_raw(p) };
            if let Some(old) = current.replace(new) {
                free(self.retired.swap(Box::into_raw(old), Ordering::AcqRel));
            }
            true
        }

        /// Frees a model retired by the audio thread. Call from non-real-time threads.
        pub fn collect_garbage(&self) {
            free(self.retired.swap(ptr::null_mut(), Ordering::AcqRel));
        }
    }

    impl Drop for AtomicModel {
        fn drop(&mut self) {
            free(*self.pending.get_mut());
            free(*self.retired.get_mut());
        }
    }
}
