//! Thin real-FFT wrapper around [`realfft`] with pre-allocated scratch space.
//!
//! All buffers are allocated in [`RealFft::new`]; [`RealFft::forward`] and
//! [`RealFft::inverse`] never allocate, so they are safe to call on the audio
//! thread.

use std::sync::Arc;

use realfft::num_complex::Complex;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};

use super::Sample;

/// Complex spectrum bin type.
pub type Bin = Complex<Sample>;

/// Forward/inverse real FFT pair of a fixed size.
pub struct RealFft {
    forward: Arc<dyn RealToComplex<Sample>>,
    inverse: Arc<dyn ComplexToReal<Sample>>,
    scratch_forward: Vec<Bin>,
    scratch_inverse: Vec<Bin>,
}

impl RealFft {
    /// Plan a real FFT of `size` points (must be even and > 0).
    pub fn new(size: usize) -> Self {
        debug_assert!(size >= 2 && size % 2 == 0);
        let mut planner = RealFftPlanner::<Sample>::new();
        let forward = planner.plan_fft_forward(size);
        let inverse = planner.plan_fft_inverse(size);
        let scratch_forward = forward.make_scratch_vec();
        let scratch_inverse = inverse.make_scratch_vec();
        Self {
            forward,
            inverse,
            scratch_forward,
            scratch_inverse,
        }
    }

    /// Number of complex bins (`size / 2 + 1`).
    #[inline]
    pub fn complex_size(&self) -> usize {
        self.forward.complex_len()
    }

    /// Unnormalized forward transform. `input` (length `size`) is used as
    /// scratch and its contents are destroyed.
    #[inline]
    pub fn forward(&mut self, input: &mut [Sample], output: &mut [Bin]) {
        let res = self
            .forward
            .process_with_scratch(input, output, &mut self.scratch_forward);
        debug_assert!(res.is_ok(), "forward FFT buffer size mismatch");
    }

    /// Unnormalized inverse transform (the result is scaled by `size`).
    /// `input` (length `size / 2 + 1`) is used as scratch and its contents
    /// are destroyed.
    #[inline]
    pub fn inverse(&mut self, input: &mut [Bin], output: &mut [Sample]) {
        // The DC and Nyquist bins of a real signal's spectrum are purely real;
        // clear any rounding residue so realfft does not reject the input.
        if let Some(first) = input.first_mut() {
            first.im = 0.0;
        }
        if let Some(last) = input.last_mut() {
            last.im = 0.0;
        }
        let res = self
            .inverse
            .process_with_scratch(input, output, &mut self.scratch_inverse);
        debug_assert!(res.is_ok(), "inverse FFT buffer size mismatch");
    }
}

/// `acc += a * b` (element-wise complex multiply-accumulate).
#[inline]
pub fn complex_multiply_accumulate(acc: &mut [Bin], a: &[Bin], b: &[Bin]) {
    debug_assert!(acc.len() == a.len() && a.len() == b.len());
    for ((r, x), y) in acc.iter_mut().zip(a).zip(b) {
        *r += *x * *y;
    }
}
