//! WAV / FLAC decoding to mono `f32`.

use std::io::Cursor;

use super::IrError;
use crate::convolver::Sample;

/// Container format of an IR file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrFormat {
    /// RIFF/WAVE.
    Wav,
    /// Native FLAC stream.
    Flac,
}

impl IrFormat {
    /// Detect the format from the leading magic bytes.
    pub fn sniff(bytes: &[u8]) -> Option<Self> {
        if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
            Some(Self::Wav)
        } else if bytes.starts_with(b"fLaC") {
            Some(Self::Flac)
        } else {
            None
        }
    }
}

/// Decoded IR: first channel only, `f32` samples, native sample rate.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedIr {
    /// Mono samples (first channel of the file).
    pub samples: Vec<Sample>,
    /// File sample rate in Hz.
    pub sample_rate: u32,
    /// Channel count of the file.
    pub channels: u16,
    /// Container format.
    pub format: IrFormat,
}

/// Decode a WAV or FLAC file held in memory. Multi-channel files are reduced
/// to their first channel, as the C++ `loadCabinet` does.
pub fn decode_ir_bytes(bytes: &[u8]) -> Result<DecodedIr, IrError> {
    let decoded = match IrFormat::sniff(bytes) {
        Some(IrFormat::Wav) => decode_wav(bytes)?,
        Some(IrFormat::Flac) => decode_flac(bytes)?,
        None => return Err(IrError::UnknownFormat),
    };
    if decoded.sample_rate == 0 {
        return Err(IrError::InvalidSampleRate(0.0));
    }
    if decoded.samples.is_empty() {
        return Err(IrError::Empty);
    }
    Ok(decoded)
}

/// Scale factor mapping a signed `bits`-bit integer to `[-1, 1)`
/// (same as dr_wav / dr_flac `*_f32` conversion).
fn int_scale(bits: u32) -> Result<f32, IrError> {
    if (1..=32).contains(&bits) {
        Ok((1.0f64 / (1u64 << (bits - 1)) as f64) as f32)
    } else {
        Err(IrError::UnsupportedSampleFormat(format!(
            "{bits}-bit integer samples"
        )))
    }
}

fn decode_wav(bytes: &[u8]) -> Result<DecodedIr, IrError> {
    let mut reader = hound::WavReader::new(Cursor::new(bytes)).map_err(|e| wav_error(&e))?;
    let spec = reader.spec();
    let channels = usize::from(spec.channels);
    if channels == 0 {
        return Err(IrError::Empty);
    }

    // Keep every `channels`-th sample (first channel), like the C++ loader.
    let samples: Vec<Sample> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let scale = int_scale(u32::from(spec.bits_per_sample))?;
            reader
                .samples::<i32>()
                .step_by(channels)
                .map(|s| s.map(|v| v as f32 * scale))
                .collect::<Result<_, _>>()
                .map_err(|e| wav_error(&e))?
        }
        hound::SampleFormat::Float => {
            if spec.bits_per_sample != 32 {
                return Err(IrError::UnsupportedSampleFormat(format!(
                    "{}-bit float WAV",
                    spec.bits_per_sample
                )));
            }
            reader
                .samples::<f32>()
                .step_by(channels)
                .collect::<Result<_, _>>()
                .map_err(|e| wav_error(&e))?
        }
    };

    Ok(DecodedIr {
        samples,
        sample_rate: spec.sample_rate,
        channels: spec.channels,
        format: IrFormat::Wav,
    })
}

fn wav_error(e: &hound::Error) -> IrError {
    match e {
        hound::Error::Unsupported => {
            IrError::UnsupportedSampleFormat("unsupported WAV encoding".to_owned())
        }
        other => IrError::Decode(other.to_string()),
    }
}

fn decode_flac(bytes: &[u8]) -> Result<DecodedIr, IrError> {
    let mut reader =
        claxon::FlacReader::new(Cursor::new(bytes)).map_err(|e| IrError::Decode(e.to_string()))?;
    let info = reader.streaminfo();
    let channels = info.channels as usize;
    if channels == 0 {
        return Err(IrError::Empty);
    }
    let scale = int_scale(info.bits_per_sample)?;

    let samples: Vec<Sample> = reader
        .samples()
        .step_by(channels)
        .map(|s| s.map(|v| v as f32 * scale))
        .collect::<Result<_, _>>()
        .map_err(|e| IrError::Decode(e.to_string()))?;

    Ok(DecodedIr {
        samples,
        sample_rate: info.sample_rate,
        // FLAC allows at most 8 channels.
        channels: info.channels as u16,
        format: IrFormat::Flac,
    })
}
