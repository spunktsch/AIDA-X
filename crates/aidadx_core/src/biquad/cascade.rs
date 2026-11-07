use super::filter::{Biquad, BiquadType};

/// A cascade of biquad filters, matching the C++ filter chain:
/// dc_blocker -> in_lpf -> bass -> mid -> treble -> depth -> presence
///
/// Ported from `AidaToneControl` in aidadsp-plugin.cpp.
#[derive(Debug, Clone)]
pub struct BiquadCascade {
    dc_blocker: Biquad,
    in_lpf: Biquad,
    bass: Biquad,
    mid: Biquad,
    treble: Biquad,
    depth: Biquad,
    presence: Biquad,
}

impl BiquadCascade {
    pub fn new() -> Self {
        Self {
            dc_blocker: Biquad::new(),
            in_lpf: Biquad::new(),
            bass: Biquad::new(),
            mid: Biquad::new(),
            treble: Biquad::new(),
            depth: Biquad::new(),
            presence: Biquad::new(),
        }
    }

    pub fn reset(&mut self) {
        self.dc_blocker.reset();
        self.in_lpf.reset();
        self.bass.reset();
        self.mid.reset();
        self.treble.reset();
        self.depth.reset();
        self.presence.reset();
    }

    /// Initialize all filters with default parameters.
    pub fn init_defaults(&mut self, sample_rate: f64) {
        let dc_cutoff = 35.0 / sample_rate;
        self.dc_blocker
            .set_biquad(BiquadType::HighPass, dc_cutoff, COMMON_Q, 0.0);

        self.in_lpf
            .set_biquad(BiquadType::LowPass, 0.5, COMMON_Q, 0.0);

        self.bass
            .set_biquad(BiquadType::LowShelf, DEFAULT_BASS_FREQ, COMMON_Q, 0.0);

        self.mid
            .set_biquad(BiquadType::Peak, DEFAULT_MID_FREQ, 1.0, 0.0);

        self.treble
            .set_biquad(BiquadType::HighShelf, DEFAULT_TREBLE_FREQ, COMMON_Q, 0.0);

        self.depth
            .set_biquad(BiquadType::Peak, DEPTH_FREQ / sample_rate, COMMON_Q, 0.0);

        self.presence.set_biquad(
            BiquadType::HighShelf,
            PRESENCE_FREQ / sample_rate,
            COMMON_Q,
            0.0,
        );
    }

    /// Process a buffer through the cascade.
    pub fn process(&mut self, input: &[f32], output: &mut [f32]) {
        let len = input.len().min(output.len());
        if len == 0 {
            return;
        }

        for i in 0..len {
            output[i] = input[i];
        }

        self.dc_blocker.process_inplace(&mut output[..len]);
    }

    /// Process in the pre-model EQ position.
    /// When mid_type == Bandpass, only mid is applied.
    /// Otherwise: depth -> bass -> mid -> treble -> presence
    pub fn process_eq_pre(&mut self, buffer: &mut [f32], mid_type: MidEqType) {
        if buffer.is_empty() {
            return;
        }

        if mid_type == MidEqType::Bandpass {
            self.mid.process_inplace(buffer);
        } else {
            self.depth.process_inplace(buffer);
            self.bass.process_inplace(buffer);
            self.mid.process_inplace(buffer);
            self.treble.process_inplace(buffer);
            self.presence.process_inplace(buffer);
        }
    }

    /// Process in the post-model EQ position.
    pub fn process_eq_post(&mut self, buffer: &mut [f32], mid_type: MidEqType) {
        self.process_eq_pre(buffer, mid_type);
    }

    pub fn dc_blocker(&mut self) -> &mut Biquad {
        &mut self.dc_blocker
    }

    pub fn in_lpf(&mut self) -> &mut Biquad {
        &mut self.in_lpf
    }

    pub fn bass(&mut self) -> &mut Biquad {
        &mut self.bass
    }

    pub fn mid(&mut self) -> &mut Biquad {
        &mut self.mid
    }

    pub fn treble(&mut self) -> &mut Biquad {
        &mut self.treble
    }

    pub fn depth(&mut self) -> &mut Biquad {
        &mut self.depth
    }

    pub fn presence(&mut self) -> &mut Biquad {
        &mut self.presence
    }

    pub fn dc_blocker_mut(&mut self) -> &mut Biquad {
        &mut self.dc_blocker
    }

    pub fn in_lpf_mut(&mut self) -> &mut Biquad {
        &mut self.in_lpf
    }

    pub fn bass_mut(&mut self) -> &mut Biquad {
        &mut self.bass
    }

    pub fn mid_mut(&mut self) -> &mut Biquad {
        &mut self.mid
    }

    pub fn treble_mut(&mut self) -> &mut Biquad {
        &mut self.treble
    }

    pub fn depth_mut(&mut self) -> &mut Biquad {
        &mut self.depth
    }

    pub fn presence_mut(&mut self) -> &mut Biquad {
        &mut self.presence
    }
}

impl Default for BiquadCascade {
    fn default() -> Self {
        Self::new()
    }
}

/// Mid-equalizer type matching C++ MidEqType enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MidEqType {
    #[default]
    Peak,
    Bandpass,
}

/// EQ position matching C++ EqPos enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EqPos {
    #[default]
    Pre,
    Post,
}

/// Parameter indices matching the C++ plugin parameters
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ParameterIndex {
    INLPF = 0,
    INLEVEL = 1,
    NETBYPASS = 2,
    EQBYPASS = 3,
    EQPOS = 4,
    BASSGAIN = 5,
    BASSFREQ = 6,
    MIDGAIN = 7,
    MIDFREQ = 8,
    MIDQ = 9,
    MTYPE = 10,
    TREBLEGAIN = 11,
    TREBLEFREQ = 12,
    DEPTH = 13,
    PRESENCE = 14,
    OUTLEVEL = 15,
    CABSIMBYPASS = 16,
    GLOBALBYPASS = 17,
    PARAM1 = 18,
    PARAM2 = 19,
    DCBLOCKER = 20,
    ModelInputSize = 21,
    MeterIn = 22,
    MeterOut = 23,
}

const COMMON_Q: f64 = 0.707;
const DEPTH_FREQ: f64 = 75.0;
const PRESENCE_FREQ: f64 = 900.0;
const DEFAULT_BASS_FREQ: f64 = 0.2;
const DEFAULT_MID_FREQ: f64 = 0.3;
const DEFAULT_TREBLE_FREQ: f64 = 0.5;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cascade_creation() {
        let cascade = BiquadCascade::new();
    }

    #[test]
    fn test_cascade_init_defaults() {
        let mut cascade = BiquadCascade::new();
        cascade.init_defaults(48000.0);

        assert_eq!(
            cascade.dc_blocker().params().filter_type,
            BiquadType::HighPass
        );
        assert_eq!(cascade.in_lpf().params().filter_type, BiquadType::LowPass);
        assert_eq!(cascade.bass().params().filter_type, BiquadType::LowShelf);
        assert_eq!(cascade.mid().params().filter_type, BiquadType::Peak);
        assert_eq!(cascade.treble().params().filter_type, BiquadType::HighShelf);
        assert_eq!(cascade.depth().params().filter_type, BiquadType::Peak);
        assert_eq!(
            cascade.presence().params().filter_type,
            BiquadType::HighShelf
        );
    }

    #[test]
    fn test_cascade_process_zeros() {
        let mut cascade = BiquadCascade::new();
        cascade.init_defaults(48000.0);

        let input = vec![0.0f32; 64];
        let mut output = vec![0.0f32; 64];
        cascade.process(&input, &mut output);

        for &v in &output {
            assert_eq!(v, 0.0, "Expected zero output for zero input, got {}", v);
        }
    }

    #[test]
    fn test_cascade_process_identity() {
        let mut cascade = BiquadCascade::new();
        cascade.init_defaults(48000.0);

        let input = vec![1.0f32; 100];
        let mut output = vec![0.0f32; 100];
        cascade.process(&input, &mut output);

        for v in &output {
            assert!(v.is_finite(), "Output should be finite");
        }
    }

    #[test]
    fn test_eq_pre_bandpass_mode() {
        let mut cascade = BiquadCascade::new();
        cascade.init_defaults(48000.0);

        let mut buffer = vec![0.5f32; 64];
        cascade.process_eq_pre(&mut buffer, MidEqType::Bandpass);

        for v in &buffer {
            assert!(v.is_finite(), "Output should be finite in bandpass mode");
        }
    }

    #[test]
    fn test_eq_pre_peak_mode() {
        let mut cascade = BiquadCascade::new();
        cascade.init_defaults(48000.0);

        let mut buffer = vec![0.5f32; 64];
        cascade.process_eq_pre(&mut buffer, MidEqType::Peak);

        for v in &buffer {
            assert!(v.is_finite(), "Output should be finite in peak mode");
        }
    }

    #[test]
    fn test_eq_post_matches_eq_pre() {
        let mut cascade_pre = BiquadCascade::new();
        let mut cascade_post = BiquadCascade::new();
        cascade_pre.init_defaults(48000.0);
        cascade_post.init_defaults(48000.0);

        let mut buffer = vec![0.5f32; 64];
        cascade_pre.process_eq_pre(&mut buffer, MidEqType::Peak);
        let pre_result = buffer.clone();

        let mut buffer2 = vec![0.5f32; 64];
        cascade_post.process_eq_post(&mut buffer2, MidEqType::Peak);

        assert_eq!(
            pre_result, buffer2,
            "eq_pre and eq_post should produce same results"
        );
    }

    #[test]
    fn test_cascade_reset() {
        let mut cascade = BiquadCascade::new();
        cascade.init_defaults(48000.0);

        let input = vec![1.0f32; 64];
        let mut output = vec![0.0f32; 64];
        cascade.process(&input, &mut output);

        cascade.reset();

        let mut output2 = vec![0.0f32; 64];
        cascade.process(&input, &mut output2);

        for i in 0..64 {
            let diff = (output[i] - output2[i]).abs();
            assert!(
                diff < 1e-6,
                "After reset, output should match: diff={}",
                diff
            );
        }
    }

    #[test]
    fn test_mid_eq_type_enum() {
        assert_eq!(MidEqType::Peak as u8, 0);
        assert_eq!(MidEqType::Bandpass as u8, 1);
    }

    #[test]
    fn test_eq_pos_enum() {
        assert_eq!(EqPos::Pre as u8, 0);
        assert_eq!(EqPos::Post as u8, 1);
    }

    #[test]
    fn test_parameter_indices() {
        assert_eq!(ParameterIndex::INLPF as u32, 0);
        assert_eq!(ParameterIndex::MeterOut as u32, 23);
    }
}
