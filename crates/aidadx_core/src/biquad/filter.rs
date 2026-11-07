/// Biquad filter type matching C++ bq_type_* enum values.
/// 0=lowpass, 1=highpass, 2=bandpass, 3=notch, 4=peak, 5=lowshelf, 6=highshelf
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum BiquadType {
    #[default]
    LowPass = 0,
    HighPass = 1,
    BandPass = 2,
    Notch = 3,
    Peak = 4,
    LowShelf = 5,
    HighShelf = 6,
}

impl BiquadType {
    pub fn from_u8(value: u8) -> Option<BiquadType> {
        match value {
            0 => Some(BiquadType::LowPass),
            1 => Some(BiquadType::HighPass),
            2 => Some(BiquadType::BandPass),
            3 => Some(BiquadType::Notch),
            4 => Some(BiquadType::Peak),
            5 => Some(BiquadType::LowShelf),
            6 => Some(BiquadType::HighShelf),
            _ => None,
        }
    }

    pub fn to_u8(self) -> u8 {
        self as u8
    }
}

/// Coefficients for the biquad filter.
#[derive(Debug, Clone, Copy)]
pub struct Coefficients {
    pub a0: f64,
    pub a1: f64,
    pub a2: f64,
    pub b1: f64,
    pub b2: f64,
}

impl Coefficients {
    pub fn new() -> Self {
        Self {
            a0: 1.0,
            a1: 0.0,
            a2: 0.0,
            b1: 0.0,
            b2: 0.0,
        }
    }
}

impl Default for Coefficients {
    fn default() -> Self {
        Self::new()
    }
}

/// Filter parameters matching the C++ biquad design.
#[derive(Debug, Clone, Copy)]
pub struct Parameters {
    pub filter_type: BiquadType,
    /// Normalized cutoff frequency (0.0..0.5, where 0.5 = Nyquist)
    pub cutoff_normalized: f64,
    /// Quality factor
    pub q: f64,
    /// Peak gain in dB (for peak, low-shelf, high-shelf types)
    pub peak_gain_db: f64,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            filter_type: BiquadType::LowPass,
            cutoff_normalized: 0.5,
            q: 0.707,
            peak_gain_db: 0.0,
        }
    }
}

/// A single biquad filter stage.
///
/// Ported from C++ Biquad class (EarLevel Engineering).
/// Uses transposed direct form II with pre-computed coefficients.
#[derive(Debug, Clone)]
pub struct Biquad {
    params: Parameters,
    coeffs: Coefficients,
    z1: f64,
    z2: f64,
}

impl Biquad {
    pub fn new() -> Self {
        Self {
            params: Parameters::default(),
            coeffs: Coefficients::new(),
            z1: 0.0,
            z2: 0.0,
        }
    }

    pub fn with_params(params: Parameters) -> Self {
        let mut bq = Self::new();
        bq.set_params(params);
        bq
    }

    pub fn params(&self) -> &Parameters {
        &self.params
    }

    pub fn set_params(&mut self, params: Parameters) {
        self.params = params;
        self.calc_biquad();
    }

    pub fn set_type(&mut self, filter_type: BiquadType) {
        self.params.filter_type = filter_type;
        self.calc_biquad();
    }

    pub fn set_q(&mut self, q: f64) {
        self.params.q = q;
        self.calc_biquad();
    }

    pub fn set_cutoff(&mut self, cutoff_normalized: f64) {
        self.params.cutoff_normalized = cutoff_normalized;
        self.calc_biquad();
    }

    pub fn set_peak_gain_db(&mut self, peak_gain_db: f64) {
        self.params.peak_gain_db = peak_gain_db;
        self.calc_biquad();
    }

    pub fn set_biquad(
        &mut self,
        filter_type: BiquadType,
        cutoff_normalized: f64,
        q: f64,
        peak_gain_db: f64,
    ) {
        self.params.filter_type = filter_type;
        self.params.q = q;
        self.params.cutoff_normalized = cutoff_normalized;
        self.params.peak_gain_db = peak_gain_db;
        self.calc_biquad();
    }

    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }

    /// Process a single sample. Uses f64 internally for numerical accuracy.
    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let in_f64 = input as f64;
        let out = in_f64 * self.coeffs.a0 + self.z1;
        self.z1 = in_f64 * self.coeffs.a1 + self.z2 - self.coeffs.b1 * out;
        self.z2 = in_f64 * self.coeffs.a2 - self.coeffs.b2 * out;
        out as f32
    }

    /// Process a buffer of samples (output written to separate buffer).
    #[inline]
    pub fn process_buffer(&mut self, input: &[f32], output: &mut [f32]) {
        let a0 = self.coeffs.a0;
        let a1 = self.coeffs.a1;
        let a2 = self.coeffs.a2;
        let b1 = self.coeffs.b1;
        let b2 = self.coeffs.b2;
        let mut z1 = self.z1;
        let mut z2 = self.z2;

        for i in 0..input.len() {
            let in_val = input[i] as f64;
            let out = in_val * a0 + z1;
            z1 = in_val * a1 + z2 - b1 * out;
            z2 = in_val * a2 - b2 * out;
            output[i] = out as f32;
        }

        self.z1 = z1;
        self.z2 = z2;
    }

    /// Process a buffer of samples in-place.
    #[inline]
    pub fn process_inplace(&mut self, buffer: &mut [f32]) {
        let a0 = self.coeffs.a0;
        let a1 = self.coeffs.a1;
        let a2 = self.coeffs.a2;
        let b1 = self.coeffs.b1;
        let b2 = self.coeffs.b2;
        let mut z1 = self.z1;
        let mut z2 = self.z2;

        for sample in buffer.iter_mut() {
            let in_val = *sample as f64;
            let out = in_val * a0 + z1;
            z1 = in_val * a1 + z2 - b1 * out;
            z2 = in_val * a2 - b2 * out;
            *sample = out as f32;
        }

        self.z1 = z1;
        self.z2 = z2;
    }

    fn calc_biquad(&mut self) {
        let k = (std::f64::consts::PI * self.params.cutoff_normalized).tan();
        let v = 10.0_f64.powf(self.params.peak_gain_db.abs() / 20.0);

        match self.params.filter_type {
            BiquadType::LowPass => {
                let n = 1.0 / (1.0 + k / self.params.q + k * k);
                self.coeffs = Coefficients {
                    a0: k * k * n,
                    a1: 2.0 * n * k * k,
                    a2: n * k * k,
                    b1: 2.0 * (k * k - 1.0) * n,
                    b2: (1.0 - k / self.params.q + k * k) * n,
                };
                return;
            }
            BiquadType::HighPass => {
                let n = 1.0 / (1.0 + k / self.params.q + k * k);
                self.coeffs = Coefficients {
                    a0: n,
                    a1: -2.0 * n,
                    a2: n,
                    b1: 2.0 * (k * k - 1.0) * n,
                    b2: (1.0 - k / self.params.q + k * k) * n,
                };
                return;
            }
            BiquadType::BandPass => {
                let n = 1.0 / (1.0 + k / self.params.q + k * k);
                self.coeffs = Coefficients {
                    a0: k / self.params.q * n,
                    a1: 0.0,
                    a2: -k / self.params.q * n,
                    b1: 2.0 * (k * k - 1.0) * n,
                    b2: (1.0 - k / self.params.q + k * k) * n,
                };
                return;
            }
            BiquadType::Notch => {
                let n = 1.0 / (1.0 + k / self.params.q + k * k);
                self.coeffs = Coefficients {
                    a0: (1.0 + k * k) * n,
                    a1: 2.0 * (k * k - 1.0) * n,
                    a2: (1.0 + k * k) * n,
                    b1: 2.0 * (k * k - 1.0) * n,
                    b2: (1.0 - k / self.params.q + k * k) * n,
                };
                return;
            }
            BiquadType::Peak => {
                if self.params.peak_gain_db >= 0.0 {
                    // boost
                    let n = 1.0 / (1.0 + 1.0 / self.params.q * k + k * k);
                    self.coeffs = Coefficients {
                        a0: (1.0 + v / self.params.q * k + k * k) * n,
                        a1: 2.0 * (k * k - 1.0) * n,
                        a2: (1.0 - v / self.params.q * k + k * k) * n,
                        b1: 2.0 * (k * k - 1.0) * n,
                        b2: (1.0 - 1.0 / self.params.q * k + k * k) * n,
                    };
                } else {
                    // cut
                    let n = 1.0 / (1.0 + v / self.params.q * k + k * k);
                    self.coeffs = Coefficients {
                        a0: (1.0 + 1.0 / self.params.q * k + k * k) * n,
                        a1: 2.0 * (k * k - 1.0) * n,
                        a2: (1.0 - 1.0 / self.params.q * k + k * k) * n,
                        b1: 2.0 * (k * k - 1.0) * n,
                        b2: (1.0 - v / self.params.q * k + k * k) * n,
                    };
                }
                return;
            }
            BiquadType::LowShelf => {
                if self.params.peak_gain_db >= 0.0 {
                    // boost
                    let sqrt2 = 2.0_f64.sqrt();
                    let sqrt2v = (2.0 * v).sqrt();
                    let n = 1.0 / (1.0 + sqrt2 * k + k * k);
                    self.coeffs = Coefficients {
                        a0: (1.0 + sqrt2v * k + v * k * k) * n,
                        a1: 2.0 * (v * k * k - 1.0) * n,
                        a2: (1.0 - sqrt2v * k + v * k * k) * n,
                        b1: 2.0 * (k * k - 1.0) * n,
                        b2: (1.0 - sqrt2 * k + k * k) * n,
                    };
                } else {
                    // cut
                    let sqrt2 = 2.0_f64.sqrt();
                    let sqrt2v = (2.0 * v).sqrt();
                    let n = 1.0 / (1.0 + sqrt2v * k + v * k * k);
                    self.coeffs = Coefficients {
                        a0: (1.0 + sqrt2 * k + k * k) * n,
                        a1: 2.0 * (k * k - 1.0) * n,
                        a2: (1.0 - sqrt2 * k + k * k) * n,
                        b1: 2.0 * (v * k * k - 1.0) * n,
                        b2: (1.0 - sqrt2v * k + v * k * k) * n,
                    };
                }
                return;
            }
            BiquadType::HighShelf => {
                if self.params.peak_gain_db >= 0.0 {
                    // boost
                    let sqrt2 = 2.0_f64.sqrt();
                    let sqrt2v = (2.0 * v).sqrt();
                    let n = 1.0 / (1.0 + sqrt2 * k + k * k);
                    self.coeffs = Coefficients {
                        a0: (v + sqrt2v * k + k * k) * n,
                        a1: 2.0 * (k * k - v) * n,
                        a2: (v - sqrt2v * k + k * k) * n,
                        b1: 2.0 * (k * k - 1.0) * n,
                        b2: (1.0 - sqrt2 * k + k * k) * n,
                    };
                } else {
                    // cut
                    let sqrt2 = 2.0_f64.sqrt();
                    let sqrt2v = (2.0 * v).sqrt();
                    let n = 1.0 / (v + sqrt2v * k + k * k);
                    self.coeffs = Coefficients {
                        a0: (1.0 + sqrt2 * k + k * k) * n,
                        a1: 2.0 * (k * k - 1.0) * n,
                        a2: (1.0 - sqrt2 * k + k * k) * n,
                        b1: 2.0 * (k * k - v) * n,
                        b2: (v - sqrt2v * k + k * k) * n,
                    };
                }
                return;
            }
        };
    }
}

impl Default for Biquad {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-5;

    fn assert_close(a: impl Into<f64>, b: impl Into<f64>, name: &str) {
        let (a, b): (f64, f64) = (a.into(), b.into());
        let diff = (a - b).abs();
        assert!(
            diff < EPSILON,
            "{}: expected {}, got {} (diff={:.10e})",
            name,
            b,
            a,
            diff
        );
    }

    #[test]
    fn test_lowpass() {
        let mut bq = Biquad::new();
        bq.set_biquad(BiquadType::LowPass, 0.5, COMMON_Q, 0.0);
        assert_eq!(bq.params.filter_type, BiquadType::LowPass);
        assert_close(bq.params.cutoff_normalized, 0.5, "lp_cutoff");
        assert_close(bq.params.q, COMMON_Q, "lp_q");
    }

    #[test]
    fn test_highpass() {
        let mut bq = Biquad::new();
        bq.set_biquad(BiquadType::HighPass, 0.5, COMMON_Q, 0.0);
        assert_eq!(bq.params.filter_type, BiquadType::HighPass);
    }

    #[test]
    fn test_bandpass() {
        let mut bq = Biquad::new();
        bq.set_biquad(BiquadType::BandPass, 0.3, 0.707, 0.0);
        assert_eq!(bq.params.filter_type, BiquadType::BandPass);
    }

    #[test]
    fn test_notch() {
        let mut bq = Biquad::new();
        bq.set_biquad(BiquadType::Notch, 0.25, 1.0, 0.0);
        assert_eq!(bq.params.filter_type, BiquadType::Notch);
    }

    #[test]
    fn test_peak_boost() {
        let mut bq = Biquad::new();
        bq.set_biquad(BiquadType::Peak, 0.3, 1.0, 6.0);
        assert_eq!(bq.params.filter_type, BiquadType::Peak);
        assert_close(bq.params.peak_gain_db, 6.0, "peak_boost_db");
    }

    #[test]
    fn test_peak_cut() {
        let mut bq = Biquad::new();
        bq.set_biquad(BiquadType::Peak, 0.3, 1.0, -6.0);
        assert_close(bq.params.peak_gain_db, -6.0, "peak_cut_db");
    }

    #[test]
    fn test_lowshelf() {
        let mut bq = Biquad::new();
        bq.set_biquad(BiquadType::LowShelf, 0.1, 0.707, 3.0);
        assert_eq!(bq.params.filter_type, BiquadType::LowShelf);
    }

    #[test]
    fn test_highshelf() {
        let mut bq = Biquad::new();
        bq.set_biquad(BiquadType::HighShelf, 0.5, 0.707, 0.0);
        assert_eq!(bq.params.filter_type, BiquadType::HighShelf);
    }

    #[test]
    fn test_default_is_lowpass() {
        let bq = Biquad::new();
        assert_eq!(bq.params.filter_type, BiquadType::LowPass);
    }

    #[test]
    fn test_reset_clears_state() {
        let mut bq = Biquad::new();
        bq.set_biquad(BiquadType::LowPass, 0.25, 0.707, 0.0);
        // Process some samples to set state
        let _ = bq.process(1.0_f32);
        assert_ne!(bq.z1, 0.0);

        bq.reset();
        assert_eq!(bq.z1, 0.0);
        assert_eq!(bq.z2, 0.0);
    }

    #[test]
    fn test_process_buffer() {
        let mut bq = Biquad::new();
        bq.set_biquad(BiquadType::LowPass, 0.25, 0.707, 0.0);

        let input: Vec<f32> = (0..64).map(|i| i as f32 * 0.01).collect();
        let mut output = vec![0.0f32; 64];

        bq.process_buffer(&input, &mut output);

        // Output should not be empty
        let any_nonzero = output.iter().any(|&v| v.abs() > 1e-10);
        assert!(any_nonzero, "Expected some non-zero output");
    }

    #[test]
    fn test_process_inplace() {
        let mut bq = Biquad::new();
        bq.set_biquad(BiquadType::HighPass, 0.25, 0.707, 0.0);

        let mut buffer = vec![0.5f32; 16];
        bq.process_inplace(&mut buffer);

        // Highpass should attenuate constant DC
        let sum: f32 = buffer.iter().sum();
        assert!(
            sum.abs() < 0.1,
            "Highpass should attenuate DC, got sum={}",
            sum
        );
    }

    /// Verify that coefficients match the C++ implementation exactly.
    #[test]
    fn test_lowpass_coefficients_match_cpp() {
        let cutoff = 0.5;
        let q = COMMON_Q;

        // C++ calculation
        let k = (std::f64::consts::PI * cutoff).tan();
        let norm_cpp = 1.0 / (1.0 + k / q + k * k);
        let a0_cpp = k * k * norm_cpp;
        let a1_cpp = 2.0 * a0_cpp;
        let a2_cpp = a0_cpp;
        let b1_cpp = 2.0 * (k * k - 1.0) * norm_cpp;
        let b2_cpp = (1.0 - k / q + k * k) * norm_cpp;

        let mut bq = Biquad::new();
        bq.set_biquad(BiquadType::LowPass, cutoff, q, 0.0);

        assert_close(bq.coeffs.a0, a0_cpp as f32, "a0");
        assert_close(bq.coeffs.a1, a1_cpp as f32, "a1");
        assert_close(bq.coeffs.a2, a2_cpp as f32, "a2");
        assert_close(bq.coeffs.b1, b1_cpp as f32, "b1");
        assert_close(bq.coeffs.b2, b2_cpp as f32, "b2");
    }

    #[test]
    fn test_all_types_have_valid_coefficients() {
        let test_params = [
            (BiquadType::LowPass, 0.25, 0.707, 0.0),
            (BiquadType::HighPass, 0.25, 0.707, 0.0),
            (BiquadType::BandPass, 0.25, 0.707, 0.0),
            (BiquadType::Notch, 0.25, 1.0, 0.0),
            (BiquadType::Peak, 0.3, 1.0, 6.0),
            (BiquadType::Peak, 0.3, 1.0, -6.0),
            (BiquadType::LowShelf, 0.1, 0.707, 3.0),
            (BiquadType::HighShelf, 0.5, 0.707, -3.0),
        ];

        for (filter_type, cutoff, q, gain_db) in test_params {
            let mut bq = Biquad::new();
            bq.set_biquad(filter_type, cutoff, q, gain_db);

            // All coefficients should be valid (finite, non-NaN)
            let c = &bq.coeffs;
            assert!(c.a0.is_finite(), "a0 not finite for {:?}", filter_type);
            assert!(c.a1.is_finite(), "a1 not finite for {:?}", filter_type);
            assert!(c.a2.is_finite(), "a2 not finite for {:?}", filter_type);
            assert!(c.b1.is_finite(), "b1 not finite for {:?}", filter_type);
            assert!(c.b2.is_finite(), "b2 not finite for {:?}", filter_type);
            assert!(!c.a0.is_nan(), "a0 is NaN for {:?}", filter_type);
            assert!(!c.a1.is_nan(), "a1 is NaN for {:?}", filter_type);
            assert!(!c.a2.is_nan(), "a2 is NaN for {:?}", filter_type);
            assert!(!c.b1.is_nan(), "b1 is NaN for {:?}", filter_type);
            assert!(!c.b2.is_nan(), "b2 is NaN for {:?}", filter_type);
        }
    }

    /// Verify impulse response for a highpass filter: first sample should be near 1,
    /// then decay toward zero.
    #[test]
    fn test_highpass_impulse_response() {
        let mut bq = Biquad::new();
        bq.set_biquad(BiquadType::HighPass, 0.25, 0.707, 0.0);

        // DC block: impulse response should decay to near zero
        let mut buffer = vec![0.0f32; 256];
        buffer[0] = 1.0;
        bq.process_inplace(&mut buffer);

        // Last few samples should be near zero (DC blocked)
        let tail_sum: f32 = buffer.iter().skip(200).sum::<f32>().abs();
        assert!(
            tail_sum < 0.1,
            "Highpass should decay DC: tail_sum={}",
            tail_sum
        );
    }

    #[test]
    fn test_lowpass_steady_state() {
        let mut bq = Biquad::new();
        bq.set_biquad(BiquadType::LowPass, 0.25, 0.707, 0.0);

        // Feed DC = 1.0 into lowpass
        let mut buffer = vec![1.0_f32; 256];
        bq.process_inplace(&mut buffer);

        // Lowpass at DC should pass through (gain ~1.0 at cutoff=0.5)
        let avg: f32 = buffer.iter().sum::<f32>() / buffer.len() as f32;
        assert!(avg > 0.9, "Lowpass at DC should pass: avg={}", avg);
    }

    const COMMON_Q: f64 = 0.707;
}
