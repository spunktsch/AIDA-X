//! Partitioned FFT convolution engine for cabinet impulse responses.
//!
//! Port of the C++ [FFTConvolver](https://github.com/HiFi-LoFi/FFTConvolver)
//! (`FFTConvolver` + `TwoStageFFTConvolver`) used by the AIDA-X plugin, with
//! the FFT backend replaced by the pure-Rust [`realfft`]/`rustfft` crates.
//!
//! - [`FftConvolver`]: uniformly partitioned, zero-latency convolver.
//! - [`TwoStageConvolver`]: small head partitions for the start of the IR and
//!   large tail partitions for the rest; zero latency, low CPU for long IRs.
//!   Use [`DEFAULT_HEAD_BLOCK_SIZE`] / [`DEFAULT_TAIL_BLOCK_SIZE`] to match
//!   the C++ plugin.
//!
//! `init`/`reset` allocate and must run off the audio thread; `process` is
//! allocation-free and accepts any call length.
//!
//! IR file loading (WAV/FLAC) and sample-rate conversion are not part of this
//! module.

mod fft;
mod partitioned;
mod two_stage;

#[cfg(test)]
mod tests;

use std::fmt;

pub use partitioned::FftConvolver;
pub use two_stage::{TwoStageConvolver, DEFAULT_HEAD_BLOCK_SIZE, DEFAULT_TAIL_BLOCK_SIZE};

/// Audio sample type.
pub type Sample = f32;

/// IR samples with magnitude below this at the end of the IR are trimmed
/// (same threshold as the C++ implementation).
pub const ZERO_TRIM_THRESHOLD: Sample = 0.000_001;

/// Convolver configuration error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvolverError {
    /// A block size of zero was requested.
    InvalidBlockSize,
}

impl fmt::Display for ConvolverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBlockSize => f.write_str("invalid block size: must be > 0"),
        }
    }
}

impl std::error::Error for ConvolverError {}

/// Length of `ir` without trailing near-zero samples.
fn trimmed_len(ir: &[Sample]) -> usize {
    ir.iter()
        .rposition(|s| s.abs() >= ZERO_TRIM_THRESHOLD)
        .map_or(0, |i| i + 1)
}
