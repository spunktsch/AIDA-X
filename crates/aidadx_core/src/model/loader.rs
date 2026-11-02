#[cfg(feature = "candle")]
use crate::model::inference::ModelInference;
use crate::model::parser::parse_model;
use crate::model::types::{LayerType, ModelArch};

use parking_lot::Mutex;

pub fn detect_arch(
    layer_type: LayerType,
    hidden_size: usize,
    input_size: usize,
) -> Option<ModelArch> {
    let hidden = match hidden_size {
        8 => true,
        12 => true,
        16 => true,
        20 => true,
        24 => true,
        32 => true,
        40 => true,
        64 => true,
        80 => true,
        _ => false,
    };

    if !hidden || input_size < 1 || input_size > 3 {
        return None;
    }

    match layer_type {
        LayerType::Gru => match (hidden_size, input_size) {
            (8, 1) => Some(ModelArch::Gru8_1),
            (8, 2) => Some(ModelArch::Gru8_2),
            (8, 3) => Some(ModelArch::Gru8_3),
            (12, 1) => Some(ModelArch::Gru12_1),
            (12, 2) => Some(ModelArch::Gru12_2),
            (12, 3) => Some(ModelArch::Gru12_3),
            (16, 1) => Some(ModelArch::Gru16_1),
            (16, 2) => Some(ModelArch::Gru16_2),
            (16, 3) => Some(ModelArch::Gru16_3),
            (20, 1) => Some(ModelArch::Gru20_1),
            (20, 2) => Some(ModelArch::Gru20_2),
            (20, 3) => Some(ModelArch::Gru20_3),
            (24, 1) => Some(ModelArch::Gru24_1),
            (24, 2) => Some(ModelArch::Gru24_2),
            (24, 3) => Some(ModelArch::Gru24_3),
            (32, 1) => Some(ModelArch::Gru32_1),
            (32, 2) => Some(ModelArch::Gru32_2),
            (32, 3) => Some(ModelArch::Gru32_3),
            (40, 1) => Some(ModelArch::Gru40_1),
            (40, 2) => Some(ModelArch::Gru40_2),
            (40, 3) => Some(ModelArch::Gru40_3),
            (64, 1) => Some(ModelArch::Gru64_1),
            (64, 2) => Some(ModelArch::Gru64_2),
            (64, 3) => Some(ModelArch::Gru64_3),
            (80, 1) => Some(ModelArch::Gru80_1),
            (80, 2) => Some(ModelArch::Gru80_2),
            (80, 3) => Some(ModelArch::Gru80_3),
            _ => None,
        },
        LayerType::Lstm => match (hidden_size, input_size) {
            (8, 1) => Some(ModelArch::Lstm8_1),
            (8, 2) => Some(ModelArch::Lstm8_2),
            (8, 3) => Some(ModelArch::Lstm8_3),
            (12, 1) => Some(ModelArch::Lstm12_1),
            (12, 2) => Some(ModelArch::Lstm12_2),
            (12, 3) => Some(ModelArch::Lstm12_3),
            (16, 1) => Some(ModelArch::Lstm16_1),
            (16, 2) => Some(ModelArch::Lstm16_2),
            (16, 3) => Some(ModelArch::Lstm16_3),
            (20, 1) => Some(ModelArch::Lstm20_1),
            (20, 2) => Some(ModelArch::Lstm20_2),
            (20, 3) => Some(ModelArch::Lstm20_3),
            (24, 1) => Some(ModelArch::Lstm24_1),
            (24, 2) => Some(ModelArch::Lstm24_2),
            (24, 3) => Some(ModelArch::Lstm24_3),
            (32, 1) => Some(ModelArch::Lstm32_1),
            (32, 2) => Some(ModelArch::Lstm32_2),
            (32, 3) => Some(ModelArch::Lstm32_3),
            (40, 1) => Some(ModelArch::Lstm40_1),
            (40, 2) => Some(ModelArch::Lstm40_2),
            (40, 3) => Some(ModelArch::Lstm40_3),
            (64, 1) => Some(ModelArch::Lstm64_1),
            (64, 2) => Some(ModelArch::Lstm64_2),
            (64, 3) => Some(ModelArch::Lstm64_3),
            (80, 1) => Some(ModelArch::Lstm80_1),
            (80, 2) => Some(ModelArch::Lstm80_2),
            (80, 3) => Some(ModelArch::Lstm80_3),
            _ => None,
        },
        LayerType::Dense => None,
    }
}

pub fn detect_input_size_from_config(config: &crate::model::types::ModelConfig) -> usize {
    let in_shape = &config.in_shape;
    in_shape.iter().rev().find_map(|v| v.map(|n| n as usize)).unwrap_or(1)
}

pub fn detect_hidden_size_from_config(config: &crate::model::types::ModelConfig) -> usize {
    if config.layers.is_empty() {
        return 0;
    }
    let first_layer = &config.layers[0];
    let shape = &first_layer.shape;
    shape.iter().rev().find_map(|v| v.map(|n| n as usize)).unwrap_or(0)
}

#[cfg(feature = "candle")]
pub fn custom_model_creator(json_str: &str) -> Result<DynamicModel, String> {
    let config = parse_model(json_str)?;
    create_model(&config)
}

#[cfg(feature = "candle")]
pub fn create_model(config: &crate::model::types::ModelConfig) -> Result<DynamicModel, String> {
    let layer_type = match config.layers.first().map(|l| &l.layer_type) {
        Some(LayerType::Gru) => LayerType::Gru,
        Some(LayerType::Lstm) => LayerType::Lstm,
        _ => return Err("First layer must be GRU or LSTM".to_string()),
    };

    let hidden_size = detect_hidden_size_from_config(config);
    let input_size = detect_input_size_from_config(config);

    let arch = detect_arch(layer_type, hidden_size, input_size)
        .ok_or_else(|| format!("Unsupported model architecture: layer_type={}, hidden={}, input={}", layer_type, hidden_size, input_size))?;

    if input_size > 3 {
        return Err(format!("Input size {} not supported (max 3)", input_size));
    }

    let inference = ModelInference::from_config(config, arch)?;

    let input_gain = if config.in_gain > -90.0 {
        f32::powf(10.0, config.in_gain * 0.05)
    } else {
        0.0
    };

    let output_gain = if config.out_gain > -90.0 {
        f32::powf(10.0, config.out_gain * 0.05)
    } else {
        0.0
    };

    Ok(DynamicModel {
        arch,
        inference,
        input_skip: config.in_skip != 0,
        input_gain,
        output_gain,
    })
}

#[cfg(feature = "candle")]
#[derive(Clone)]
pub struct DynamicModel {
    pub arch: ModelArch,
    pub inference: ModelInference,
    pub input_skip: bool,
    pub input_gain: f32,
    pub output_gain: f32,
}

#[cfg(feature = "candle")]
impl DynamicModel {
    pub fn forward(&self, input: &mut [f32], param1: f32, param2: f32) {
        let input_size = self.arch.input_size();
        let has_skip = self.input_skip;
        let in_gain = self.input_gain;
        let out_gain = self.output_gain;

        if in_gain != 1.0 {
            for sample in input.iter_mut().take(input_size) {
                *sample *= in_gain;
            }
        }

        let output = match self
            .inference
            .forward(input, param1, param2)
        {
            Ok(out) => out[0],
            Err(_) => 0.0,
        };

        if has_skip {
            for i in 0..input.len() {
                input[i] += output;
            }
        } else {
            for i in 0..input.len() {
                input[i] = output * out_gain;
            }
        }

        if has_skip && out_gain != 1.0 {
            for sample in input.iter_mut() {
                *sample *= out_gain;
            }
        }
    }

    pub fn reset(&self) {
    }
}

#[cfg(feature = "candle")]
#[derive(Clone)]
pub struct AtomicModel {
    pub model: Mutex<DynamicModel>,
}

#[cfg(feature = "candle")]
impl std::fmt::Debug for AtomicModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AtomicModel").finish()
    }
}

#[cfg(feature = "candle")]
impl AtomicModel {
    pub fn new(model: DynamicModel) -> Self {
        Self {
            model: Mutex::new(model),
        }
    }

    pub fn swap(&self, new_model: DynamicModel) {
        let mut existing = self.model.lock();
        *existing = new_model;
    }

    pub fn model(&self) -> parking_lot::MutexGuard<DynamicModel> {
        self.model.lock()
    }
}
