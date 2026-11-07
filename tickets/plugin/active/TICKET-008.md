---
id: TICKET-008
status: active
assignee:
priority: high
created: 2026-11-01
updated: 2026-11-01
---

# Implement VST3 and CLAP plugin backends

## Description
Create plugin entry points for VST3 and CLAP formats. The core audio pipeline from `aidadx_core` is format-agnostic; this ticket wraps it in the required plugin framework.

## Acceptance Criteria
- [x] `aidadx_plugin` crate with VST3 and CLAP features
- [x] VST3 implementation using aurix
- [x] CLAP implementation using clap-ffi with manual host integration
- [x] Plugin label: "AIDA_X", unique ID: "aida"
- [x] Brand: "AIDA DSP", URI: "http://aidadsp.cc/plugins/aidadsp-bundle/rt-neural-loader"
- [x] All 24 parameters initialized with correct names, units, ranges, enums
- [x] State serialization/deserialization (JSON format matching DPF current format)
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
