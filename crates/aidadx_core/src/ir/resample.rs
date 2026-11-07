//! One-shot IR sample-rate conversion (replacement for r8brain's
//! `CDSPResampler16IR::oneshot`).
//!
//! Integer sample rates (the normal case) use rubato's synchronous FFT
//! resampler, which is linear-phase and — with even FFT sizes — exactly
//! time-aligned after its delay is trimmed. Non-integer rates fall back to
//! rubato's asynchronous windowed-sinc resampler, which may leave a residual
//! timing offset below one output sample.

use rubato::audioadapter_buffers::direct::SequentialSlice;
use rubato::{
    Async, Fft, FixedAsync, FixedSync, Resampler, SincInterpolationParameters,
    SincInterpolationType, WindowFunction,
};

use super::IrError;
use crate::convolver::Sample;

const WINDOW: WindowFunction = WindowFunction::BlackmanHarris2;

/// Minimum FFT size on the input side (anti-aliasing filter length) for the
/// synchronous resampler.
const MIN_FFT_SIZE_IN: usize = 1024;
/// Above this FFT size (pathological coprime rates) the sinc resampler is used.
const MAX_FFT_SIZE: usize = 1 << 20;

const SINC_LEN: usize = 256;
const SINC_OVERSAMPLING: usize = 256;
const SINC_CHUNK_SIZE: usize = 1024;

/// Output length for `input_len` frames resampled from `source_rate` to
/// `target_rate`: `ceil(input_len * target / source) + 1`, approximating the
/// value r8brain's `getMaxOutLen()` returns for the C++ `oneshot()` call.
pub fn resampled_len(input_len: usize, source_rate: f64, target_rate: f64) -> usize {
    (input_len as f64 * target_rate / source_rate).ceil() as usize + 1
}

/// Resample a mono IR from `source_rate` to `target_rate`.
///
/// Linear-phase band-limited resampling with unity passband gain; the filter
/// delay is removed so the output is time-aligned with the input. The output
/// length is [`resampled_len`]; the tail beyond the input is flushed with
/// zeros, as r8brain's `oneshot()` does. Allocates.
pub fn resample(
    input: &[Sample],
    source_rate: f64,
    target_rate: f64,
) -> Result<Vec<Sample>, IrError> {
    for rate in [source_rate, target_rate] {
        if !(rate.is_finite() && rate > 0.0) {
            return Err(IrError::InvalidSampleRate(rate));
        }
    }
    let out_len = resampled_len(input.len(), source_rate, target_rate);
    if source_rate == target_rate || input.is_empty() {
        let mut out = input.to_vec();
        out.resize(out_len, 0.0);
        return Ok(out);
    }

    let mut resampler = make_resampler(source_rate, target_rate)?;
    let input_buf = SequentialSlice::new(input, 1, input.len())
        .map_err(|e| IrError::Resample(e.to_string()))?;
    // `process_all` trims the startup delay, flushes the filter tail and
    // returns `ceil(len * ratio)` frames.
    let mut out = resampler
        .process_all(&input_buf, input.len(), None)
        .map_err(|e| IrError::Resample(e.to_string()))?
        .take_data();
    out.resize(out_len, 0.0);
    Ok(out)
}

fn make_resampler(
    source_rate: f64,
    target_rate: f64,
) -> Result<Box<dyn Resampler<Sample>>, IrError> {
    let err = |e: rubato::ResamplerConstructionError| IrError::Resample(e.to_string());

    if let (Some(fi), Some(fo)) = (as_integer_rate(source_rate), as_integer_rate(target_rate)) {
        let g = gcd(fi, fo);
        let (base_in, base_out) = (fi / g, fo / g);
        let mut chunks = MIN_FFT_SIZE_IN.div_ceil(base_in);
        // Odd FFT sizes leave a half-sample offset after delay trimming.
        if (chunks * base_in) % 2 != 0 || (chunks * base_out) % 2 != 0 {
            chunks *= 2;
        }
        let (fft_in, fft_out) = (chunks * base_in, chunks * base_out);
        if fft_in <= MAX_FFT_SIZE && fft_out <= MAX_FFT_SIZE {
            let r = Fft::<Sample>::new_custom(fi, fo, fft_in, 1, 1, WINDOW, FixedSync::Input)
                .map_err(err)?;
            return Ok(Box::new(r));
        }
    }

    let params = SincInterpolationParameters {
        sinc_len: SINC_LEN,
        // Automatic: highest cutoff that keeps aliasing below the window sidelobes.
        f_cutoff: None,
        oversampling_factor: SINC_OVERSAMPLING,
        interpolation: SincInterpolationType::Cubic,
        window: WINDOW,
    };
    let r = Async::<Sample>::new_sinc(
        target_rate / source_rate,
        1.0,
        &params,
        SINC_CHUNK_SIZE,
        1,
        FixedAsync::Input,
    )
    .map_err(err)?;
    Ok(Box::new(r))
}

fn as_integer_rate(rate: f64) -> Option<usize> {
    (rate.fract() == 0.0 && (1.0..=1.0e7).contains(&rate)).then_some(rate as usize)
}

fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}
