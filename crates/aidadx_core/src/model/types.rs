//! Plain data types describing an AIDA-X / RTNeural model JSON file.

use std::fmt;

/// Error type for model parsing, validation and inference.
#[derive(Debug)]
pub enum ModelError {
    /// The JSON could not be parsed or does not follow the expected schema.
    Parse(String),
    /// The JSON is well-formed but describes a model AIDA-X cannot run.
    Unsupported(String),
    /// The inference backend (RTNeural) rejected the model.
    Backend(String),
    /// A tensor operation failed during model construction or inference.
    #[cfg(feature = "candle")]
    Candle(candle_core::Error),
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelError::Parse(msg) => write!(f, "model parse error: {msg}"),
            ModelError::Unsupported(msg) => write!(f, "unsupported model: {msg}"),
            ModelError::Backend(msg) => write!(f, "inference backend error: {msg}"),
            #[cfg(feature = "candle")]
            ModelError::Candle(e) => write!(f, "tensor error: {e}"),
        }
    }
}

impl std::error::Error for ModelError {}

#[cfg(feature = "candle")]
impl From<candle_core::Error> for ModelError {
    fn from(e: candle_core::Error) -> Self {
        ModelError::Candle(e)
    }
}

pub type ModelResult<T> = Result<T, ModelError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayerType {
    Gru,
    Lstm,
    Dense,
}

impl LayerType {
    /// Number of stacked gates in the kernel/recurrent matrices (Keras layout).
    pub fn gate_count(self) -> usize {
        match self {
            LayerType::Gru => 3,
            LayerType::Lstm => 4,
            LayerType::Dense => 1,
        }
    }
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

/// A dense, row-major N-dimensional array of weights as found in the JSON
/// (`[[...], [...]]` becomes `shape = [rows, cols]`).
#[derive(Debug, Clone, PartialEq)]
pub struct WeightArray {
    pub shape: Vec<usize>,
    pub data: Vec<f32>,
}

impl WeightArray {
    /// Row `i` of a 2-D array.
    pub fn row(&self, i: usize) -> &[f32] {
        let cols = self.shape[1];
        &self.data[i * cols..(i + 1) * cols]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayerConfig {
    pub layer_type: LayerType,
    /// Activation name from the JSON (ignored by the RNN/Dense layers AIDA-X uses).
    pub activation: String,
    /// Raw `shape` field, e.g. `[null, null, 12]`.
    pub shape: Vec<Option<i64>>,
    /// Weight arrays in file order.
    pub weights: Vec<WeightArray>,
}

impl LayerConfig {
    /// Output size of the layer (`shape.back()` in RTNeural's parser).
    pub fn units(&self) -> usize {
        self.shape.last().copied().flatten().map(|n| n.max(0) as usize).unwrap_or(0)
    }
}

/// Parsed model file. Gains are kept in dB exactly as stored in the JSON.
#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub in_shape: Vec<Option<i64>>,
    /// `in_skip` (0 or 1): model output is added to the (gained) input signal.
    pub in_skip: i64,
    /// Input gain in dB (0.0 when absent).
    pub in_gain: f32,
    /// Output gain in dB (0.0 when absent).
    pub out_gain: f32,
    pub layers: Vec<LayerConfig>,
}

impl ModelConfig {
    /// Network input size (`in_shape.back()`): 1 = audio only, 2/3 = audio + params.
    pub fn input_size(&self) -> usize {
        self.in_shape.last().copied().flatten().map(|n| n.max(0) as usize).unwrap_or(0)
    }

    /// Hidden size of the recurrent (first) layer.
    pub fn hidden_size(&self) -> usize {
        self.layers.first().map(LayerConfig::units).unwrap_or(0)
    }
}

/// dB to linear coefficient, identical to `DB_CO` in `aidadsp-plugin.cpp`.
pub fn db_to_coef(db: f32) -> f32 {
    if db > -90.0 {
        10f32.powf(db * 0.05)
    } else {
        0.0
    }
}

pub const SUPPORTED_HIDDEN_SIZES: [usize; 9] = [8, 12, 16, 20, 24, 32, 40, 64, 80];
pub const MAX_INPUT_SIZE: usize = 3;

macro_rules! model_archs {
    ($( $variant:ident => ($lt:ident, $hidden:literal, $input:literal, $name:literal) ),* $(,)?) => {
        /// The 54 architectures supported by the C++ plugin (`model_variant.hpp`).
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum ModelArch {
            $( $variant, )*
        }

        impl ModelArch {
            pub const ALL: [ModelArch; 54] = [ $( ModelArch::$variant, )* ];

            fn parts(self) -> (LayerType, usize, usize, &'static str) {
                match self {
                    $( ModelArch::$variant => (LayerType::$lt, $hidden, $input, $name), )*
                }
            }
        }
    };
}

model_archs! {
    Gru8_1 => (Gru, 8, 1, "GRU_8_1"),    Gru8_2 => (Gru, 8, 2, "GRU_8_2"),    Gru8_3 => (Gru, 8, 3, "GRU_8_3"),
    Gru12_1 => (Gru, 12, 1, "GRU_12_1"), Gru12_2 => (Gru, 12, 2, "GRU_12_2"), Gru12_3 => (Gru, 12, 3, "GRU_12_3"),
    Gru16_1 => (Gru, 16, 1, "GRU_16_1"), Gru16_2 => (Gru, 16, 2, "GRU_16_2"), Gru16_3 => (Gru, 16, 3, "GRU_16_3"),
    Gru20_1 => (Gru, 20, 1, "GRU_20_1"), Gru20_2 => (Gru, 20, 2, "GRU_20_2"), Gru20_3 => (Gru, 20, 3, "GRU_20_3"),
    Gru24_1 => (Gru, 24, 1, "GRU_24_1"), Gru24_2 => (Gru, 24, 2, "GRU_24_2"), Gru24_3 => (Gru, 24, 3, "GRU_24_3"),
    Gru32_1 => (Gru, 32, 1, "GRU_32_1"), Gru32_2 => (Gru, 32, 2, "GRU_32_2"), Gru32_3 => (Gru, 32, 3, "GRU_32_3"),
    Gru40_1 => (Gru, 40, 1, "GRU_40_1"), Gru40_2 => (Gru, 40, 2, "GRU_40_2"), Gru40_3 => (Gru, 40, 3, "GRU_40_3"),
    Gru64_1 => (Gru, 64, 1, "GRU_64_1"), Gru64_2 => (Gru, 64, 2, "GRU_64_2"), Gru64_3 => (Gru, 64, 3, "GRU_64_3"),
    Gru80_1 => (Gru, 80, 1, "GRU_80_1"), Gru80_2 => (Gru, 80, 2, "GRU_80_2"), Gru80_3 => (Gru, 80, 3, "GRU_80_3"),
    Lstm8_1 => (Lstm, 8, 1, "LSTM_8_1"),    Lstm8_2 => (Lstm, 8, 2, "LSTM_8_2"),    Lstm8_3 => (Lstm, 8, 3, "LSTM_8_3"),
    Lstm12_1 => (Lstm, 12, 1, "LSTM_12_1"), Lstm12_2 => (Lstm, 12, 2, "LSTM_12_2"), Lstm12_3 => (Lstm, 12, 3, "LSTM_12_3"),
    Lstm16_1 => (Lstm, 16, 1, "LSTM_16_1"), Lstm16_2 => (Lstm, 16, 2, "LSTM_16_2"), Lstm16_3 => (Lstm, 16, 3, "LSTM_16_3"),
    Lstm20_1 => (Lstm, 20, 1, "LSTM_20_1"), Lstm20_2 => (Lstm, 20, 2, "LSTM_20_2"), Lstm20_3 => (Lstm, 20, 3, "LSTM_20_3"),
    Lstm24_1 => (Lstm, 24, 1, "LSTM_24_1"), Lstm24_2 => (Lstm, 24, 2, "LSTM_24_2"), Lstm24_3 => (Lstm, 24, 3, "LSTM_24_3"),
    Lstm32_1 => (Lstm, 32, 1, "LSTM_32_1"), Lstm32_2 => (Lstm, 32, 2, "LSTM_32_2"), Lstm32_3 => (Lstm, 32, 3, "LSTM_32_3"),
    Lstm40_1 => (Lstm, 40, 1, "LSTM_40_1"), Lstm40_2 => (Lstm, 40, 2, "LSTM_40_2"), Lstm40_3 => (Lstm, 40, 3, "LSTM_40_3"),
    Lstm64_1 => (Lstm, 64, 1, "LSTM_64_1"), Lstm64_2 => (Lstm, 64, 2, "LSTM_64_2"), Lstm64_3 => (Lstm, 64, 3, "LSTM_64_3"),
    Lstm80_1 => (Lstm, 80, 1, "LSTM_80_1"), Lstm80_2 => (Lstm, 80, 2, "LSTM_80_2"), Lstm80_3 => (Lstm, 80, 3, "LSTM_80_3"),
}

impl ModelArch {
    /// Equivalent of the `is_model_type_*` checks in `model_variant.hpp`.
    pub fn from_parts(layer_type: LayerType, hidden_size: usize, input_size: usize) -> Option<Self> {
        Self::ALL.iter().copied().find(|a| {
            let (lt, h, i, _) = a.parts();
            lt == layer_type && h == hidden_size && i == input_size
        })
    }

    pub fn layer_type(&self) -> LayerType {
        self.parts().0
    }

    pub fn hidden_size(&self) -> usize {
        self.parts().1
    }

    pub fn input_size(&self) -> usize {
        self.parts().2
    }

    pub fn arch_name(&self) -> &'static str {
        self.parts().3
    }
}

impl fmt::Display for ModelArch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.arch_name())
    }
}
