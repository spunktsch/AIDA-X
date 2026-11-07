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

/// Exponential value smoother.
///
/// Smooths transitions using exponential interpolation with a time constant.
#[derive(Debug, Clone)]
pub struct ExponentialValueSmoother {
    time_constant: f64,
    sample_rate: f64,
    target_value: f64,
    current_value: f64,
    alpha: f64,
}

impl ExponentialValueSmoother {
    pub fn new() -> Self {
        Self {
            time_constant: 0.1,
            sample_rate: 48000.0,
            target_value: 0.0,
            current_value: 0.0,
            alpha: 0.0,
        }
    }

    pub fn with_time_constant(time_constant: f64) -> Self {
        let mut s = Self::new();
        s.time_constant = time_constant;
        s
    }

    pub fn time_constant(&self) -> f64 {
        self.time_constant
    }

    pub fn set_time_constant(&mut self, time_constant: f64) {
        self.time_constant = time_constant;
    }

    pub fn target_value(&self) -> f64 {
        self.target_value
    }
}

impl Default for ExponentialValueSmoother {
    fn default() -> Self {
        Self::new()
    }
}

impl ValueSmoother for ExponentialValueSmoother {
    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_rate = sample_rate;
        self.alpha = 1.0 - (1.0 / self.time_constant / sample_rate).exp();
    }

    fn set_target_value(&mut self, target: f64) {
        self.target_value = target;
    }

    #[inline]
    fn next(&mut self) -> f64 {
        let diff = self.target_value - self.current_value;
        if diff.abs() < 1e-10 {
            self.current_value = self.target_value;
            return self.current_value;
        }
        self.current_value += diff * self.alpha;
        self.current_value
    }

    fn clear_to_target_value(&mut self) {
        self.current_value = self.target_value;
    }

    fn peek(&self) -> f64 {
        self.current_value
    }
}

/// Linear value smoother.
///
/// Smooths transitions using linear interpolation with a time constant.
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
            time_constant: 0.1,
            sample_rate: 48000.0,
            target_value: 0.0,
            current_value: 0.0,
            step: 0.0,
        }
    }

    pub fn with_time_constant(time_constant: f64) -> Self {
        let mut s = Self::new();
        s.time_constant = time_constant;
        s
    }

    pub fn time_constant(&self) -> f64 {
        self.time_constant
    }

    pub fn set_time_constant(&mut self, time_constant: f64) {
        self.time_constant = time_constant;
    }

    pub fn target_value(&self) -> f64 {
        self.target_value
    }
}

impl Default for LinearValueSmoother {
    fn default() -> Self {
        Self::new()
    }
}

impl ValueSmoother for LinearValueSmoother {
    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_rate = sample_rate;
        self.step = 1.0 / self.time_constant / sample_rate;
    }

    fn set_target_value(&mut self, target: f64) {
        self.target_value = target;
    }

    #[inline]
    fn next(&mut self) -> f64 {
        let diff = self.target_value - self.current_value;
        if diff.abs() < 1e-10 {
            self.current_value = self.target_value;
            return self.current_value;
        }
        let step = if diff.abs() < self.step.abs() {
            diff
        } else {
            diff.signum() * self.step
        };
        self.current_value += step;
        self.current_value
    }

    fn clear_to_target_value(&mut self) {
        self.current_value = self.target_value;
    }

    fn peek(&self) -> f64 {
        self.current_value
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
        smoother.set_target_value(1.0);

        let initial = smoother.peek();
        assert_eq!(initial, 0.0, "Initial value should be 0");
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
