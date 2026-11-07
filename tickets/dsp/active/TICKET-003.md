---
id: TICKET-003
status: active
assignee:
priority: critical
created: 2026-11-01
updated: 2026-11-01
---

# Port FFT convolution cabinet simulator

## Description
Port the two-stage FFT convolution engine from `modules/FFTConvolver/` to `aidadx_core/src/convolver/`. Supports mono/stereo IR loading, resampling, and real-time convolution.

## Acceptance Criteria
- [x] `convolver/` module with `TwoStageConvolver` struct
- [x] Supports WAV and FLAC input files via `hound`/`symphonia`
- [x] Sample-rate conversion via `rubato` crate for IR resampling
- [x] Two-stage partitioned convolution (matching DPF FFTConvolver algorithm)
- [x] Lock-free audio thread interface via `crossbeam` channel
- [x] Unit tests: impulse response accuracy, latency measurement
- [x] Benchmarks: CPU usage vs. C++ baseline

## Implementation Notes
- Current implementation: `FFTConvolver.cpp`, `TwoStageFFTConvolver.cpp`, `AudioFFT.cpp`
- Partition size and overlap-add must match for phase coherence
- `r8brain` resampler port or use `rubato` crate
- Cabinet gain compensation: -12dB (0.251f) baked into convolver bypass crossfade
