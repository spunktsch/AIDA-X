---
id: TICKET-002
status: active
assignee:
priority: critical
created: 2026-11-01
updated: 2026-11-07
---

# Port biquad filter engine from C++ to Rust

## Description
Port the biquad filter cascade from `src/Biquad.h`/`src/Biquad.cpp` to `aidadx_core/src/biquad/`. Must support all 7 filter types: HPF, LPF, BPF, notch, peak, low-shelf, high-shelf.

## Acceptance Criteria
- [x] `biquad/` module with types for all 7 filter types
- [x] `BiquadCascade` struct that chains multiple filters
- [ ] SIMD-safe processing using `rustfft` vectorization or inline assembly
- [x] Unit tests for each filter type (sine sweep, impulse response)
- [ ] Matches C++ output within floating-point tolerance (1e-5)
- [x] `AidaToneControl` state machine ported (eq_pos, mid_type, smoothers)

## Checklist
- [x] Code implemented
- [x] Tests written
- [x] Self-review passed
- [ ] PR created
- [ ] Merged

## Implementation Notes
- Current C++ uses `bq_type_*` enum (0-6)
- Filter chain: dc_blocker -> in_lpf -> bass -> mid -> treble -> depth -> presence
- `ExponentialValueSmoother` and `LinearValueSmoother` from DPF need porting
- Q values and cutoff frequencies parameterized via `parameters[]` array

## Audit 2026-11-07
Integrated on `integration/rust-core` (commits 4e7cde4, 5379502). Original commit b3ab656 had 14 test compile errors and 7 failing tests; root causes: inverted exponential-smoother coefficient, fixed-step linear smoother, zeroed CascadeParameters defaults (NaN), per-block instead of per-sample gain ramp, DC blocker default off. Now ported from DPF `ValueSmoother.hpp` / `aidadsp-plugin.cpp`; 46 biquad tests pass.
Not met: no SIMD path; no numeric comparison against real C++ output (only formula-level verification).
