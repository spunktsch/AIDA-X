---
id: TICKET-022
status: active
assignee:
priority: medium
created: 2026-11-07
updated: 2026-11-07
---

# Golden-reference tests for biquad/tone control against C++

## Description
Biquad port is verified only at formula level, not against real C++ output.

## Acceptance Criteria
- [x] Small C++ harness (built with MSVC/cl or cc) runs the original `src/` biquad + tone control and dumps reference output for a fixed set of parameters and signals
- [x] Reference data committed as test fixtures; Rust tests compare within 1e-5
- [x] Covers all filter types, EQ pre/post, mid type, DC blocker, smoother ramps
- [x] Any mismatch fixed in Rust

## Implementation Notes
SIMD optimisation out of scope (separate ticket if profiling shows need).

## Checklist
- [x] Code implemented
- [x] Tests written
- [x] Self-review passed
- [ ] PR created
- [ ] Merged

## Results

### Harness
- `crates/aidadx_core/tests/cpp_reference/gen_biquad_ref.cpp` compiles `src/Biquad.cpp`
  unchanged and DPF `distrho/extra/ValueSmoother.hpp` unchanged (submodule
  `c1ef624`). The tone-control code of `src/aidadsp-plugin.cpp` sits inside the DPF
  `Plugin` class, so the parts it needs are copied verbatim with file:line citations:
  `DB_CO`/`MAP`/constants (33-49), `AidaToneControl` (56-106), `applyGainRamp`/
  `applyBiquadFilter`/`applyToneControls` (141-179), the tone-control cases of
  `setParameterValue` (504-575), and the `run()` order (1003-1057) without the model and cabinet.
- To build it, run `build_and_run.bat` (MSVC; it calls the VS 2022 Build Tools `vcvarsall` when `cl` is not on PATH) or `build_and_run.sh` (`$CXX`,
  `-ffp-contract=off`). Set `DPF_DIR` if the submodule is not checked out. The script
  writes `tests/fixtures/biquad_golden.bin` (LE f32, 1.4 MB) plus `biquad_golden.idx`
  (text index `name offset len`). The input signals are stored too, so Rust does not
  regenerate them. Regeneration is deterministic (same SHA-256 on rerun).
- `crates/aidadx_core/tests/biquad_golden.rs` replays every scenario through the Rust API
  and fails if a fixture record is not covered. It needs no C++ compiler.

### Coverage (120 records)
- **Filters (63):** all 7 types (LP, HP, BP, notch, peak boost/cut/flat, low shelf
  boost/cut/flat, high shelf boost/cut). Each runs at 2-3 fc/Q/gain settings, including
  in_lpf max and the 35 Hz DC blocker, plus mid-stream `setPeakGain`/`setFc`/`setQ` changes
  (the filter state carries over). Signals: impulse, exponential sweep 20 Hz-20 kHz, and LCG
  white noise, 1024 samples each. The Rust side runs both the per-sample and the block API.
- **Chain (52):** 13 scenarios × {impulse, 0.5 step, sweep, noise}, 4096 samples in
  256-sample blocks. Scenarios: defaults, EQ post and pre with all bands, bandpass mid,
  the MTYPE quirk (MTYPE changes only the routing; the mid biquad type changes at the next
  `sampleRateChanged`), DC blocker off, LPF off/max/automated, EQ bypass, a cold-start gain
  ramp (no `activate()`), 14-step parameter automation across gains/EQ/pos/DC, and 44.1 kHz.
- **Smoothers (5):** exponential T60 1 / 0.25 / 0.1 s with retargets, exponential at
  44.1 kHz after `clearToTargetValue`, and linear tau 0.1 s with a mid-segment retarget and
  an unchanged-target no-op.

### Tolerance
`|rust - cpp| <= 1e-5 * max(1, peak|cpp|)` per record. That is absolute 1e-5 for
unity-level signals (inputs are at most 1.0). Boosted outputs (EQ up to +8 dB, master up
to +15 dB) get the same tolerance relative to their peak, because f32 rounding error grows
with magnitude. **Observed max error after the fixes: 0.0 in all 120 records, in both debug
and release builds (bit-exact on Windows/MSVC).** The 1e-5 margin covers libm `tan`/`pow`/`exp`
differences on other platforms.

### Bugs found and fixed (all in `crates/aidadx_core/src/biquad/`)
1. **Smoothers computed in f64, DPF uses float.** Root cause: `1 - coef` of a 1 s T60
   smoother at 48 kHz is about 1.4e-4. Rounding `coef` to f32 shifts the time constant
   by about 0.04 %, so the f64 port's ramps drifted from C++: 6.2e-5 (T60 1 s),
   6.3e-4 (44.1 kHz from -6 to +12 dB), 6.1e-5 for the linear smoother, and 2.7e-4 in
   the `automation` chain. That is above 1e-5 in 4 of the 5 smoother records and 3 of the
   chain records. Fix: `ExponentialValueSmoother`/`LinearValueSmoother` now keep all state
   in f32, with float `d_isNotEqual`, `(float)(1.0/6.91)` and f32 `exp`. The public API stays f64.
   `next_f32()` was added so ramps can skip the f64 round trip.
2. **`db_to_coeff`/`map` computed in f64, C++ `DB_CO`/`MAP` use float.** This changed gain
   targets and the in_lpf cutoff by about 1 ulp. Both now compute in f32 (signature unchanged).
3. **Constants and defaults not rounded to float.** `COMMON_Q` was `0.707` instead of
   `0.707f` (0.7070000171661377), as were `INLPF_MAX/MIN_CO` and the `CascadeParameters`
   defaults `mid_q` (0.707f) and `in_lpf` (66.216f). The default-mid_q case gave errors of
   up to 4.8e-7 (mid_type_switch_quirk, sr44100_all_bands). Fixed.
4. **Hardening, no measured effect:** the low-pass `a1`/`a2` now use the same evaluation
   order as `Biquad.cpp` (`2 * a0`, `a0`). `INLPF` enable uses `d_isNotZero` semantics
   (`|v| >= FLT_EPSILON` instead of `> 1e-10`).

No behavioural change in the filter structure itself: the biquad topology, the coefficient
formulas and the cascade order were already correct (the filter records were bit-exact
before the fixes).

Also fixed: all clippy warnings in `biquad/`.
