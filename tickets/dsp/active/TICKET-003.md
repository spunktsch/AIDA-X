---
id: TICKET-003
status: active
assignee:
priority: critical
created: 2026-11-01
updated: 2026-11-07
---

# Port FFT convolution cabinet simulator

## Description
Port the two-stage FFT convolution engine from `modules/FFTConvolver/` to `aidadx_core/src/convolver/`. Supports mono/stereo IR loading, resampling, and real-time convolution.

## Acceptance Criteria
- [x] `convolver/` module with `TwoStageConvolver` struct
- [x] Supports WAV and FLAC input files (`hound` + `claxon`, TICKET-018)
- [x] Sample-rate conversion via `rubato` crate for IR resampling (TICKET-018)
- [x] Two-stage partitioned convolution (matching DPF FFTConvolver algorithm)
- [x] Lock-free audio thread interface (`aidadx_core::handoff::Handoff` instead of crossbeam, TICKET-024)
- [x] Unit tests: impulse response accuracy, latency measurement
- [ ] Benchmarks: CPU usage vs. C++ baseline

## Implementation Notes
- Current implementation: `FFTConvolver.cpp`, `TwoStageFFTConvolver.cpp`, `AudioFFT.cpp`
- Partition size and overlap-add must match for phase coherence
- `r8brain` resampler port or use `rubato` crate
- Cabinet gain compensation: -12dB (0.251f) baked into convolver bypass crossfade

## Audit 2026-11-07
Integrated on `integration/rust-core` (commit 46c9f07). Old branch never compiled (pffft_rust build failure, typo'd dep, borrow errors, per-block allocations). Rewritten as faithful port of `modules/FFTConvolver` on `realfft` 3.5.1; allocation-free `process`, verified against naive convolution across many IR/block sizes; ~650x realtime (500 ms IR).
Not met: WAV/FLAC loading, rubato resampling, crossbeam audio-thread interface, C++ benchmark comparison. Differences: tail1 stage runs synchronously; any IR length accepted.

## Checklist
- [x] Code implemented
- [x] Tests written
- [x] Self-review passed
- [ ] PR created
- [ ] Merged

## Review 2026-11-07
Remaining: benchmark vs. C++ FFTConvolver (Rust: ~650x realtime, 500 ms IR; no C++ number yet).
