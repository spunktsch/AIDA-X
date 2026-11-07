---
id: TICKET-005
status: active
assignee:
priority: high
created: 2026-11-01
updated: 2026-11-01
---

# Implement full audio processing pipeline and parameter system

## Description
Wire together biquad, convolver, and neural model into a single processing graph. Implement the complete audio callback matching `aidadsp-plugin.cpp::run()`. Port all 24 parameters with smoothing.

## Acceptance Criteria
- [ ] `pipeline/` module with `AudioGraph` builder
- [ ] Complete signal chain: input LPF -> pre-gain -> pre-EQ -> model -> DC blocker -> cab -> post-EQ -> output gain -> bypass crossfade
- [ ] All 24 parameters mapped with correct ranges and enums
- [ ] Parameter smoothing (exponential for gains, linear for frequency)
- [ ] Dual output channel support (stereo AU)
- [ ] Bypass crossfade with gain ramp (0.25s time constant)
- [ ] Peak meter calculation (60fps max, color-coded: green/yellow/red)
- [ ] Integration test: full signal chain processes buffer without dropouts

## Implementation Notes
- Parameter indices 0-23 map to current DPF parameters
- `kParameterModelInputSize`, `kParameterMeterIn`, `kParameterMeterOut` are output-only (read from plugin)
- Bypass uses `bypassGain` exponential smoother with 0.25s time constant
- Meter uses peak detection at 60fps (16.667ms frames at 44.1kHz = ~736 samples)
- MOD_BUILD conditional compiles must be handled (web variant)
