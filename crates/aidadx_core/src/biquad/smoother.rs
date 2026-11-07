/// Value smoother traits and implementations.
/// Ported from DPF's ExponentialValueSmoother and LinearValueSmoother.

/// Common trait for value smoothers.
pub trait ValueSmoother: Clone + std::fmt::Debug {
    /// Set the sample rate.
    fn set_sample_rate(&mut self, sample_rate: f64);

    /// Set the target value.
    fn set_target_value(&mut self, target: f64);

    /// Get the next smoothed value for one sample.
    fn next(&mut self) -> f64;

    /// Clear to current target value (stop smoothing).
    fn clear_to_target_value(&mut self);

    /// Peek at the next value without advancing.
    fn peek(&self) -> f64;
}

/// Mirrors DPF's `d_isNotEqual`: values differ by at least machine epsilon.
#[inline]
fn is_not_equal(a: f64, b: f64) -> bool {
    (a - b).abs() >= f64::EPSILON
}

/// DPF converts a T60 time constant to the exponential time constant tau
/// with this factor (`newT60 * (1.0 / 6.91)`).
const T60_TO_TAU: f64 = 1.0 / 6.91;

/// Exponential value smoother.
///
/// Port of DPF `ExponentialValueSmoother` (`distrho/extra/ValueSmoother.hpp`):
/// a first-order low-pass whose length is given as a T60 time constant
/// (the time for a 1-to-0 transition to fall to -60 dB). It is asymptotic and
/// never snaps to the target. A default-constructed smoother (tau = 0) jumps
/// to its target immediately, exactly like the C++ one.
#[derive(Debug, Clone)]
pub struct ExponentialValueSmoother {
    /// T60 time constant in seconds, as passed to `set_time_constant`.
    time_constant: f64,
    tau: f64,
    sample_rate: f64,
    target_value: f64,
    current_value: f64,
    coef: f64,
}

impl ExponentialValueSmoother {
    pub fn new() -> Self {
        Self {
            time_constant: 0.0,
            tau: 0.0,
            sample_rate: 0.0,
            target_value: 0.0,
            current_value: 0.0,
            coef: 0.0,
        }
    }

    pub fn with_time_constant(time_constant: f64) -> Self {
        let mut s = Self::new();
        s.set_time_constant(time_constant);
        s
    }

    /// The T60 time constant in seconds.
    pub fn time_constant(&self) -> f64 {
        self.time_constant
    }

    /// Set the T60 time constant in seconds.
    pub fn set_time_constant(&mut self, time_constant: f64) {
        self.time_constant = time_constant;
        let new_tau = time_constant * T60_TO_TAU;
        if is_not_equal(self.tau, new_tau) {
            self.tau = new_tau;
            self.update_coef();
        }
    }

    pub fn target_value(&self) -> f64 {
        self.target_value
    }

    /// The current (last output) value, like C++ `getCurrentValue()`.
    pub fn current_value(&self) -> f64 {
        self.current_value
    }

    fn update_coef(&mut self) {
        // With tau * sample_rate == 0 this is exp(-inf) == 0: no smoothing.
        self.coef = (-1.0 / (self.tau * self.sample_rate)).exp();
    }
}

impl Default for ExponentialValueSmoother {
    fn default() -> Self {
        Self::new()
    }
}

impl ValueSmoother for ExponentialValueSmoother {
    fn set_sample_rate(&mut self, sample_rate: f64) {
        if is_not_equal(self.sample_rate, sample_rate) {
            self.sample_rate = sample_rate;
            self.update_coef();
        }
    }

    fn set_target_value(&mut self, target: f64) {
        self.target_value = target;
    }

    #[inline]
    fn next(&mut self) -> f64 {
        self.current_value = self.peek();
        self.current_value
    }

    fn clear_to_target_value(&mut self) {
        self.current_value = self.target_value;
    }

    #[inline]
    fn peek(&self) -> f64 {
        self.current_value * self.coef + self.target_value * (1.0 - self.coef)
    }
}

/// Linear value smoother.
///
/// Port of DPF `LinearValueSmoother` (`distrho/extra/ValueSmoother.hpp`):
/// every time the target changes, a new linear segment starts that reaches
/// the target after `time_constant` seconds, whatever the distance. A
/// default-constructed smoother (tau = 0) jumps to its target immediately.
#[derive(Debug, Clone)]
pub struct LinearValueSmoother {
    time_constant: f64,
    sample_rate: f64,
    target_value: f64,
    current_value: f64,
    step: f64,
}

impl LinearValueSmoother {
    pub fn new() -> Self {
        Self {
            time_constant: 0.0,
            sample_rate: 0.0,
            target_value: 0.0,
            current_value: 0.0,
            step: 0.0,
        }
    }

    pub fn with_time_constant(time_constant: f64) -> Self {
        let mut s = Self::new();
        s.set_time_constant(time_constant);
        s
    }

    /// The segment duration in seconds.
    pub fn time_constant(&self) -> f64 {
        self.time_constant
    }

    /// Set the segment duration in seconds.
    pub fn set_time_constant(&mut self, time_constant: f64) {
        if is_not_equal(self.time_constant, time_constant) {
            self.time_constant = time_constant;
            self.update_step();
        }
    }

    pub fn target_value(&self) -> f64 {
        self.target_value
    }

    /// The current (last output) value, like C++ `getCurrentValue()`.
    pub fn current_value(&self) -> f64 {
        self.current_value
    }

    fn update_step(&mut self) {
        self.step = (self.target_value - self.current_value).abs()
            / (self.time_constant * self.sample_rate);
    }
}

impl Default for LinearValueSmoother {
    fn default() -> Self {
        Self::new()
    }
}

impl ValueSmoother for LinearValueSmoother {
    fn set_sample_rate(&mut self, sample_rate: f64) {
        if is_not_equal(self.sample_rate, sample_rate) {
            self.sample_rate = sample_rate;
            self.update_step();
        }
    }

    fn set_target_value(&mut self, target: f64) {
        if is_not_equal(self.target_value, target) {
            self.target_value = target;
            self.update_step();
        }
    }

    #[inline]
    fn next(&mut self) -> f64 {
        self.current_value = self.peek();
        self.current_value
    }

    fn clear_to_target_value(&mut self) {
        self.current_value = self.target_value;
    }

    #[inline]
    fn peek(&self) -> f64 {
        // f64::min ignores NaN like C++ std::fmin (step is NaN for 0/0).
        let dy = self.target_value - self.current_value;
        self.current_value + dy.abs().min(self.step).copysign(dy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exponential_smoother_basic() {
        let mut smoother = ExponentialValueSmoother::new();
        smoother.set_sample_rate(48000.0);
        smoother.set_target_value(1.0);

        let mut prev = 0.0;
        for _ in 0..1000 {
            let val = smoother.next();
            assert!(val >= prev, "Value should monotonically increase");
            prev = val;
        }

        assert!(
            (prev - 1.0).abs() < 0.01,
            "Should approach target: got {}",
            prev
        );
    }

    #[test]
    fn test_exponential_smoother_clear() {
        let mut smoother = ExponentialValueSmoother::new();
        smoother.set_sample_rate(48000.0);
        smoother.set_target_value(0.5);
        smoother.clear_to_target_value();

        assert!(
            (smoother.peek() - 0.5).abs() < 1e-10,
            "Clear should set current to target"
        );
    }

    #[test]
    fn test_exponential_smoother_peek() {
        let mut smoother = ExponentialValueSmoother::new();
        smoother.set_sample_rate(48000.0);
        smoother.set_time_constant(0.1);
        smoother.set_target_value(1.0);

        // Like DPF, peek() returns the value next() would produce, without advancing.
        assert_eq!(smoother.current_value(), 0.0, "Initial value should be 0");
        let peeked = smoother.peek();
        assert_eq!(smoother.current_value(), 0.0, "peek must not advance");
        assert_eq!(peeked, smoother.next(), "peek should equal the next value");
    }

    #[test]
    fn test_exponential_smoother_matches_dpf_coefficient() {
        // DPF: tau = T60 / 6.91, coef = exp(-1 / (tau * sr)),
        // next = mem * coef + target * (1 - coef)
        let mut smoother = ExponentialValueSmoother::with_time_constant(1.0);
        smoother.set_sample_rate(48000.0);
        smoother.set_target_value(1.0);

        let coef = (-1.0_f64 / (1.0 / 6.91 * 48000.0)).exp();
        assert!((smoother.next() - (1.0 - coef)).abs() < 1e-15);

        // After T60 seconds a 0-to-1 transition is within -60 dB of the target.
        for _ in 1..48000 {
            smoother.next();
        }
        assert!((1.0 - smoother.current_value()) < 1e-3 * 1.01);
    }

    #[test]
    fn test_linear_smoother_segment_duration() {
        // DPF recomputes the step on every target change so that each segment
        // takes exactly time_constant seconds, whatever its length.
        let mut smoother = LinearValueSmoother::with_time_constant(0.01);
        smoother.set_sample_rate(1000.0);
        smoother.set_target_value(5.0);
        for _ in 0..9 {
            smoother.next();
        }
        assert!(smoother.current_value() < 5.0);
        assert!((smoother.next() - 5.0).abs() < 1e-12);
    }

    #[test]
    fn test_exponential_smoother_direction_change() {
        let mut smoother = ExponentialValueSmoother::new();
        smoother.set_sample_rate(48000.0);
        smoother.set_target_value(1.0);

        for _ in 0..1000 {
            smoother.next();
        }
        let up_val = smoother.peek();

        smoother.set_target_value(0.0);
        for _ in 0..1000 {
            smoother.next();
        }
        let down_val = smoother.peek();

        assert!(up_val > 0.5, "Should have smoothed up: {}", up_val);
        assert!(down_val < 0.5, "Should have smoothed down: {}", down_val);
    }

    #[test]
    fn test_linear_smoother_basic() {
        let mut smoother = LinearValueSmoother::new();
        smoother.set_sample_rate(48000.0);
        smoother.set_target_value(1.0);

        let mut prev = 0.0;
        for _ in 0..1000 {
            let val = smoother.next();
            assert!(val >= prev, "Value should monotonically increase");
            prev = val;
        }

        assert!(
            (prev - 1.0).abs() < 0.01,
            "Should approach target: got {}",
            prev
        );
    }

    #[test]
    fn test_linear_smoother_clear() {
        let mut smoother = LinearValueSmoother::new();
        smoother.set_sample_rate(48000.0);
        smoother.set_target_value(0.5);
        smoother.clear_to_target_value();

        assert!(
            (smoother.peek() - 0.5).abs() < 1e-10,
            "Clear should set current to target"
        );
    }

    #[test]
    fn test_linear_smoother_step_limit() {
        let mut smoother = LinearValueSmoother::new();
        smoother.set_sample_rate(48000.0);
        smoother.set_time_constant(0.1);

        let expected_step = 1.0 / 0.1 / 48000.0;
        smoother.set_target_value(1.0);
        let first_step = smoother.next();

        assert!(
            (first_step - expected_step).abs() < 1e-15,
            "Step should be bounded: got {}, expected {}",
            first_step,
            expected_step
        );
    }

    #[test]
    fn test_smoother_no_change_when_at_target() {
        let mut smoother = ExponentialValueSmoother::new();
        smoother.set_sample_rate(48000.0);
        smoother.set_target_value(0.5);
        smoother.clear_to_target_value();

        let val1 = smoother.next();
        let val2 = smoother.next();
        assert!(
            (val1 - val2).abs() < 1e-15,
            "Value should not change when at target"
        );
    }
}
