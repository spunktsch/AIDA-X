use serde::Deserialize;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerType {
    Gru,
    Lstm,
    Dense,
}

impl fmt::Display for LayerType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LayerType::Gru => write!(f, "gru"),
            LayerType::Lstm => write!(f, "lstm"),
            LayerType::Dense => write!(f, "dense"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayerConfig {
    pub layer_type: LayerType,
    pub shape: Vec<Option<i64>>,
    pub weights: Vec<Vec<Vec<f32>>>,
}

impl LayerConfig {
    pub fn input_size(&self) -> usize {
        self.shape.iter().rev().find_map(|v| v.map(|n| n as usize)).unwrap_or(0)
    }

    pub fn hidden_size(&self) -> usize {
        if self.layer_type == LayerType::Dense {
            0
        } else {
            self.shape.iter().rev().nth(1).and_then(|v| v.map(|n| n as usize)).unwrap_or(0)
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub in_shape: Vec<Option<i64>>,
    pub in_skip: i64,
    pub in_gain: f32,
    pub out_gain: f32,
    pub layers: Vec<LayerConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelArch {
    Gru8_1,
    Gru8_2,
    Gru8_3,
    Gru12_1,
    Gru12_2,
    Gru12_3,
    Gru16_1,
    Gru16_2,
    Gru16_3,
    Gru20_1,
    Gru20_2,
    Gru20_3,
    Gru24_1,
    Gru24_2,
    Gru24_3,
    Gru32_1,
    Gru32_2,
    Gru32_3,
    Gru40_1,
    Gru40_2,
    Gru40_3,
    Gru64_1,
    Gru64_2,
    Gru64_3,
    Gru80_1,
    Gru80_2,
    Gru80_3,
    Lstm8_1,
    Lstm8_2,
    Lstm8_3,
    Lstm12_1,
    Lstm12_2,
    Lstm12_3,
    Lstm16_1,
    Lstm16_2,
    Lstm16_3,
    Lstm20_1,
    Lstm20_2,
    Lstm20_3,
    Lstm24_1,
    Lstm24_2,
    Lstm24_3,
    Lstm32_1,
    Lstm32_2,
    Lstm32_3,
    Lstm40_1,
    Lstm40_2,
    Lstm40_3,
    Lstm64_1,
    Lstm64_2,
    Lstm64_3,
    Lstm80_1,
    Lstm80_2,
    Lstm80_3,
}

impl ModelArch {
    pub fn layer_type(&self) -> LayerType {
        match self {
            ModelArch::Gru8_1
            | ModelArch::Gru8_2
            | ModelArch::Gru8_3
            | ModelArch::Gru12_1
            | ModelArch::Gru12_2
            | ModelArch::Gru12_3
            | ModelArch::Gru16_1
            | ModelArch::Gru16_2
            | ModelArch::Gru16_3
            | ModelArch::Gru20_1
            | ModelArch::Gru20_2
            | ModelArch::Gru20_3
            | ModelArch::Gru24_1
            | ModelArch::Gru24_2
            | ModelArch::Gru24_3
            | ModelArch::Gru32_1
            | ModelArch::Gru32_2
            | ModelArch::Gru32_3
            | ModelArch::Gru40_1
            | ModelArch::Gru40_2
            | ModelArch::Gru40_3
            | ModelArch::Gru64_1
            | ModelArch::Gru64_2
            | ModelArch::Gru64_3
            | ModelArch::Gru80_1
            | ModelArch::Gru80_2
            | ModelArch::Gru80_3 => LayerType::Gru,
            ModelArch::Lstm8_1
            | ModelArch::Lstm8_2
            | ModelArch::Lstm8_3
            | ModelArch::Lstm12_1
            | ModelArch::Lstm12_2
            | ModelArch::Lstm12_3
            | ModelArch::Lstm16_1
            | ModelArch::Lstm16_2
            | ModelArch::Lstm16_3
            | ModelArch::Lstm20_1
            | ModelArch::Lstm20_2
            | ModelArch::Lstm20_3
            | ModelArch::Lstm24_1
            | ModelArch::Lstm24_2
            | ModelArch::Lstm24_3
            | ModelArch::Lstm32_1
            | ModelArch::Lstm32_2
            | ModelArch::Lstm32_3
            | ModelArch::Lstm40_1
            | ModelArch::Lstm40_2
            | ModelArch::Lstm40_3
            | ModelArch::Lstm64_1
            | ModelArch::Lstm64_2
            | ModelArch::Lstm64_3
            | ModelArch::Lstm80_1
            | ModelArch::Lstm80_2
            | ModelArch::Lstm80_3 => LayerType::Lstm,
        }
    }

    pub fn hidden_size(&self) -> usize {
        match self {
            ModelArch::Gru8_1 | ModelArch::Gru8_2 | ModelArch::Gru8_3
            | ModelArch::Lstm8_1 | ModelArch::Lstm8_2 | ModelArch::Lstm8_3 => 8,
            ModelArch::Gru12_1 | ModelArch::Gru12_2 | ModelArch::Gru12_3
            | ModelArch::Lstm12_1 | ModelArch::Lstm12_2 | ModelArch::Lstm12_3 => 12,
            ModelArch::Gru16_1 | ModelArch::Gru16_2 | ModelArch::Gru16_3
            | ModelArch::Lstm16_1 | ModelArch::Lstm16_2 | ModelArch::Lstm16_3 => 16,
            ModelArch::Gru20_1 | ModelArch::Gru20_2 | ModelArch::Gru20_3
            | ModelArch::Lstm20_1 | ModelArch::Lstm20_2 | ModelArch::Lstm20_3 => 20,
            ModelArch::Gru24_1 | ModelArch::Gru24_2 | ModelArch::Gru24_3
            | ModelArch::Lstm24_1 | ModelArch::Lstm24_2 | ModelArch::Lstm24_3 => 24,
            ModelArch::Gru32_1 | ModelArch::Gru32_2 | ModelArch::Gru32_3
            | ModelArch::Lstm32_1 | ModelArch::Lstm32_2 | ModelArch::Lstm32_3 => 32,
            ModelArch::Gru40_1 | ModelArch::Gru40_2 | ModelArch::Gru40_3
            | ModelArch::Lstm40_1 | ModelArch::Lstm40_2 | ModelArch::Lstm40_3 => 40,
            ModelArch::Gru64_1 | ModelArch::Gru64_2 | ModelArch::Gru64_3
            | ModelArch::Lstm64_1 | ModelArch::Lstm64_2 | ModelArch::Lstm64_3 => 64,
            ModelArch::Gru80_1 | ModelArch::Gru80_2 | ModelArch::Gru80_3
            | ModelArch::Lstm80_1 | ModelArch::Lstm80_2 | ModelArch::Lstm80_3 => 80,
        }
    }

    pub fn input_size(&self) -> usize {
        match self {
            ModelArch::Gru8_1 | ModelArch::Gru12_1 | ModelArch::Gru16_1
            | ModelArch::Gru20_1 | ModelArch::Gru24_1 | ModelArch::Gru32_1
            | ModelArch::Gru40_1 | ModelArch::Gru64_1 | ModelArch::Gru80_1
            | ModelArch::Lstm8_1 | ModelArch::Lstm12_1 | ModelArch::Lstm16_1
            | ModelArch::Lstm20_1 | ModelArch::Lstm24_1 | ModelArch::Lstm32_1
            | ModelArch::Lstm40_1 | ModelArch::Lstm64_1 | ModelArch::Lstm80_1 => 1,
            ModelArch::Gru8_2 | ModelArch::Gru12_2 | ModelArch::Gru16_2
            | ModelArch::Gru20_2 | ModelArch::Gru24_2 | ModelArch::Gru32_2
            | ModelArch::Gru40_2 | ModelArch::Gru64_2 | ModelArch::Gru80_2
            | ModelArch::Lstm8_2 | ModelArch::Lstm12_2 | ModelArch::Lstm16_2
            | ModelArch::Lstm20_2 | ModelArch::Lstm24_2 | ModelArch::Lstm32_2
            | ModelArch::Lstm40_2 | ModelArch::Lstm64_2 | ModelArch::Lstm80_2 => 2,
            ModelArch::Gru8_3 | ModelArch::Gru12_3 | ModelArch::Gru16_3
            | ModelArch::Gru20_3 | ModelArch::Gru24_3 | ModelArch::Gru32_3
            | ModelArch::Gru40_3 | ModelArch::Gru64_3 | ModelArch::Gru80_3
            | ModelArch::Lstm8_3 | ModelArch::Lstm12_3 | ModelArch::Lstm16_3
            | ModelArch::Lstm20_3 | ModelArch::Lstm24_3 | ModelArch::Lstm32_3
            | ModelArch::Lstm40_3 | ModelArch::Lstm64_3 | ModelArch::Lstm80_3 => 3,
        }
    }

    pub fn arch_name(&self) -> &'static str {
        match self {
            ModelArch::Gru8_1 => "GRU_8_1",
            ModelArch::Gru8_2 => "GRU_8_2",
            ModelArch::Gru8_3 => "GRU_8_3",
            ModelArch::Gru12_1 => "GRU_12_1",
            ModelArch::Gru12_2 => "GRU_12_2",
            ModelArch::Gru12_3 => "GRU_12_3",
            ModelArch::Gru16_1 => "GRU_16_1",
            ModelArch::Gru16_2 => "GRU_16_2",
            ModelArch::Gru16_3 => "GRU_16_3",
            ModelArch::Gru20_1 => "GRU_20_1",
            ModelArch::Gru20_2 => "GRU_20_2",
            ModelArch::Gru20_3 => "GRU_20_3",
            ModelArch::Gru24_1 => "GRU_24_1",
            ModelArch::Gru24_2 => "GRU_24_2",
            ModelArch::Gru24_3 => "GRU_24_3",
            ModelArch::Gru32_1 => "GRU_32_1",
            ModelArch::Gru32_2 => "GRU_32_2",
            ModelArch::Gru32_3 => "GRU_32_3",
            ModelArch::Gru40_1 => "GRU_40_1",
            ModelArch::Gru40_2 => "GRU_40_2",
            ModelArch::Gru40_3 => "GRU_40_3",
            ModelArch::Gru64_1 => "GRU_64_1",
            ModelArch::Gru64_2 => "GRU_64_2",
            ModelArch::Gru64_3 => "GRU_64_3",
            ModelArch::Gru80_1 => "GRU_80_1",
            ModelArch::Gru80_2 => "GRU_80_2",
            ModelArch::Gru80_3 => "GRU_80_3",
            ModelArch::Lstm8_1 => "LSTM_8_1",
            ModelArch::Lstm8_2 => "LSTM_8_2",
            ModelArch::Lstm8_3 => "LSTM_8_3",
            ModelArch::Lstm12_1 => "LSTM_12_1",
            ModelArch::Lstm12_2 => "LSTM_12_2",
            ModelArch::Lstm12_3 => "LSTM_12_3",
            ModelArch::Lstm16_1 => "LSTM_16_1",
            ModelArch::Lstm16_2 => "LSTM_16_2",
            ModelArch::Lstm16_3 => "LSTM_16_3",
            ModelArch::Lstm20_1 => "LSTM_20_1",
            ModelArch::Lstm20_2 => "LSTM_20_2",
            ModelArch::Lstm20_3 => "LSTM_20_3",
            ModelArch::Lstm24_1 => "LSTM_24_1",
            ModelArch::Lstm24_2 => "LSTM_24_2",
            ModelArch::Lstm24_3 => "LSTM_24_3",
            ModelArch::Lstm32_1 => "LSTM_32_1",
            ModelArch::Lstm32_2 => "LSTM_32_2",
            ModelArch::Lstm32_3 => "LSTM_32_3",
            ModelArch::Lstm40_1 => "LSTM_40_1",
            ModelArch::Lstm40_2 => "LSTM_40_2",
            ModelArch::Lstm40_3 => "LSTM_40_3",
            ModelArch::Lstm64_1 => "LSTM_64_1",
            ModelArch::Lstm64_2 => "LSTM_64_2",
            ModelArch::Lstm64_3 => "LSTM_64_3",
            ModelArch::Lstm80_1 => "LSTM_80_1",
            ModelArch::Lstm80_2 => "LSTM_80_2",
            ModelArch::Lstm80_3 => "LSTM_80_3",
        }
    }
}
