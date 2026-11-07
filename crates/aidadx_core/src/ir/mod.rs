//! Cabinet impulse response (IR) loading.
//!
//! Rust port of the C++ cabinet loader in `src/aidadsp-plugin.cpp`
//! (`loadCabinetFromFile` / `loadCabinet`):
//!
//! 1. Decode WAV ([`hound`]) or FLAC ([`claxon`]) to `f32` in `[-1, 1)`.
//! 2. Multi-channel files: keep only the first channel (the C++ loader copies
//!    every `channels`-th sample, i.e. the left channel).
//! 3. If the file sample rate differs from the host sample rate, resample with
//!    a linear-phase band-limited resampler ([`rubato`], see [`resample`]);
//!    the output length is `ceil(frames * target / source) + 1`, approximating
//!    r8brain's `getMaxOutLen()` used by the C++ `oneshot()` call. The
//!    resampler delay is compensated, so the IR stays time-aligned, and the
//!    passband gain is unity.
//!
//! Unlike the C++ loader, the format is detected from the file contents
//! (`RIFF....WAVE` / `fLaC`) instead of the `.flac` extension.
//! 4. No normalisation and no extra trimming: like the C++ code, the IR is
//!    handed to the convolver unchanged; [`TwoStageConvolver::init`] trims
//!    trailing near-zero samples itself.
//!
//! Everything here allocates and must run off the audio thread.

mod decode;
mod resample;

#[cfg(test)]
mod tests;

use std::fmt;
use std::path::Path;

use crate::convolver::{
    ConvolverError, Sample, TwoStageConvolver, DEFAULT_HEAD_BLOCK_SIZE, DEFAULT_TAIL_BLOCK_SIZE,
};

pub use decode::{decode_ir_bytes, DecodedIr, IrFormat};
pub use resample::{resample, resampled_len};

/// Error returned by the IR loader.
#[derive(Debug)]
pub enum IrError {
    /// The file could not be read.
    Io(std::io::Error),
    /// The data is neither a RIFF/WAVE nor a FLAC stream.
    UnknownFormat,
    /// The WAV/FLAC decoder rejected the data.
    Decode(String),
    /// The sample format is not supported (e.g. 64-bit float WAV, A-law).
    UnsupportedSampleFormat(String),
    /// The file contains no audio frames (or no channels).
    Empty,
    /// A source or target sample rate is zero, negative or not finite.
    InvalidSampleRate(f64),
    /// The resampler could not be constructed or failed.
    Resample(String),
    /// The convolver rejected the IR.
    Convolver(ConvolverError),
}

impl fmt::Display for IrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::UnknownFormat => f.write_str("unknown IR file format (expected WAV or FLAC)"),
            Self::Decode(e) => write!(f, "decode error: {e}"),
            Self::UnsupportedSampleFormat(e) => write!(f, "unsupported sample format: {e}"),
            Self::Empty => f.write_str("IR contains no audio frames"),
            Self::InvalidSampleRate(sr) => write!(f, "invalid sample rate: {sr}"),
            Self::Resample(e) => write!(f, "resampling error: {e}"),
            Self::Convolver(e) => write!(f, "convolver error: {e}"),
        }
    }
}

impl std::error::Error for IrError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Convolver(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for IrError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<ConvolverError> for IrError {
    fn from(e: ConvolverError) -> Self {
        Self::Convolver(e)
    }
}

/// A mono IR ready for the convolver.
#[derive(Debug, Clone, PartialEq)]
pub struct IrData {
    /// Mono samples at [`sample_rate`](Self::sample_rate).
    pub samples: Vec<Sample>,
    /// Sample rate of `samples` (the requested target rate).
    pub sample_rate: f64,
    /// Sample rate of the source file.
    pub source_sample_rate: u32,
    /// Channel count of the source file (only the first channel is used).
    pub source_channels: u16,
    /// Frame count of the source file (before resampling).
    pub source_frames: usize,
    /// Container format of the source file.
    pub format: IrFormat,
}

impl IrData {
    /// Build a [`TwoStageConvolver`] with the C++ plugin's block sizes
    /// (128 head / 1024 tail). Allocates; call off the audio thread.
    pub fn build_convolver(&self) -> Result<TwoStageConvolver, IrError> {
        let mut conv = TwoStageConvolver::new();
        conv.init(
            DEFAULT_HEAD_BLOCK_SIZE,
            DEFAULT_TAIL_BLOCK_SIZE,
            &self.samples,
        )?;
        Ok(conv)
    }
}

/// Load an IR file (WAV or FLAC, detected from the file contents) and
/// resample it to `target_sample_rate`.
pub fn load_ir_file(path: impl AsRef<Path>, target_sample_rate: f64) -> Result<IrData, IrError> {
    let bytes = std::fs::read(path)?;
    load_ir_bytes(&bytes, target_sample_rate)
}

/// Load an in-memory IR file (WAV or FLAC) and resample it to
/// `target_sample_rate`.
pub fn load_ir_bytes(bytes: &[u8], target_sample_rate: f64) -> Result<IrData, IrError> {
    if !(target_sample_rate.is_finite() && target_sample_rate > 0.0) {
        return Err(IrError::InvalidSampleRate(target_sample_rate));
    }
    let decoded = decode_ir_bytes(bytes)?;
    let source_frames = decoded.samples.len();

    // Same comparison as C++ `if (sampleRate != hostSampleRate)`.
    let samples = if f64::from(decoded.sample_rate) == target_sample_rate {
        decoded.samples
    } else {
        resample(
            &decoded.samples,
            f64::from(decoded.sample_rate),
            target_sample_rate,
        )?
    };

    Ok(IrData {
        samples,
        sample_rate: target_sample_rate,
        source_sample_rate: decoded.sample_rate,
        source_channels: decoded.channels,
        source_frames,
        format: decoded.format,
    })
}

/// Load an IR file and return a ready-to-use convolver (C++ block sizes).
pub fn load_ir_convolver(
    path: impl AsRef<Path>,
    target_sample_rate: f64,
) -> Result<TwoStageConvolver, IrError> {
    load_ir_file(path, target_sample_rate)?.build_convolver()
}
