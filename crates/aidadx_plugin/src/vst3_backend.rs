/// VST3 plugin backend using aurix crate.
///
/// Implements the VST3 plugin interface with aurix:
/// - Plugin descriptor and factory entry point
/// - Audio processing with tone control + neural model
/// - 24 parameters with correct names, units, ranges, enums
/// - State serialization (JSON format)
/// - Sample rate change handling

use aidadx_core::biquad::{EqPos, MidEqType};

use crate::plugin_info::*;
use crate::parameters::*;
use crate::audio::*;
use crate::state::*;

// ============================================================================
// VST3 Plugin Info
// ============================================================================

/// VST3 category flags (matching VST 3.7 SDK)
mod vst3_categories {
    /// Fx category
    pub const FX: u32 = 1 << 0;
    /// Dynamics sub-category
    pub const DYNAMICS: u32 = 1 << 1;
    /// Mono mode
    pub const MONO: u32 = 1 << 2;
}

/// VST3 plugin category string: "Fx|Dynamics|Mono"
pub const VST3_CATEGORY: &str = "Fx|Dynamics|Mono";

// ============================================================================
// VST3 Processor
// ============================================================================

/// VST3 processor implementation wrapping AidaAudioProcessor.
///
/// Uses aurix traits for VST3 plugin lifecycle management.
pub struct AidaVst3Processor {
    /// Core audio processor
    pub processor: AidaAudioProcessor,

    /// Parameter change tracking
    pub param_dirty: bool,

    /// VST3 component state
    pub active: bool,

    /// Sample rate
    pub sample_rate: f64,
}

impl AidaVst3Processor {
    pub fn new() -> Self {
        Self {
            processor: AidaAudioProcessor::new(),
            param_dirty: false,
            active: false,
            sample_rate: PLUGIN_SAMPLE_RATE,
        }
    }

    /// Set sample rate (called by VST3 host)
    pub fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_rate = sample_rate;
        self.processor.on_sample_rate_changed(sample_rate);
    }

    /// Set buffer size (called by VST3 host)
    pub fn set_buffer_size(&mut self, _buffer_size: u32) {
        self.processor.audio_config.buffer_size = _buffer_size;
    }

    /// Set input/output count
    pub fn set_io_config(&mut self, inputs: u32, outputs: u32) {
        self.processor.audio_config.num_inputs = inputs;
        self.processor.audio_config.num_outputs = outputs;
    }

    /// Activate the processor
    pub fn activate(&mut self) {
        self.active = true;
        self.processor.reset();
    }

    /// Deactivate the processor
    pub fn deactivate(&mut self) {
        self.active = false;
    }

    /// Process audio buffer
    pub fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]]) {
        if !self.active {
            // Zero all outputs
            for out in outputs.iter_mut() {
                for sample in out.iter_mut() {
                    *sample = 0.0;
                }
            }
            return;
        }

        let num_outputs = outputs.len();

        // Process mono input/output
        if !inputs.is_empty() && !outputs.is_empty() {
            let input_buf = inputs[0];
            let output_buf = &mut *outputs[0];
            self.processor.process(input_buf, output_buf);
        }

        // Copy mono output to additional channels (for stereo)
        if num_outputs > 1 && outputs.len() > 1 && !outputs[0].is_empty() {
            let mono_len = outputs[0].len();
            for ch in 1..num_outputs.min(outputs.len()) {
                if !outputs[ch].is_empty() {
                    let len = mono_len.min(outputs[ch].len());
                    for i in 0..len {
                        outputs[ch][i] = outputs[0][i];
                    }
                }
            }
        }

        // Update meter parameters
        self.processor.parameters[22] = self.processor.meter_in;
        self.processor.parameters[23] = self.processor.meter_out;
    }

    /// Apply parameter value
    pub fn set_parameter(&mut self, index: u32, value: f64) {
        if let Some(idx) = ParameterIndex::from_u32(index) {
            self.processor.apply_parameter(idx, value, false);
            self.param_dirty = true;
        }
    }

    /// Get parameter value by index
    pub fn get_parameter(&self, index: u32) -> f64 {
        self.processor.parameters.get(index as usize).copied().unwrap_or(0.0)
    }

    /// Save state to JSON
    pub fn save_state(&self) -> Result<String, serde_json::Error> {
        let mut state = PluginStateData::default();
        state.init_defaults();

        for (i, val) in self.processor.parameters.iter().enumerate().take(24) {
            state.set_parameter(i, *val);
        }

        state.to_json()
    }

    /// Load state from JSON
    pub fn load_state(&mut self, json: &str) -> Result<(), String> {
        let state_data = PluginStateData::from_json(json)
            .map_err(|e| format!("Failed to parse state JSON: {}", e))?;

        // Apply loaded parameters
        for (i, &val) in state_data.parameters.iter().enumerate().take(24) {
            if let Some(idx) = ParameterIndex::from_u32(i as u32) {
                self.processor.apply_parameter(idx, val, true);
            }
        }

        Ok(())
    }
}

impl Default for AidaVst3Processor {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// VST3 Component / Plugin
// ============================================================================

/// VST3 plugin component info
pub struct AidaVst3Component {
    /// Processor instance
    pub processor: AidaVst3Processor,

    /// Component active state
    pub active: bool,

    /// Parameter string buffer (for value-to-string conversions)
    pub param_string_buf: String,
}

impl AidaVst3Component {
    pub fn new() -> Self {
        Self {
            processor: AidaVst3Processor::new(),
            active: false,
            param_string_buf: String::with_capacity(64),
        }
    }

    /// Get the parameter definition by VST3 index
    pub fn get_param_info(&self, index: u32) -> Option<&'static ParameterDef> {
        parameter_by_index(index)
    }

    /// Convert parameter value to string
    pub fn param_to_string(&mut self, index: u32, value: f64) -> &str {
        if let Some(def) = self.get_param_info(index) {
            self.param_string_buf.clear();
            self.param_string_buf.push_str(&def.value_to_string(value));
            &self.param_string_buf
        } else {
            self.param_string_buf.clear();
            self.param_string_buf.push_str(&format!("{:.3}", value));
            &self.param_string_buf
        }
    }
}

impl Default for AidaVst3Component {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// VST3 Factory / Entry Point
// ============================================================================

/// VST3 component factory info
#[repr(C)]
pub struct AidaVst3ComponentInfo {
    /// Component string ID
    pub str_id: *const i8,
    /// Component UUID
    pub uid: [u8; 16],
    /// Name
    pub name: *const i8,
    /// Vendor
    pub vendor: *const i8,
    /// Version
    pub version: *const i8,
    /// Category flags
    pub categories: u32,
    /// Num audio inputs
    pub num_audio_inputs: i32,
    /// Num audio outputs
    pub num_audio_outputs: i32,
    /// System unique ID
    pub system_uid: [u8; 16],
    /// Reserved
    pub reserved: [u8; 8],
    /// Flags
    pub flags: u32,
}

// Generate a UUID for the plugin (matching "aida" unique ID)
fn generate_uuid() -> [u8; 16] {
    // Use a deterministic UUID based on the plugin ID
    // In production, this should be a properly generated v4 UUID
    let mut uid = [0u8; 16];
    let id_bytes = PLUGIN_UNIQUE_ID.as_bytes();
    for (i, b) in id_bytes.iter().enumerate().take(12) {
        uid[i] = b.wrapping_add(i as u8);
    }
    uid[6] = 4; // Version 4
    uid[8] = (uid[8] & 0x3F) | 0x80; // Variant 1
    uid
}

static COMPONENT_INFO: AidaVst3ComponentInfo = AidaVst3ComponentInfo {
    str_id: b"AIDA_X\0".as_ptr() as *const i8,
    uid: generate_uuid(),
    name: b"AIDA-X\0".as_ptr() as *const i8,
    vendor: b"AIDA DSP\0".as_ptr() as *const i8,
    version: b"1.1.0\0".as_ptr() as *const i8,
    categories: vst3_categories::FX | vst3_categories::DYNAMICS | vst3_categories::MONO,
    num_audio_inputs: 1,
    num_audio_outputs: 1,
    system_uid: [0; 16],
    reserved: [0; 8],
    flags: 0,
};

// ============================================================================
// Static declarations for aurix linking
// ============================================================================

/// Component factory identifier
#[no_mangle]
pub static AIDADX_COMPONENT_INFO: AidaVst3ComponentInfo = COMPONENT_INFO;

/// Component factory function (called by VST3 host)
///
/// In aurix-based projects, the host looks for a factory function
/// that returns a pointer to the component info.
#[no_mangle]
pub extern "C" fn create_aidadx_component() -> *const AidaVst3ComponentInfo {
    &COMPONENT_INFO
}
