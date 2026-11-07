//! Model creation (port of `custom_model_creator` / `loadModelFromStream`)
//! and the real-time model wrapper (`DynamicModel` + `applyModel`).

#[cfg(feature = "candle")]
use crate::model::inference::ModelInference;
#[cfg(feature = "candle")]
use crate::model::parser::parse_model;
use crate::model::types::{
    LayerType, ModelArch, ModelConfig, ModelError, ModelResult, MAX_INPUT_SIZE,
};
#[cfg(feature = "candle")]
use crate::model::types::db_to_coef;

#[cfg(feature = "candle")]
use std::ptr;
#[cfg(feature = "candle")]
use std::sync::atomic::{AtomicPtr, Ordering};

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

/// Parses a JSON model and builds a ready-to-run [`DynamicModel`].
#[cfg(feature = "candle")]
pub fn custom_model_creator(json_str: &str) -> ModelResult<DynamicModel> {
    let config = parse_model(json_str)?;
    create_model(&config)
}

#[cfg(feature = "candle")]
pub fn create_model(config: &ModelConfig) -> ModelResult<DynamicModel> {
    let arch = detect_arch_from_config(config)?;
    let inference = ModelInference::from_config(config, arch)?;
    Ok(DynamicModel {
        arch,
        inference,
        input_skip: config.in_skip != 0,
        input_gain: db_to_coef(config.in_gain),
        output_gain: db_to_coef(config.out_gain),
    })
}

/// A loaded model plus the extra info stored next to it in the C++ plugin.
#[cfg(feature = "candle")]
#[derive(Clone, Debug)]
pub struct DynamicModel {
    pub arch: ModelArch,
    pub inference: ModelInference,
    /// Model output is added to the (gained) input instead of replacing it.
    pub input_skip: bool,
    /// Linear input gain (from `in_gain` dB).
    pub input_gain: f32,
    /// Linear output gain (from `out_gain` dB).
    pub output_gain: f32,
}

#[cfg(feature = "candle")]
impl DynamicModel {
    pub fn from_json(json_str: &str) -> ModelResult<Self> {
        custom_model_creator(json_str)
    }

    pub fn input_size(&self) -> usize {
        self.arch.input_size()
    }

    /// Clears the recurrent state.
    pub fn reset(&mut self) -> ModelResult<()> {
        self.inference.reset()
    }

    /// One raw network step for a single audio sample (no gains, no skip).
    /// `param1`/`param2` are used only by 2- and 3-input models.
    pub fn forward_sample(&mut self, sample: f32, param1: f32, param2: f32) -> ModelResult<f32> {
        let input = [sample, param1, param2];
        self.inference.forward(&input[..self.arch.input_size()])
    }

    /// Processes a mono buffer in place, mirroring `applyModel` in `aidadsp-plugin.cpp`:
    /// input gain on every sample, then either `out += net(out)` (skip) or
    /// `out = net(out) * output_gain`; with skip, output gain is applied afterwards.
    /// Parameters are held constant over the block (smoothing is the caller's job).
    pub fn process(&mut self, buffer: &mut [f32], param1: f32, param2: f32) -> ModelResult<()> {
        if self.input_gain != 1.0 {
            buffer.iter_mut().for_each(|s| *s *= self.input_gain);
        }
        for s in buffer.iter_mut() {
            let y = self.forward_sample(*s, param1, param2)?;
            if self.input_skip {
                *s += y;
            } else {
                *s = y * self.output_gain;
            }
        }
        if self.input_skip && self.output_gain != 1.0 {
            buffer.iter_mut().for_each(|s| *s *= self.output_gain);
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
#[cfg(feature = "candle")]
pub struct AtomicModel {
    pending: AtomicPtr<DynamicModel>,
    retired: AtomicPtr<DynamicModel>,
}

#[cfg(feature = "candle")]
const _: () = {
    const fn assert_send<T: Send>() {}
    assert_send::<DynamicModel>();
};

#[cfg(feature = "candle")]
impl Default for AtomicModel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "candle")]
impl std::fmt::Debug for AtomicModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AtomicModel")
            .field("pending", &!self.pending.load(Ordering::Relaxed).is_null())
            .field("retired", &!self.retired.load(Ordering::Relaxed).is_null())
            .finish()
    }
}

#[cfg(feature = "candle")]
fn free(p: *mut DynamicModel) {
    if !p.is_null() {
        // SAFETY: every non-null pointer stored in the slots came from
        // `Box::into_raw` and is owned exclusively by whoever swapped it out.
        drop(unsafe { Box::from_raw(p) });
    }
}

#[cfg(feature = "candle")]
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

#[cfg(feature = "candle")]
impl Drop for AtomicModel {
    fn drop(&mut self) {
        free(*self.pending.get_mut());
        free(*self.retired.get_mut());
    }
}
