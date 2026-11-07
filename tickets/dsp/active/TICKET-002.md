---
id: TICKET-002
status: active
assignee:
priority: critical
created: 2026-11-01
updated: 2026-11-01
---

# Port biquad filter engine from C++ to Rust

## Description
Port the biquad filter cascade from `src/Biquad.h`/`src/Biquad.cpp` to `aidadx_core/src/biquad/`. Must support all 7 filter types: HPF, LPF, BPF, notch, peak, low-shelf, high-shelf.

## Acceptance Criteria
- [x] `biquad/` module with types for all 7 filter types
- [x] `BiquadCascade` struct that chains multiple filters
- [x] SIMD-safe processing using `rustfft` vectorization or inline assembly
- [x] Unit tests for each filter type (sine sweep, impulse response)
- [x] Matches C++ output within floating-point tolerance (1e-5)
- [x] `AidaToneControl` state machine ported (eq_pos, mid_type, smoothers)

## Checklist
- [x] Code implemented
- [x] Tests written
- [x] Self-review passed
- [ ] PR created (requires authentication)
- [ ] Merged

## Implementation Notes
- Current C++ uses `bq_type_*` enum (0-6)
- Filter chain: dc_blocker -> in_lpf -> bass -> mid -> treble -> depth -> presence
- `ExponentialValueSmoother` and `LinearValueSmoother` from DPF need porting
- Q values and cutoff frequencies parameterized via `parameters[]` array
