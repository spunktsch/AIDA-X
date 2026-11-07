/// Plugin state serialization/deserialization (JSON format).
///
/// Matches the DPF state format for compatibility:
///   - Parameter values
///   - Model file path
///   - Cabinet IR file path
///   - Audio file path (standalone only)
///   - Meter reset command

use serde::{Deserialize, Serialize};

/// State container matching DPF kState definitions:
///   kStateModelFile, kStateImpulseFile, kStateAudioFile (standalone)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStateData {
    /// Parameter values (24 values matching DPF parameter array)
    #[serde(default)]
    pub parameters: Vec<f64>,

    /// Neural model file path
    #[serde(default)]
    pub model_file: Option<String>,

    /// Cabinet impulse response file path
    #[serde(default)]
    pub cabinet_file: Option<String>,

    /// Audio loop file path (standalone only)
    #[serde(default)]
    pub audio_file: Option<String>,
}

impl Default for PluginStateData {
    fn default() -> Self {
        Self {
            parameters: vec![0.0; 24],
            model_file: None,
            cabinet_file: None,
            audio_file: None,
        }
    }
}

impl PluginStateData {
    /// Serialize state to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize state from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Initialize parameter defaults matching DPF
    pub fn init_defaults(&mut self) {
        let params = crate::parameters::all_parameters();
        self.parameters.clear();
        for p in params.iter() {
            self.parameters.push(p.default_value);
        }
    }

    /// Get parameter value by index
    pub fn get_parameter(&self, index: usize) -> f64 {
        self.parameters.get(index).copied().unwrap_or(0.0)
    }

    /// Set parameter value by index
    pub fn set_parameter(&mut self, index: usize, value: f64) {
        while self.parameters.len() <= index {
            self.parameters.push(0.0);
        }
        self.parameters[index] = value;
    }
}

/// State key constants matching DPF state keys
pub mod state_keys {
    pub const MODEL_FILE: &str = "json";
    pub const CABINET_FILE: &str = "cabinet";
    pub const AUDIO_FILE: &str = "audiofile";
    pub const RESET_METERS: &str = "reset-meters";
    pub const PARAMETER_PREFIX: &str = "param:";
}
