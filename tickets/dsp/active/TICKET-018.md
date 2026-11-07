---
id: TICKET-018
status: active
assignee:
priority: high
created: 2026-11-07
updated: 2026-11-07
---

# IR file loading (WAV/FLAC) and sample-rate conversion

## Description
Remainder of TICKET-003: the convolver exists but cannot load IR files.

## Acceptance Criteria
- [x] Load mono/stereo WAV (`hound`) and FLAC (`symphonia` or `claxon`) into f32, stereo -> mono as the C++ does
- [ ] Resample IR to host sample rate (`rubato`), matching C++ r8brain behaviour in length/gain
- [x] Silence trimming / normalisation identical to C++ loader (`src/aidadsp-plugin.cpp`)
- [x] Loader API runs off the audio thread and returns a ready-to-use convolver
- [x] Tests with the bundled IRs in `files/`

## Implementation Notes
Pure-Rust crates only (no C deps) to keep iOS/WASM viable.

## Checklist
- [x] Code implemented
- [x] Tests written
- [x] Self-review passed
- [ ] PR created
- [ ] Merged

## Results
Module `aidadx_core::ir` (`crates/aidadx_core/src/ir/`): `load_ir_file(path, target_sr)`,
`load_ir_bytes(bytes, target_sr)` -> `IrData`, `IrData::build_convolver()` and
`load_ir_convolver(path, target_sr)` -> ready `TwoStageConvolver` (128/1024 blocks, as C++).
Lower level: `decode_ir_bytes`, `resample`, `resampled_len`. Errors: `IrError`.
Crates (pure Rust, pinned): `hound =3.5.1`, `claxon =0.4.3`,
`rubato =5.0.1` (`default-features = false, features = ["fft_resampler"]`).

C++ behaviour reproduced (`src/aidadsp-plugin.cpp` `loadCabinetFromFile`/`loadCabinet`, ~l.749-815):
- Multi-channel -> first (left) channel only; no downmix.
- Resample only if file rate != host rate; output length `ceil(n*dst/src)+1`.
- No normalisation and no trimming in the loader; trailing near-zero trimming happens
  in `TwoStageConvolver::init` as in FFTConvolver. The -12 dB `kCabinetMaxGain` is
  applied in `run()`, not in the loader, so it is not part of this module.

Verified by tests (`cargo test -p aidadx_core`: 79 passed, 1 pre-existing ignored):
bundled `files/V30-P2-audix-i5-deerinkstudios.wav` (48 kHz/24-bit/mono/8192 frames) decodes
bit-exactly vs. raw PCM and the convolver reproduces it; WAV int 8/16/24/32 + float32, FLAC
8/16/24 mono/stereo (fixtures generated in-test); 44.1k->48k and 6 other rate pairs keep
length ratio, unity gain (<0.1 %) and time alignment (<0.01 sample); error cases.

### Deviations from C++
- **Stereo length**: the C++ channel loop (`for (i=0,j=0; j<numFrames; ++i, j+=channels)`)
  bounds `j` by frames instead of samples, so stereo IRs are truncated to the first
  `1/channels` of their length. Rust keeps the full left channel (C++ bug not reproduced).
- **Resampler**: rubato instead of r8brain `CDSPResampler16IR`. Integer rates use rubato's
  synchronous FFT resampler (BlackmanHarris2, >=1024-tap, even FFT sizes -> exact alignment);
  non-integer rates fall back to the async sinc resampler (256 taps), which can be up to
  ~1 output frame early. Output length `ceil+1` approximates r8brain `getMaxOutLen(0)`,
  which can be a few frames longer depending on its internal stage chain (not verified
  against r8brain output; trailing frames are flush/near-zero and get trimmed by the convolver).
  Criterion "matching r8brain length/gain" therefore left unticked pending a comparison
  against real r8brain output.
- **Format detection** by magic bytes (`RIFF..WAVE` / `fLaC`) instead of the `.flac`
  extension; RIFX/RF64/W64, 64-bit float, A-law/u-law and ADPCM WAVs (supported by dr_wav)
  are rejected with `IrError::UnsupportedSampleFormat`/`Decode`.
- Default (embedded) cabinet and the convolver hot-swap are plugin concerns, not implemented here.

