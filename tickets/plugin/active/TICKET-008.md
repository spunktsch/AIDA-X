---
id: TICKET-008
status: active
assignee:
priority: high
created: 2026-11-01
updated: 2026-11-07
---

# Implement VST3 and CLAP plugin backends

## Description
Create plugin entry points for VST3 and CLAP formats. The core audio pipeline from `aidadx_core` is format-agnostic; this ticket wraps it in the required plugin framework.

## Acceptance Criteria
- [x] `aidadx_plugin` crate with VST3 and CLAP features
- [x] VST3 implementation (nih-plug; originally specified `aurix`, which does not exist)
- [x] CLAP implementation (nih-plug; originally specified `clap-ffi`, which does not exist)
- [ ] Plugin label: "AIDA_X", unique ID: "aida"
- [x] Brand: "AIDA DSP", URI: "http://aidadsp.cc/plugins/aidadsp-bundle/rt-neural-loader"
- [ ] All 24 parameters initialized with correct names, units, ranges, enums
- [ ] State serialization/deserialization (JSON format matching DPF current format)
- [x] Sample rate and buffer size change callbacks
- [x] Audio input/output port configuration (1 in / 1 out, optional stereo 2/2)
- [x] CLAP features: "audio-effect", "multi-effects", "mono"
- [x] VST3 categories: "Fx|Dynamics|Mono"
- [ ] Built and tested in a DAW (Reaper, Bitwig, or similar)

## Implementation Notes
- Current DPF defines plugin info via macros in `DistrhoPluginCommon.hpp`
- VST3 SDK is large (~500MB extracted); consider using `aurix` crate as abstraction layer
- CLAP is simpler; prefer starting with CLAP first for faster iteration
- Parameter enumeration must match DPF exactly for state compatibility

## Audit 2026-11-07
Original code targeted non-existent crates and never compiled. Rebuilt on nih-plug rev de421011 (commit 145a8e3), integrated on `integration/rust-core`. Release DLL exports `clap_entry`/`GetPluginFactory`; 10 tests pass.
Not met / deviations: only 21 host params (ModelInSize/MeterIn/MeterOut are not host params in nih-plug); VST3 class ID and param hashing differ from DPF, so existing projects/automation are not compatible; state is nih-plug format (model/IR paths under DPF keys `json`/`cabinet`). Model and cabinet stages are pass-through TODOs (convolver now exists in core; model needs realtime backend, see TICKET-004); file loading only logs paths. No DAW / clap-validator / pluginval run yet.
