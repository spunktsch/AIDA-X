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
- [ ] Load mono/stereo WAV (`hound`) and FLAC (`symphonia` or `claxon`) into f32, stereo -> mono as the C++ does
- [ ] Resample IR to host sample rate (`rubato`), matching C++ r8brain behaviour in length/gain
- [ ] Silence trimming / normalisation identical to C++ loader (`src/aidadsp-plugin.cpp`)
- [ ] Loader API runs off the audio thread and returns a ready-to-use convolver
- [ ] Tests with the bundled IRs in `files/`

## Implementation Notes
Pure-Rust crates only (no C deps) to keep iOS/WASM viable.

## Checklist
- [ ] Code implemented
- [ ] Tests written
- [ ] Self-review passed
- [ ] PR created
- [ ] Merged
