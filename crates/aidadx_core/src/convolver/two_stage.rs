//! Two-stage partitioned FFT convolver (port of
//! `fftconvolver::TwoStageFFTConvolver`).
//!
//! The impulse response is split into up to three parts, with `H` the head
//! block size and `T` the tail block size:
//!
//! | part   | IR range        | convolver block size | output delay |
//! |--------|-----------------|----------------------|--------------|
//! | head   | `[0, T)`        | `H`                  | none         |
//! | tail 0 | `[T, 2T)`       | `H`                  | `T`          |
//! | tail 1 | `[2T, len)`     | `T`                  | `2T`         |
//!
//! The tail convolvers' results are computed ahead of time into
//! "precalculated" buffers that are summed into the output exactly when the
//! IR offset of their part makes them due, so the whole convolver remains
//! zero-latency while the long tail uses large, cheap FFT partitions.
//!
//! Unlike the C++ version (which rejects IRs of `2T` samples or less and
//! relies on the caller to fall back to a single-stage convolver), this port
//! handles any IR length: tail stages are simply omitted when not needed.
//!
//! The tail-1 stage runs synchronously in [`TwoStageConvolver::process`]
//! (equivalent to the C++ base class without the background thread). All
//! memory is allocated in [`TwoStageConvolver::init`]; `process` is
//! allocation-free.

use super::partitioned::FftConvolver;
use super::{trimmed_len, ConvolverError, Sample};

/// Head block size used by the AIDA-X plugin (`kHeadBlockSize`).
pub const DEFAULT_HEAD_BLOCK_SIZE: usize = 128;
/// Tail block size used by the AIDA-X plugin (`kTailBlockSize`).
pub const DEFAULT_TAIL_BLOCK_SIZE: usize = 1024;

/// Zero-latency two-stage FFT convolver.
#[derive(Default)]
pub struct TwoStageConvolver {
    head_block_size: usize,
    tail_block_size: usize,
    head: FftConvolver,
    tail0: FftConvolver,
    tail1: FftConvolver,
    tail_output0: Vec<Sample>,
    tail_precalculated0: Vec<Sample>,
    tail_output1: Vec<Sample>,
    tail_precalculated1: Vec<Sample>,
    tail_input: Vec<Sample>,
    tail_input_fill: usize,
    precalculated_pos: usize,
}

impl TwoStageConvolver {
    /// Create an empty convolver. Until [`init`](Self::init) is called with a
    /// non-silent IR, [`process`](Self::process) outputs silence.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load an impulse response.
    ///
    /// Block sizes are rounded up to powers of two; if `head_block_size` is
    /// larger than `tail_block_size` they are swapped. Trailing near-zero
    /// samples of `ir` are trimmed.
    ///
    /// Allocates; must not be called on the audio thread.
    pub fn init(
        &mut self,
        head_block_size: usize,
        tail_block_size: usize,
        ir: &[Sample],
    ) -> Result<(), ConvolverError> {
        self.reset();

        if head_block_size == 0 || tail_block_size == 0 {
            return Err(ConvolverError::InvalidBlockSize);
        }
        let (head_bs, tail_bs) = if head_block_size > tail_block_size {
            (tail_block_size, head_block_size)
        } else {
            (head_block_size, tail_block_size)
        };
        let head_bs = head_bs.next_power_of_two();
        let tail_bs = tail_bs.next_power_of_two();

        let ir_len = trimmed_len(ir);
        if ir_len == 0 {
            return Ok(());
        }
        self.head_block_size = head_bs;
        self.tail_block_size = tail_bs;

        let head_ir_len = ir_len.min(tail_bs);
        self.head.init(head_bs, &ir[..head_ir_len])?;

        if ir_len > tail_bs {
            let tail0_ir_len = (ir_len - tail_bs).min(tail_bs);
            self.tail0
                .init(head_bs, &ir[tail_bs..tail_bs + tail0_ir_len])?;
            self.tail_output0 = vec![0.0; tail_bs];
            self.tail_precalculated0 = vec![0.0; tail_bs];
        }

        if ir_len > 2 * tail_bs {
            self.tail1.init(tail_bs, &ir[2 * tail_bs..ir_len])?;
            self.tail_output1 = vec![0.0; tail_bs];
            self.tail_precalculated1 = vec![0.0; tail_bs];
        }

        // A tail part made only of near-zero samples is trimmed to nothing by
        // the inner convolver; drop its buffers so it costs nothing.
        if !self.tail0.is_initialized() {
            self.tail_output0 = Vec::new();
            self.tail_precalculated0 = Vec::new();
        }
        if !self.tail1.is_initialized() {
            self.tail_output1 = Vec::new();
            self.tail_precalculated1 = Vec::new();
        }

        if !self.tail_precalculated0.is_empty() || !self.tail_precalculated1.is_empty() {
            self.tail_input = vec![0.0; tail_bs];
        }
        Ok(())
    }

    /// Convolve `input` and write the result to `output` (overwriting it).
    ///
    /// Any call length is accepted. Real-time safe (no allocation, no locks).
    ///
    /// # Panics
    /// If `input` and `output` have different lengths.
    pub fn process(&mut self, input: &[Sample], output: &mut [Sample]) {
        // Head (also validates lengths and writes silence when empty).
        self.head.process(input, output);

        if self.tail_input.is_empty() {
            return;
        }

        let head_bs = self.head_block_size;
        let tail_bs = self.tail_block_size;
        let len = input.len();
        let mut processed = 0;
        while processed < len {
            let processing = (len - processed).min(head_bs - (self.tail_input_fill % head_bs));
            debug_assert!(self.tail_input_fill + processing <= tail_bs);

            // Sum precalculated tail output into the head output.
            let out = &mut output[processed..processed + processing];
            let pre = self.precalculated_pos..self.precalculated_pos + processing;
            if !self.tail_precalculated0.is_empty() {
                for (o, t) in out.iter_mut().zip(&self.tail_precalculated0[pre.clone()]) {
                    *o += t;
                }
            }
            if !self.tail_precalculated1.is_empty() {
                for (o, t) in out.iter_mut().zip(&self.tail_precalculated1[pre]) {
                    *o += t;
                }
            }
            self.precalculated_pos += processing;

            // Collect input for the tail convolvers.
            self.tail_input[self.tail_input_fill..self.tail_input_fill + processing]
                .copy_from_slice(&input[processed..processed + processing]);
            self.tail_input_fill += processing;

            // 1st tail block: processed every head block, published every tail block.
            if !self.tail_precalculated0.is_empty() && self.tail_input_fill % head_bs == 0 {
                let offset = self.tail_input_fill - head_bs;
                self.tail0.process(
                    &self.tail_input[offset..offset + head_bs],
                    &mut self.tail_output0[offset..offset + head_bs],
                );
                if self.tail_input_fill == tail_bs {
                    std::mem::swap(&mut self.tail_precalculated0, &mut self.tail_output0);
                }
            }

            // 2nd..Nth tail blocks: publish the previous result, then compute
            // the next one (done on a background thread in the C++ plugin).
            if !self.tail_precalculated1.is_empty() && self.tail_input_fill == tail_bs {
                std::mem::swap(&mut self.tail_precalculated1, &mut self.tail_output1);
                self.tail1.process(&self.tail_input, &mut self.tail_output1);
            }

            if self.tail_input_fill == tail_bs {
                self.tail_input_fill = 0;
                self.precalculated_pos = 0;
            }

            processed += processing;
        }
    }

    /// Clear the IR and all state, releasing memory. Must not be called on the
    /// audio thread.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Head block size (after rounding), or 0 when empty.
    pub fn head_block_size(&self) -> usize {
        self.head_block_size
    }

    /// Tail block size (after rounding), or 0 when empty.
    pub fn tail_block_size(&self) -> usize {
        self.tail_block_size
    }

    /// Whether a non-silent IR is loaded.
    pub fn is_initialized(&self) -> bool {
        // Block sizes are only set once a non-silent IR was accepted (the head
        // part alone may be silent, e.g. for a pure delay longer than T).
        self.head_block_size != 0
    }

    /// Processing latency in samples (always 0).
    pub fn latency(&self) -> usize {
        0
    }
}
