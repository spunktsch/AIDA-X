use std::path::PathBuf;

use super::*;

fn bundled_ir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../files/V30-P2-audix-i5-deerinkstudios.wav")
}

/// Deterministic test signal in [-0.9, 0.9], channel `ch` scaled by `1/(ch+1)`.
fn test_signal(frames: usize, ch: usize) -> Vec<f64> {
    (0..frames)
        .map(|i| 0.9 * (i as f64 * 0.37 + ch as f64).sin() / (ch + 1) as f64)
        .collect()
}

fn write_wav_int(frames: usize, channels: u16, bits: u16, sr: u32) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ir.wav");
    let spec = hound::WavSpec {
        channels,
        sample_rate: sr,
        bits_per_sample: bits,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    let max = (1i64 << (bits - 1)) as f64;
    let sig: Vec<Vec<f64>> = (0..channels as usize)
        .map(|c| test_signal(frames, c))
        .collect();
    for i in 0..frames {
        for ch in &sig {
            let v = (ch[i] * max).round() as i32;
            if bits == 8 {
                w.write_sample(v as i8).unwrap();
            } else if bits == 16 {
                w.write_sample(v as i16).unwrap();
            } else {
                w.write_sample(v).unwrap();
            }
        }
    }
    w.finalize().unwrap();
    (dir, path)
}

fn write_wav_float(samples: &[f32], channels: u16, sr: u32) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ir.wav");
    let spec = hound::WavSpec {
        channels,
        sample_rate: sr,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    for &s in samples {
        w.write_sample(s).unwrap();
    }
    w.finalize().unwrap();
    (dir, path)
}

// ---------------------------------------------------------------------------
// Minimal FLAC encoder (VERBATIM subframes, single frame) for test fixtures.

struct BitWriter {
    bytes: Vec<u8>,
    acc: u64,
    nbits: u32,
}

impl BitWriter {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            acc: 0,
            nbits: 0,
        }
    }
    fn put(&mut self, value: u64, bits: u32) {
        for i in (0..bits).rev() {
            self.acc = (self.acc << 1) | ((value >> i) & 1);
            self.nbits += 1;
            if self.nbits == 8 {
                self.bytes.push(self.acc as u8);
                self.acc = 0;
                self.nbits = 0;
            }
        }
    }
    fn align(&mut self) {
        while self.nbits != 0 {
            self.put(0, 1);
        }
    }
}

fn crc8(data: &[u8]) -> u8 {
    let mut crc = 0u8;
    for &b in data {
        crc ^= b;
        for _ in 0..8 {
            crc = if crc & 0x80 != 0 {
                (crc << 1) ^ 0x07
            } else {
                crc << 1
            };
        }
    }
    crc
}

fn crc16(data: &[u8]) -> u16 {
    let mut crc = 0u16;
    for &b in data {
        crc ^= u16::from(b) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x8005
            } else {
                crc << 1
            };
        }
    }
    crc
}

/// Encode interleaved-by-channel integer samples (`chans[c][i]`) as FLAC.
fn encode_flac(chans: &[Vec<i32>], bits: u32, sr: u32) -> Vec<u8> {
    let frames = chans[0].len();
    assert!((16..=65535).contains(&frames));
    let mut out = b"fLaC".to_vec();

    let mut si = BitWriter::new();
    si.put(1, 1); // last metadata block
    si.put(0, 7); // STREAMINFO
    si.put(34, 24);
    si.put(frames as u64, 16); // min block size
    si.put(frames as u64, 16); // max block size
    si.put(0, 24);
    si.put(0, 24);
    si.put(u64::from(sr), 20);
    si.put(chans.len() as u64 - 1, 3);
    si.put(u64::from(bits) - 1, 5);
    si.put(frames as u64, 36);
    si.put(0, 64);
    si.put(0, 64); // MD5 unknown
    out.extend_from_slice(&si.bytes);

    let mut fr = BitWriter::new();
    fr.put(0b11_1111_1111_1110, 14);
    fr.put(0, 1);
    fr.put(0, 1); // fixed block size
    fr.put(0b0111, 4); // 16-bit block size - 1 at end of header
    fr.put(0b0000, 4); // sample rate from STREAMINFO
    fr.put(chans.len() as u64 - 1, 4); // independent channels
    let size_code = match bits {
        8 => 0b001,
        16 => 0b100,
        24 => 0b110,
        _ => panic!("unsupported bits"),
    };
    fr.put(size_code, 3);
    fr.put(0, 1);
    fr.put(0, 8); // frame number 0 (UTF-8 coded)
    fr.put(frames as u64 - 1, 16);
    let crc = crc8(&fr.bytes);
    fr.put(u64::from(crc), 8);
    for ch in chans {
        fr.put(0, 1);
        fr.put(0b000001, 6); // VERBATIM
        fr.put(0, 1);
        for &s in ch {
            fr.put((s as u32 as u64) & ((1u64 << bits) - 1), bits);
        }
    }
    fr.align();
    let crc = crc16(&fr.bytes);
    fr.put(u64::from(crc), 16);
    out.extend_from_slice(&fr.bytes);
    out
}

fn flac_fixture(frames: usize, channels: usize, bits: u32, sr: u32) -> (Vec<u8>, Vec<Vec<i32>>) {
    let max = (1i64 << (bits - 1)) as f64;
    let chans: Vec<Vec<i32>> = (0..channels)
        .map(|c| {
            test_signal(frames, c)
                .iter()
                .map(|v| (v * max).round() as i32)
                .collect()
        })
        .collect();
    (encode_flac(&chans, bits, sr), chans)
}

fn rms(x: &[f32]) -> f64 {
    (x.iter().map(|&v| f64::from(v) * f64::from(v)).sum::<f64>() / x.len() as f64).sqrt()
}

// ---------------------------------------------------------------------------

#[test]
fn bundled_ir_loads_without_resampling() {
    let ir = load_ir_file(bundled_ir(), 48_000.0).unwrap();
    assert_eq!(ir.format, IrFormat::Wav);
    assert_eq!(ir.source_sample_rate, 48_000);
    assert_eq!(ir.source_channels, 1);
    assert_eq!(ir.source_frames, 8192);
    assert_eq!(ir.samples.len(), 8192);
    assert!(ir.samples.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
    assert!(ir.samples.iter().any(|s| s.abs() > 0.1));

    let conv = ir.build_convolver().unwrap();
    assert!(conv.is_initialized());
    assert_eq!(conv.head_block_size(), 128);
    assert_eq!(conv.tail_block_size(), 1024);
}

#[test]
fn bundled_ir_matches_raw_24bit_pcm() {
    // Independent decode of the 24-bit data chunk (header is 44 bytes).
    let bytes = std::fs::read(bundled_ir()).unwrap();
    let ir = load_ir_bytes(&bytes, 48_000.0).unwrap();
    for (i, s) in ir.samples.iter().enumerate().take(100) {
        let b = &bytes[44 + 3 * i..44 + 3 * i + 3];
        let v = i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8;
        assert_eq!(*s, v as f32 / 8_388_608.0);
    }
}

#[test]
fn bundled_ir_convolver_reproduces_ir() {
    let ir = load_ir_file(bundled_ir(), 48_000.0).unwrap();
    let mut conv = load_ir_convolver(bundled_ir(), 48_000.0).unwrap();
    let n = 4096;
    let mut input = vec![0.0f32; n];
    input[0] = 1.0;
    let mut output = vec![0.0f32; n];
    conv.process(&input, &mut output);
    for (o, s) in output.iter().zip(&ir.samples) {
        assert!((o - s).abs() < 1e-4, "{o} vs {s}");
    }
}

#[test]
fn bundled_ir_resampled_to_44100_and_96000() {
    let src = load_ir_file(bundled_ir(), 48_000.0).unwrap();
    for target in [44_100.0, 96_000.0] {
        let ir = load_ir_file(bundled_ir(), target).unwrap();
        assert_eq!(ir.sample_rate, target);
        assert_eq!(ir.samples.len(), resampled_len(8192, 48_000.0, target));
        // Energy per second preserved: sum(x^2)/fs is invariant for a
        // band-limited signal (the IR's energy is far below 20 kHz).
        let e_src: f64 = src
            .samples
            .iter()
            .map(|&v| f64::from(v).powi(2))
            .sum::<f64>()
            / 48_000.0;
        let e_dst: f64 = ir
            .samples
            .iter()
            .map(|&v| f64::from(v).powi(2))
            .sum::<f64>()
            / target;
        assert!(
            (e_dst / e_src - 1.0).abs() < 0.05,
            "energy ratio {}",
            e_dst / e_src
        );
        ir.build_convolver().unwrap();
    }
}

#[test]
fn wav_int_bit_depths_decode_exactly() {
    for bits in [8u16, 16, 24, 32] {
        let (_dir, path) = write_wav_int(500, 1, bits, 48_000);
        let ir = load_ir_file(&path, 48_000.0).unwrap();
        assert_eq!(ir.samples.len(), 500);
        let max = (1i64 << (bits - 1)) as f64;
        let expected = test_signal(500, 0);
        for (s, e) in ir.samples.iter().zip(&expected) {
            let q = (e * max).round() / max;
            assert!((f64::from(*s) - q).abs() < 1e-6, "bits {bits}: {s} vs {q}");
        }
    }
}

#[test]
fn wav_float_decodes_exactly() {
    let samples: Vec<f32> = test_signal(300, 0).iter().map(|&v| v as f32).collect();
    let (_dir, path) = write_wav_float(&samples, 1, 44_100);
    let ir = load_ir_file(&path, 44_100.0).unwrap();
    assert_eq!(ir.samples, samples);
    assert_eq!(ir.source_sample_rate, 44_100);
}

#[test]
fn stereo_wav_keeps_first_channel() {
    for bits in [16u16, 24] {
        let (_dir, path) = write_wav_int(400, 2, bits, 48_000);
        let ir = load_ir_file(&path, 48_000.0).unwrap();
        assert_eq!(ir.source_channels, 2);
        assert_eq!(ir.samples.len(), 400);
        let max = (1i64 << (bits - 1)) as f64;
        let left = test_signal(400, 0);
        for (s, e) in ir.samples.iter().zip(&left) {
            assert!((f64::from(*s) - (e * max).round() / max).abs() < 1e-6);
        }
    }
    // Float stereo: left = +0.5, right = -0.25.
    let inter: Vec<f32> = (0..200).flat_map(|_| [0.5f32, -0.25]).collect();
    let (_dir, path) = write_wav_float(&inter, 2, 48_000);
    let ir = load_ir_file(&path, 48_000.0).unwrap();
    assert_eq!(ir.samples, vec![0.5f32; 200]);
}

#[test]
fn flac_bit_depths_mono_and_stereo() {
    for bits in [8u32, 16, 24] {
        for channels in [1usize, 2] {
            let (bytes, chans) = flac_fixture(256, channels, bits, 44_100);
            let ir = load_ir_bytes(&bytes, 44_100.0).unwrap();
            assert_eq!(ir.format, IrFormat::Flac);
            assert_eq!(ir.source_channels as usize, channels);
            assert_eq!(ir.source_sample_rate, 44_100);
            let max = (1i64 << (bits - 1)) as f32;
            let expected: Vec<f32> = chans[0].iter().map(|&v| v as f32 / max).collect();
            assert_eq!(ir.samples, expected, "bits {bits} channels {channels}");
        }
    }
}

#[test]
fn flac_file_on_disk_loads() {
    let (bytes, _) = flac_fixture(1000, 2, 24, 48_000);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ir.flac");
    std::fs::write(&path, bytes).unwrap();
    let ir = load_ir_file(&path, 48_000.0).unwrap();
    assert_eq!(ir.samples.len(), 1000);
    load_ir_convolver(&path, 48_000.0).unwrap();
}

#[test]
fn resample_44100_to_48000_length_and_gain() {
    let n = 44_100;
    let f = 1000.0;
    let input: Vec<f32> = (0..n)
        .map(|i| (0.5 * (2.0 * std::f64::consts::PI * f * i as f64 / 44_100.0).sin()) as f32)
        .collect();
    let (_dir, path) = write_wav_float(&input, 1, 44_100);
    let ir = load_ir_file(&path, 48_000.0).unwrap();
    assert_eq!(ir.samples.len(), 48_001);
    let ratio = ir.samples.len() as f64 / n as f64;
    assert!((ratio - 48_000.0 / 44_100.0).abs() < 1e-3);

    // Steady-state region: compare with the ideal 1 kHz sine at 48 kHz
    // (checks gain and time alignment / delay compensation).
    let mid = &ir.samples[1000..47_000];
    assert!(
        (rms(mid) / (0.5 / 2f64.sqrt()) - 1.0).abs() < 0.01,
        "rms {}",
        rms(mid)
    );
    let max_err = (1000..47_000)
        .map(|i| {
            let ideal = 0.5 * (2.0 * std::f64::consts::PI * f * i as f64 / 48_000.0).sin();
            (f64::from(ir.samples[i]) - ideal).abs()
        })
        .fold(0.0, f64::max);
    assert!(max_err < 1e-3, "max error {max_err}");
}

#[test]
fn resample_impulse_stays_aligned() {
    let mut input = vec![0.0f32; 2000];
    input[100] = 1.0;
    let out = resample(&input, 48_000.0, 96_000.0).unwrap();
    assert_eq!(out.len(), 4001);
    let peak = out
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
        .unwrap()
        .0;
    assert!((199..=201).contains(&peak), "peak at {peak}");
    // DC gain of a 2x upsampled impulse: sum = 2.
    let sum: f64 = out.iter().map(|&v| f64::from(v)).sum();
    assert!((sum - 2.0).abs() < 0.02, "sum {sum}");
}

/// Fit `y[n] = A sin(w (n - d))` over the steady-state region; returns (d, A).
fn sine_lag_and_gain(y: &[f32], f: f64, fs: f64) -> (f64, f64) {
    let w = 2.0 * std::f64::consts::PI * f / fs;
    let range = 2000..y.len() - 2000;
    let n = range.len() as f64;
    let (mut a, mut b) = (0.0, 0.0);
    for i in range {
        a += f64::from(y[i]) * (w * i as f64).sin();
        b += f64::from(y[i]) * (w * i as f64).cos();
    }
    ((-b).atan2(a) / w, 2.0 * (a * a + b * b).sqrt() / n)
}

#[test]
fn resample_common_rates_aligned_with_unity_gain() {
    let f = 997.0;
    for (fi, fo) in [
        (44_100.0, 48_000.0),
        (48_000.0, 44_100.0),
        (96_000.0, 48_000.0),
        (48_000.0, 96_000.0),
        (192_000.0, 44_100.0),
        (22_050.0, 48_000.0),
        (48_000.0, 88_200.0),
    ] {
        let x: Vec<f32> = (0..(fi as usize))
            .map(|i| (0.5 * (2.0 * std::f64::consts::PI * f * i as f64 / fi).sin()) as f32)
            .collect();
        let y = resample(&x, fi, fo).unwrap();
        assert_eq!(y.len(), resampled_len(x.len(), fi, fo));
        let (lag, gain) = sine_lag_and_gain(&y, f, fo);
        assert!(lag.abs() < 0.01, "{fi}->{fo}: lag {lag}");
        assert!((gain / 0.5 - 1.0).abs() < 1e-3, "{fi}->{fo}: gain {gain}");
    }
}

#[test]
fn resample_non_integer_rate_uses_sinc_fallback() {
    let f = 997.0;
    let (fi, fo) = (44_100.0, 47_999.5);
    let x: Vec<f32> = (0..44_100)
        .map(|i| (0.5 * (2.0 * std::f64::consts::PI * f * i as f64 / fi).sin()) as f32)
        .collect();
    let y = resample(&x, fi, fo).unwrap();
    assert_eq!(y.len(), resampled_len(x.len(), fi, fo));
    let (lag, gain) = sine_lag_and_gain(&y, f, fo);
    // The asynchronous sinc resampler may be off by up to one output frame.
    assert!(lag.abs() < 1.1, "lag {lag}");
    assert!((gain / 0.5 - 1.0).abs() < 1e-3, "gain {gain}");
}

#[test]
fn errors() {
    assert!(matches!(
        load_ir_file("does/not/exist.wav", 48_000.0),
        Err(IrError::Io(_))
    ));
    assert!(matches!(
        load_ir_bytes(b"this is not audio at all", 48_000.0),
        Err(IrError::UnknownFormat)
    ));
    assert!(matches!(
        load_ir_bytes(&[], 48_000.0),
        Err(IrError::UnknownFormat)
    ));

    // Truncated WAV / FLAC headers.
    let wav = std::fs::read(bundled_ir()).unwrap();
    assert!(matches!(
        load_ir_bytes(&wav[..20], 48_000.0),
        Err(IrError::Decode(_))
    ));
    let (flac, _) = flac_fixture(64, 1, 16, 48_000);
    assert!(load_ir_bytes(&flac[..30], 48_000.0).is_err());

    // Garbage after a valid magic.
    let mut garbage = b"RIFF\0\0\0\0WAVE".to_vec();
    garbage.extend(std::iter::repeat(0xAB).take(64));
    assert!(load_ir_bytes(&garbage, 48_000.0).is_err());

    // Empty data chunk.
    let (_dir, path) = write_wav_float(&[], 1, 48_000);
    assert!(matches!(load_ir_file(&path, 48_000.0), Err(IrError::Empty)));

    // Invalid target sample rate.
    for sr in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            load_ir_file(bundled_ir(), sr),
            Err(IrError::InvalidSampleRate(_))
        ));
    }
}
