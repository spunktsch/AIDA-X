/// Audio processing pipeline matching the C++ `run()` method.
///
/// Process flow (matching C++ aidadsp-plugin.cpp):
/// 1. Input LPF (lowpass filter on input signal)
/// 2. Input gain ramp
/// 3. Pre-EQ tone controls (if eq_bypass=false && eq_pos==Pre)
/// 4. Neural model (if net_bypass=false)
/// 5. DC blocker
/// 6. Cabinet convolution (if cabsim loaded)
/// 7. Post-EQ tone controls (if eq_bypass=false && eq_pos==Post)
/// 8. Output gain ramp
/// 9. Bypass mixing

use aidadx_core::biquad::{AidaToneControl, EqPos, MidEqType};
use aidadx_core::model::loader::DynamicModel;

/// Audio buffer configuration
#[derive(Debug, Clone, Copy)]
pub struct AudioConfig {
    pub sample_rate: f64,
    pub buffer_size: u32,
    pub num_inputs: u32,
    pub num_outputs: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48_000.0,
            buffer_size: 512,
            num_inputs: 1,
            num_outputs: 1,
        }
    }
}

/// The main audio processor, wrapping the core DSP pipeline.
///
/// This is the format-agnostic core that both CLAP and VST3 backends use.
pub struct AidaAudioProcessor {
    /// Tone control state (biquad cascade + gain smoothers)
    pub tone_control: AidaToneControl,

    /// Neural model (loaded dynamically)
    pub model: Option<DynamicModel>,

    /// Cabinet IR state
    pub cabinet_enabled: bool,

    /// Bypass state
    pub bypass_enabled: bool,

    /// Current parameter values (matching C++ parameters[])
    pub parameters: [f64; 24],

    /// Audio configuration
    pub audio_config: AudioConfig,

    /// Peak meter values
    pub meter_in: f64,
    pub meter_out: f64,

    /// Model input size (for output parameter)
    pub model_input_size: u32,
}

impl AidaAudioProcessor {
    pub fn new() -> Self {
        let mut proc = Self {
            tone_control: AidaToneControl::new(),
            model: None,
            cabinet_enabled: false,
            bypass_enabled: true,
            parameters: Default::default(),
            audio_config: AudioConfig::default(),
            meter_in: 0.0,
            meter_out: 0.0,
            model_input_size: 0,
        };

        proc.initialize_defaults();
        proc
    }

    fn initialize_defaults(&mut self) {
        // Initialize parameters to their defaults from DPF
        let params = crate::parameters::all_parameters();
        for (i, p) in params.iter().enumerate() {
            self.parameters[i] = p.default_value;
        }

        // Set sample rate for tone control
        self.tone_control
            .set_sample_rate(&aidadx_core::biquad::CascadeParameters::default(), self.audio_config.sample_rate);

        // Apply default parameters to tone control
        self.apply_parameter(ParameterIndex::Antialiasing, self.parameters[0], true);
        self.apply_parameter(ParameterIndex::InputLevel, self.parameters[1], true);
        self.apply_parameter(ParameterIndex::NetBypass, self.parameters[2], true);
        self.apply_parameter(ParameterIndex::EqBypass, self.parameters[3], true);
        self.apply_parameter(ParameterIndex::EqPos, self.parameters[4], true);
        self.apply_parameter(ParameterIndex::BassGain, self.parameters[5], true);
        self.apply_parameter(ParameterIndex::BassFreq, self.parameters[6], true);
        self.apply_parameter(ParameterIndex::MidGain, self.parameters[7], true);
        self.apply_parameter(ParameterIndex::MidFreq, self.parameters[8], true);
        self.apply_parameter(ParameterIndex::MidQ, self.parameters[9], true);
        self.apply_parameter(ParameterIndex::MidType, self.parameters[10], true);
        self.apply_parameter(ParameterIndex::TrebleGain, self.parameters[11], true);
        self.apply_parameter(ParameterIndex::TrebleFreq, self.parameters[12], true);
        self.apply_parameter(ParameterIndex::Depth, self.parameters[13], true);
        self.apply_parameter(ParameterIndex::Presence, self.parameters[14], true);
        self.apply_parameter(ParameterIndex::OutputLevel, self.parameters[15], true);
        self.apply_parameter(ParameterIndex::CabSimBypass, self.parameters[16], true);
        self.apply_parameter(ParameterIndex::GlobalBypass, self.parameters[17], true);
        self.apply_parameter(ParameterIndex::DcBlocker, self.parameters[20], true);
    }

    /// Set a parameter value and update internal state.
    /// This matches the C++ setParameterValue() method.
    pub fn apply_parameter(
        &mut self,
        index: ParameterIndex,
        value: f64,
        skip_smoother_update: bool,
    ) {
        self.parameters[index as u32 as usize] = value;

        let sr = self.audio_config.sample_rate;

        match index {
            ParameterIndex::Antialiasing => {
                // INLPF: map 0..100 to INLPF_MAX_CO..INLPF_MIN_CO
                let max_co = 0.99 * 0.5;
                let min_co = 0.25 * 0.5;
                let cutoff = value * (min_co - max_co) / 100.0 + max_co;
                self.tone_control.set_parameter(0, value, sr);
                self.tone_control.input_lpf_enabled = value.abs() > 1e-10;
            }
            ParameterIndex::InputLevel => {
                self.tone_control.set_parameter(1, value, sr);
            }
            ParameterIndex::NetBypass => {
                self.tone_control.set_parameter(2, value, sr);
            }
            ParameterIndex::EqBypass => {
                self.tone_control.set_parameter(3, value, sr);
            }
            ParameterIndex::EqPos => {
                self.tone_control.set_parameter(4, value, sr);
            }
            ParameterIndex::BassGain => {
                self.tone_control.set_parameter(5, value, sr);
            }
            ParameterIndex::BassFreq => {
                self.tone_control.set_parameter(6, value, sr);
            }
            ParameterIndex::MidGain => {
                self.tone_control.set_parameter(7, value, sr);
            }
            ParameterIndex::MidFreq => {
                self.tone_control.set_parameter(8, value, sr);
            }
            ParameterIndex::MidQ => {
                self.tone_control.set_parameter(9, value, sr);
            }
            ParameterIndex::MidType => {
                self.tone_control.set_parameter(10, value, sr);
            }
            ParameterIndex::TrebleGain => {
                self.tone_control.set_parameter(11, value, sr);
            }
            ParameterIndex::TrebleFreq => {
                self.tone_control.set_parameter(12, value, sr);
            }
            ParameterIndex::Depth => {
                self.tone_control.set_parameter(13, value, sr);
            }
            ParameterIndex::Presence => {
                self.tone_control.set_parameter(14, value, sr);
            }
            ParameterIndex::OutputLevel => {
                self.tone_control.set_parameter(15, value, sr);
            }
            ParameterIndex::CabSimBypass => {
                // cab_sim_bypass=1 means bypass cabinet (gain=0)
                // cab_sim_bypass=0 means cabinet on (gain=kCabinetMaxGain=0.251)
                self.cabinet_enabled = value < 0.5;
            }
            ParameterIndex::GlobalBypass => {
                self.bypass_enabled = value > 0.5;
            }
            ParameterIndex::DcBlocker => {
                self.tone_control.set_parameter(20, value, sr);
            }
            _ => {}
        }

        if !skip_smoother_update {
            // Update model parameters 1 and 2
            if let Some(ref model) = self.model {
                let input_size = model.arch.input_size();
                match input_size {
                    2 => {
                        // param1 would be tracked via smoothers in full impl
                        let _ = model;
                    }
                    3 => {
                        let _ = model;
                    }
                    _ => {}
                }
            }
        }
    }

    /// Handle sample rate change
    pub fn on_sample_rate_changed(&mut self, sample_rate: f64) {
        self.audio_config.sample_rate = sample_rate;
        let params = aidadx_core::biquad::CascadeParameters {
            in_lpf: self.parameters[0],
            in_level: self.parameters[1],
            net_bypass: self.parameters[2],
            eq_bypass: self.parameters[3],
            eq_pos: self.parameters[4],
            bass_gain: self.parameters[5],
            bass_freq: self.parameters[6],
            mid_gain: self.parameters[7],
            mid_freq: self.parameters[8],
            mid_q: self.parameters[9],
            mid_type: self.parameters[10],
            treble_gain: self.parameters[11],
            treble_freq: self.parameters[12],
            depth: self.parameters[13],
            presence: self.parameters[14],
            out_level: self.parameters[15],
            cab_sim_bypass: self.parameters[16],
            global_bypass: self.parameters[17],
            param1: self.parameters[18],
            param2: self.parameters[19],
            dc_blocker: self.parameters[20],
        };
        self.tone_control.set_sample_rate(&params, sample_rate);
    }

    /// Process audio through the complete pipeline
    pub fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
    ) {
        let len = input.len().min(output.len());
        if len == 0 {
            return;
        }

        // Copy input to output as starting point (for bypass)
        for i in 0..len {
            output[i] = input[i];
        }

        // Track input meter (peak)
        let mut peak_in = 0.0f64;
        for &s in &input[..len] {
            let abs = s.abs() as f64;
            if abs > peak_in {
                peak_in = abs;
            }
        }
        self.meter_in = peak_in;

        // If bypass is enabled, skip processing and just pass through
        if self.bypass_enabled {
            // Update output meter
            for i in 0..len {
                let abs = output[i].abs() as f64;
                if abs > self.meter_out {
                    self.meter_out = abs;
                }
            }
            return;
        }

        // 1. Input LPF + Input gain ramp + EQ section
        //    (handled by tone_control.process which includes DC blocker at end)
        //    We need to split the tone control processing to interleave model/cab

        // 1a. Input LPF
        if self.tone_control.input_lpf_enabled {
            self.tone_control
                .cascade
                .in_lpf_mut()
                .process_inplace(&mut output[..len]);
        }

        // 1b. Input gain ramp
        let in_gain = self.tone_control.inlevel.next();
        for i in 0..len {
            output[i] *= in_gain as f32;
        }

        // 2. Pre-EQ (before neural model)
        if !self.tone_control.eq_bypass && self.tone_control.eq_pos == EqPos::Pre {
            self.tone_control
                .cascade
                .process_eq_pre(&mut output[..len], self.tone_control.mid_type);
        }

        // 3. Neural model processing
        if let Some(ref model) = self.model {
            let input_size = model.arch.input_size();
            let has_skip = model.input_skip;
            let in_gain = model.input_gain;
            let out_gain = model.output_gain;

            // Apply input gain to first input_size samples
            if (in_gain - 1.0).abs() > 1e-6 {
                for i in 0..input_size.min(len) {
                    output[i] *= in_gain;
                }
            }

            // Forward through model
            for i in 0..len {
                // Prepare input array
                let mut model_input = [0.0f32; 3];
                let end = input_size.min(len - i).min(3);

                for j in 0..end {
                    model_input[j] = output[i + j];
                }

                // For now, skip param1/param2 smoothing in the simple impl
                let param1 = self.parameters[18] as f32;
                let param2 = self.parameters[19] as f32;

                // Forward
                let output_val = match model.inference.forward(&mut model_input[..input_size], param1, param2) {
                    Ok(out) => out[0],
                    Err(_) => 0.0,
                };

                if has_skip {
                    output[i] += output_val * (out_gain as f32);
                } else {
                    output[i] = output_val * (out_gain as f32);
                }
            }
        }

        // 4. DC blocker
        if self.tone_control.dc_blocker_enabled {
            self.tone_control
                .cascade
                .dc_blocker_mut()
                .process_inplace(&mut output[..len]);
        }

        // 5. Cabinet convolution (placeholder - actual convolution needs FFTConvolver)
        if self.cabinet_enabled {
            // Placeholder: apply cabinet gain compensation
            // Actual convolution would use FFTConvolver from C++ modules
            const CAB_GAIN: f32 = 0.251; // kCabinetMaxGain = -12dB
            for i in 0..len {
                output[i] *= CAB_GAIN;
            }
        }

        // 6. Post-EQ (after neural model)
        if !self.tone_control.eq_bypass && self.tone_control.eq_pos == EqPos::Post {
            self.tone_control
                .cascade
                .process_eq_post(&mut output[..len], self.tone_control.mid_type);
        }

        // 7. Output gain ramp
        let out_gain = self.tone_control.outlevel.next();
        for i in 0..len {
            output[i] *= out_gain as f32;
        }

        // 8. Bypass mixing
        // (In the simple impl, bypass is handled at the top of the function)

        // Update output meter
        for i in 0..len {
            let abs = output[i].abs() as f64;
            if abs > self.meter_out {
                self.meter_out = abs;
            }
        }

        // Update output parameters
        self.parameters[21] = self.model_input_size as f64;
        self.parameters[22] = self.meter_in;
        self.parameters[23] = self.meter_out;
    }

    /// Set the neural model
    pub fn set_model(&mut self, model: DynamicModel) {
        model.reset();
        let input_size = model.arch.input_size();
        self.model_input_size = input_size as u32;
        self.model = Some(model);
    }

    /// Reset the processor state (called on activate)
    pub fn reset(&mut self) {
        self.tone_control.reset();
        if let Some(ref mut model) = self.model {
            model.reset();
        }
        self.meter_in = 0.0;
        self.meter_out = 0.0;
    }
}

impl Default for AidaAudioProcessor {
    fn default() -> Self {
        Self::new()
    }
}
