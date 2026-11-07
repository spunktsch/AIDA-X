//! Uniformly partitioned FFT convolver (port of `fftconvolver::FFTConvolver`).
//!
//! The impulse response is split into segments of `block_size` samples, each
//! transformed once at init time into a `2 * block_size` point spectrum.
//! Audio is processed with uniform-partition overlap-add in the frequency
//! domain using a frequency-domain delay line of input spectra.
//!
//! The convolver has zero latency: every call to [`FftConvolver::process`]
//! produces output for exactly the samples it received, regardless of how the
//! call length relates to the partition size. Partial blocks are handled by
//! re-running the FFT of the (zero-padded) partially filled input block, as
//! in the C++ original; contributions from older input blocks are summed once
//! per block and cached in `pre_multiplied`.
//!
//! All memory is allocated in [`FftConvolver::init`]; `process` is
//! allocation-free.

use super::fft::{complex_multiply_accumulate, Bin, RealFft};
use super::{trimmed_len, ConvolverError, Sample};

/// Single-stage, uniformly partitioned, zero-latency FFT convolver.
#[derive(Default)]
pub struct FftConvolver {
    block_size: usize,
    seg_count: usize,
    fft: Option<RealFft>,
    /// Frequency-domain delay line of input block spectra (ring buffer).
    segments: Vec<Vec<Bin>>,
    /// Pre-computed IR segment spectra (scaled by 1/fft_size).
    segments_ir: Vec<Vec<Bin>>,
    /// Time-domain FFT work buffer, `2 * block_size` long.
    fft_buffer: Vec<Sample>,
    /// Sum of older segments' products for the current input block.
    pre_multiplied: Vec<Bin>,
    /// Spectrum of the full convolution result for the current call.
    conv: Vec<Bin>,
    /// Overlap from the previous block's inverse FFT.
    overlap: Vec<Sample>,
    input_buffer: Vec<Sample>,
    input_fill: usize,
    current: usize,
}

impl FftConvolver {
    /// Create an empty convolver. Until [`init`](Self::init) is called with a
    /// non-silent IR, [`process`](Self::process) outputs silence.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load an impulse response. `block_size` is rounded up to the next power
    /// of two. Trailing near-zero samples of `ir` are trimmed.
    ///
    /// Allocates; must not be called on the audio thread.
    pub fn init(&mut self, block_size: usize, ir: &[Sample]) -> Result<(), ConvolverError> {
        self.reset();

        if block_size == 0 {
            return Err(ConvolverError::InvalidBlockSize);
        }

        let ir_len = trimmed_len(ir);
        if ir_len == 0 {
            return Ok(());
        }

        let block_size = block_size.next_power_of_two();
        let seg_size = 2 * block_size;
        let seg_count = ir_len.div_ceil(block_size);
        let mut fft = RealFft::new(seg_size);
        let complex_size = fft.complex_size();

        let mut fft_buffer = vec![0.0; seg_size];
        let scale = 1.0 / seg_size as Sample;
        let segments_ir = ir[..ir_len]
            .chunks(block_size)
            .map(|chunk| {
                fft_buffer.fill(0.0);
                fft_buffer[..chunk.len()].copy_from_slice(chunk);
                let mut spectrum = vec![Bin::default(); complex_size];
                fft.forward(&mut fft_buffer, &mut spectrum);
                spectrum.iter_mut().for_each(|b| *b *= scale);
                spectrum
            })
            .collect();

        self.block_size = block_size;
        self.seg_count = seg_count;
        self.fft = Some(fft);
        self.segments = vec![vec![Bin::default(); complex_size]; seg_count];
        self.segments_ir = segments_ir;
        self.fft_buffer = fft_buffer;
        self.pre_multiplied = vec![Bin::default(); complex_size];
        self.conv = vec![Bin::default(); complex_size];
        self.overlap = vec![0.0; block_size];
        self.input_buffer = vec![0.0; block_size];
        self.input_fill = 0;
        self.current = 0;
        Ok(())
    }

    /// Convolve `input` and write the result to `output` (overwriting it).
    ///
    /// Any call length is accepted. Real-time safe (no allocation, no locks).
    ///
    /// # Panics
    /// If `input` and `output` have different lengths.
    pub fn process(&mut self, input: &[Sample], output: &mut [Sample]) {
        assert_eq!(input.len(), output.len(), "input/output length mismatch");

        let Some(fft) = self.fft.as_mut() else {
            output.fill(0.0);
            return;
        };

        let block_size = self.block_size;
        let len = input.len();
        let mut processed = 0;
        while processed < len {
            let input_was_empty = self.input_fill == 0;
            let processing = (len - processed).min(block_size - self.input_fill);
            let pos = self.input_fill;
            self.input_buffer[pos..pos + processing]
                .copy_from_slice(&input[processed..processed + processing]);

            // Forward FFT of the (zero-padded) current input block.
            self.fft_buffer[..block_size].copy_from_slice(&self.input_buffer);
            self.fft_buffer[block_size..].fill(0.0);
            fft.forward(&mut self.fft_buffer, &mut self.segments[self.current]);

            // Contributions of older input blocks only change once per block.
            if input_was_empty {
                self.pre_multiplied.fill(Bin::default());
                for i in 1..self.seg_count {
                    let index_audio = (self.current + i) % self.seg_count;
                    complex_multiply_accumulate(
                        &mut self.pre_multiplied,
                        &self.segments_ir[i],
                        &self.segments[index_audio],
                    );
                }
            }
            self.conv.copy_from_slice(&self.pre_multiplied);
            complex_multiply_accumulate(
                &mut self.conv,
                &self.segments[self.current],
                &self.segments_ir[0],
            );

            // Backward FFT.
            fft.inverse(&mut self.conv, &mut self.fft_buffer);

            // Output = current result + overlap of the previous block.
            for ((out, y), ov) in output[processed..processed + processing]
                .iter_mut()
                .zip(&self.fft_buffer[pos..pos + processing])
                .zip(&self.overlap[pos..pos + processing])
            {
                *out = y + ov;
            }

            self.input_fill += processing;
            if self.input_fill == block_size {
                self.input_buffer.fill(0.0);
                self.input_fill = 0;
                self.overlap
                    .copy_from_slice(&self.fft_buffer[block_size..2 * block_size]);
                self.current = if self.current > 0 {
                    self.current - 1
                } else {
                    self.seg_count - 1
                };
            }

            processed += processing;
        }
    }

    /// Clear the IR and all state, releasing memory. Allocation-wise this only
    /// frees; it must still not be called on the audio thread.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Partition block size (after power-of-two rounding), or 0 when empty.
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    /// Number of IR partitions, or 0 when empty.
    pub fn segment_count(&self) -> usize {
        self.seg_count
    }

    /// Whether a non-silent IR is loaded.
    pub fn is_initialized(&self) -> bool {
        self.seg_count > 0
    }
}
