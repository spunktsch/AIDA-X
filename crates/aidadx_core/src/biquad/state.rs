/// AidaToneControl state machine.
///
/// Ported from `AidaToneControl` struct in aidadsp-plugin.cpp.
/// Manages the full EQ chain including all filter types, gain ramps, and state.
use super::cascade::{BiquadCascade, EqPos, MidEqType};
use super::filter::BiquadType;
use super::smoother::{ExponentialValueSmoother, ValueSmoother};

/// Convert dB to linear coefficient, matching C++ DB_CO macro.
#[inline]
pub fn db_to_coeff(g: f64) -> f64 {
    if g > -90.0 {
        10.0_f64.powf(g * 0.05)
    } else {
        0.0
    }
}

/// Map a value from one range to another, matching C++ MAP macro.
#[inline]
pub fn map(x: f64, in_min: f64, in_max: f64, out_min: f64, out_max: f64) -> f64 {
    (x - in_min) * (out_max - out_min) / (in_max - in_min) + out_min
}

/// Biquad filter cascade parameters.
///
/// `Default` yields the C++ plugin defaults (`kParameters[i].ranges.def` in
/// DistrhoPluginCommon.hpp), which the C++ constructor loads before the first
/// `sampleRateChanged()`.
#[derive(Debug, Clone, Copy)]
pub struct CascadeParameters {
    pub in_lpf: f64,
    pub in_level: f64,
    pub net_bypass: f64,
    pub eq_bypass: f64,
    pub eq_pos: f64,
    pub bass_gain: f64,
    pub bass_freq: f64,
    pub mid_gain: f64,
    pub mid_freq: f64,
    pub mid_q: f64,
    pub mid_type: f64,
    pub treble_gain: f64,
    pub treble_freq: f64,
    pub depth: f64,
    pub presence: f64,
    pub out_level: f64,
    pub cab_sim_bypass: f64,
    pub global_bypass: f64,
    pub param1: f64,
    pub param2: f64,
    pub dc_blocker: f64,
}

impl Default for CascadeParameters {
    fn default() -> Self {
        Self {
            in_lpf: 66.216,
            in_level: 0.0,
            net_bypass: 0.0,
            eq_bypass: 0.0,
            eq_pos: 0.0,
            bass_gain: 0.0,
            bass_freq: 305.0,
            mid_gain: 0.0,
            mid_freq: 750.0,
            mid_q: 0.707,
            mid_type: 0.0,
            treble_gain: 0.0,
            treble_freq: 2000.0,
            depth: 0.0,
            presence: 0.0,
            out_level: 0.0,
            cab_sim_bypass: 0.0,
            global_bypass: 0.0,
            param1: 0.0,
            param2: 0.0,
            dc_blocker: 1.0,
        }
    }
}

impl CascadeParameters {
    pub fn apply_to_cascade(&self, cascade: &mut BiquadCascade, sample_rate: f64) {
        cascade.dc_blocker_mut().set_biquad(
            BiquadType::HighPass,
            35.0 / sample_rate,
            COMMON_Q,
            0.0,
        );

        let in_lpf_cutoff = map(self.in_lpf, 0.0, 100.0, INLPF_MAX_CO, INLPF_MIN_CO);
        cascade
            .in_lpf_mut()
            .set_biquad(BiquadType::LowPass, in_lpf_cutoff, COMMON_Q, 0.0);

        cascade.bass_mut().set_biquad(
            BiquadType::LowShelf,
            self.bass_freq / sample_rate,
            COMMON_Q,
            self.bass_gain,
        );

        let mid_type = if self.mid_type > 0.5 {
            BiquadType::BandPass
        } else {
            BiquadType::Peak
        };
        cascade.mid_mut().set_biquad(
            mid_type,
            self.mid_freq / sample_rate,
            self.mid_q,
            self.mid_gain,
        );

        cascade.treble_mut().set_biquad(
            BiquadType::HighShelf,
            self.treble_freq / sample_rate,
            COMMON_Q,
            self.treble_gain,
        );

        cascade.depth_mut().set_biquad(
            BiquadType::Peak,
            DEPTH_FREQ / sample_rate,
            COMMON_Q,
            self.depth,
        );

        cascade.presence_mut().set_biquad(
            BiquadType::HighShelf,
            PRESENCE_FREQ / sample_rate,
            COMMON_Q,
            self.presence,
        );
    }
}

/// Gain ramp smoother for input/output levels.
#[derive(Debug, Clone)]
pub struct GainRamp {
    smoother: ExponentialValueSmoother,
}

impl GainRamp {
    pub fn new() -> Self {
        Self {
            smoother: ExponentialValueSmoother::new(),
        }
    }

    /// Set the T60 time constant in seconds (keeps current and target value).
    pub fn set_time_constant(&mut self, tc: f64) {
        self.smoother.set_time_constant(tc);
    }

    /// Set the sample rate (keeps current and target value, like C++).
    pub fn set_sample_rate(&mut self, sr: f64) {
        self.smoother.set_sample_rate(sr);
    }

    pub fn set_target(&mut self, db: f64) {
        self.smoother.set_target_value(db_to_coeff(db));
    }

    pub fn next(&mut self) -> f64 {
        self.smoother.next()
    }

    pub fn clear_to_target(&mut self) {
        self.smoother.clear_to_target_value();
    }

    pub fn peek(&self) -> f64 {
        self.smoother.peek()
    }
}

impl Default for GainRamp {
    fn default() -> Self {
        Self::new()
    }
}

const COMMON_Q: f64 = 0.707;
const DEPTH_FREQ: f64 = 75.0;
const PRESENCE_FREQ: f64 = 900.0;
const INLPF_MAX_CO: f64 = 0.99 * 0.5;
const INLPF_MIN_CO: f64 = 0.25 * 0.5;

/// AidaToneControl: complete state machine for tone controls.
///
/// Ported from `AidaToneControl` struct in aidadsp-plugin.cpp.
#[derive(Debug, Clone)]
pub struct AidaToneControl {
    pub cascade: BiquadCascade,
    pub inlevel: GainRamp,
    pub outlevel: GainRamp,
    pub net_bypass: bool,
    pub eq_bypass: bool,
    pub eq_pos: EqPos,
    pub mid_type: MidEqType,
    pub dc_blocker_enabled: bool,
    pub input_lpf_enabled: bool,
}

impl AidaToneControl {
    pub fn new() -> Self {
        let mut tc = Self {
            cascade: BiquadCascade::new(),
            inlevel: GainRamp::new(),
            outlevel: GainRamp::new(),
            net_bypass: false,
            eq_bypass: false,
            eq_pos: EqPos::Post,
            mid_type: MidEqType::Peak,
            dc_blocker_enabled: true, // C++: enabledDC = true (aidadsp-plugin.cpp:292)
            input_lpf_enabled: true,
        };

        tc.inlevel.set_time_constant(1.0);
        tc.outlevel.set_time_constant(1.0);
        tc
    }

    pub fn set_sample_rate(&mut self, parameters: &CascadeParameters, sample_rate: f64) {
        parameters.apply_to_cascade(&mut self.cascade, sample_rate);
        self.inlevel.set_sample_rate(sample_rate);
        self.inlevel.set_target(parameters.in_level);
        self.outlevel.set_sample_rate(sample_rate);
        self.outlevel.set_target(parameters.out_level);
    }

    pub fn set_parameter(&mut self, index: u32, value: f64, sample_rate: f64) {
        match index {
            0 => {
                let cutoff = map(value, 0.0, 100.0, INLPF_MAX_CO, INLPF_MIN_CO);
                self.cascade
                    .in_lpf_mut()
                    .set_biquad(BiquadType::LowPass, cutoff, COMMON_Q, 0.0);
                self.input_lpf_enabled = value.abs() > 1e-10;
            }
            1 => {
                self.inlevel.set_target(value);
            }
            2 => {
                self.net_bypass = value > 0.5;
            }
            3 => {
                self.eq_bypass = value > 0.5;
            }
            4 => {
                self.eq_pos = if value > 0.5 { EqPos::Pre } else { EqPos::Post };
            }
            5 => {
                self.cascade.bass_mut().set_peak_gain_db(value);
            }
            6 => {
                self.cascade.bass_mut().set_cutoff(value / sample_rate);
            }
            7 => {
                self.cascade.mid_mut().set_peak_gain_db(value);
            }
            8 => {
                self.cascade.mid_mut().set_cutoff(value / sample_rate);
            }
            9 => {
                self.cascade.mid_mut().set_q(value);
            }
            10 => {
                self.mid_type = if value > 0.5 {
                    MidEqType::Bandpass
                } else {
                    MidEqType::Peak
                };
            }
            11 => {
                self.cascade.treble_mut().set_peak_gain_db(value);
            }
            12 => {
                self.cascade.treble_mut().set_cutoff(value / sample_rate);
            }
            13 => {
                self.cascade.depth_mut().set_peak_gain_db(value);
            }
            14 => {
                self.cascade.presence_mut().set_peak_gain_db(value);
            }
            15 => {
                self.outlevel.set_target(value);
            }
            20 => {
                self.dc_blocker_enabled = value > 0.5;
            }
            _ => {}
        }
    }

    pub fn reset(&mut self) {
        self.cascade.reset();
        self.inlevel.clear_to_target();
        self.outlevel.clear_to_target();
    }

    pub fn process(&mut self, input: &[f32], output: &mut [f32]) {
        let len = input.len().min(output.len());
        if len == 0 {
            return;
        }

        for i in 0..len {
            output[i] = input[i];
        }

        if self.input_lpf_enabled {
            self.cascade
                .in_lpf_mut()
                .process_inplace(&mut output[..len]);
        }

        // Pre-gain: C++ applyGainRamp advances the smoother once per sample.
        for sample in &mut output[..len] {
            *sample *= self.inlevel.next() as f32;
        }

        if !self.eq_bypass && self.eq_pos == EqPos::Pre {
            self.cascade
                .process_eq_pre(&mut output[..len], self.mid_type);
        }

        if self.dc_blocker_enabled {
            self.cascade
                .dc_blocker_mut()
                .process_inplace(&mut output[..len]);
        }

        if !self.eq_bypass && self.eq_pos == EqPos::Post {
            self.cascade
                .process_eq_post(&mut output[..len], self.mid_type);
        }

        // Output volume, per-sample ramp as in C++ applyGainRamp.
        for sample in &mut output[..len] {
            *sample *= self.outlevel.next() as f32;
        }
    }
}

impl Default for AidaToneControl {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_to_coeff() {
        assert!((db_to_coeff(0.0) - 1.0).abs() < 1e-10, "0 dB = 1.0x");
        assert!((db_to_coeff(6.02) - 2.0).abs() < 0.01, "6 dB ~ 2x");
        assert!((db_to_coeff(-6.02) - 0.5).abs() < 0.01, "-6 dB ~ 0.5x");
        assert_eq!(db_to_coeff(-100.0), 0.0, "-100 dB = 0");
    }

    #[test]
    fn test_map() {
        let val = map(50.0, 0.0, 100.0, 1.0, 2.0);
        assert!((val - 1.5).abs() < 1e-10);
    }

    #[test]
    fn test_tone_control_creation() {
        let tc = AidaToneControl::new();
        assert_eq!(tc.eq_pos, EqPos::Post);
        assert_eq!(tc.mid_type, MidEqType::Peak);
        assert!(!tc.eq_bypass);
        assert!(!tc.net_bypass);
        assert!(tc.dc_blocker_enabled);
    }

    #[test]
    fn test_tone_control_set_parameter() {
        let mut tc = AidaToneControl::new();
        tc.set_parameter(4, 1.0, 48000.0);
        assert_eq!(tc.eq_pos, EqPos::Pre);
        tc.set_parameter(4, 0.0, 48000.0);
        assert_eq!(tc.eq_pos, EqPos::Post);
        tc.set_parameter(10, 1.0, 48000.0);
        assert_eq!(tc.mid_type, MidEqType::Bandpass);
    }

    #[test]
    fn test_tone_control_process_zeros() {
        let mut tc = AidaToneControl::new();
        let params = CascadeParameters::default();
        tc.set_sample_rate(&params, 48000.0);

        let input = vec![0.0f32; 64];
        let mut output = vec![0.0f32; 64];
        tc.process(&input, &mut output);

        for v in &output {
            assert_eq!(*v, 0.0);
        }
    }

    #[test]
    fn test_tone_control_process_valid() {
        let mut tc = AidaToneControl::new();
        let mut params = CascadeParameters::default();
        params.in_level = 0.0;
        params.out_level = 0.0;
        tc.set_sample_rate(&params, 48000.0);

        let input = vec![0.1f32; 64];
        let mut output = vec![0.0f32; 64];
        tc.process(&input, &mut output);

        for v in &output {
            assert!(v.is_finite());
        }
    }

    #[test]
    fn test_tone_control_bypass() {
        let mut tc = AidaToneControl::new();
        let mut params = CascadeParameters::default();
        params.in_level = 0.0;
        params.out_level = 0.0;
        tc.set_sample_rate(&params, 48000.0);
        // The host calls activate() before run(); it clears the gain ramps
        // to their targets (aidadsp-plugin.cpp activate()).
        tc.reset();
        tc.eq_bypass = true;
        tc.dc_blocker_enabled = false;
        tc.input_lpf_enabled = false;

        let input = vec![0.5f32; 64];
        let mut output = vec![0.0f32; 64];
        tc.process(&input, &mut output);

        for i in 0..64.min(output.len()) {
            let diff = (input[i] - output[i]).abs();
            assert!(diff < 0.01, "With bypass, output should match input");
        }
    }

    #[test]
    fn test_tone_control_reset() {
        let mut tc = AidaToneControl::new();
        let params = CascadeParameters::default();
        tc.set_sample_rate(&params, 48000.0);
        // activate(): start from the same state that reset() restores.
        tc.reset();

        let input = vec![0.5f32; 64];
        let mut output1 = vec![0.0f32; 64];
        tc.process(&input, &mut output1);

        tc.reset();
        let mut output2 = vec![0.0f32; 64];
        tc.process(&input, &mut output2);

        for i in 0..64 {
            let diff = (output1[i] - output2[i]).abs();
            assert!(diff < 1e-6, "After reset, output should match");
        }
    }

    #[test]
    fn test_gain_ramp_smoother() {
        let mut gr = GainRamp::new();
        gr.set_time_constant(0.1);
        gr.set_sample_rate(48000.0);
        gr.set_target(0.0);
        gr.clear_to_target();

        assert!((gr.peek() - 1.0).abs() < 1e-10, "0 dB should map to 1.0");
    }
}
