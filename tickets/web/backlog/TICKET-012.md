---
id: TICKET-012
status: backlog
assignee:
priority: medium
created: 2026-11-01
updated: 2026-11-07
---

# Implement WebAssembly (WASM) target with WebGPU rendering

## Description
Port AIDA-X to the web via WebAssembly. Uses egui compiled to WASM + wgpu WebGPU backend. Audio via Web Audio API or script processor node.

## Acceptance Criteria
- [ ] WASM target compiles: `cargo build --target wasm32-unknown-unknown --release`
- [ ] egui + wgpu renders pedalboard in browser canvas
- [ ] Web Audio API for low-latency audio streaming
- [ ] Drag-and-drop file loading for model JSON and IR WAV/FLAC
- [ ] Bundle size optimized with `wasm-opt` (< 5MB gzipped target)
- [ ] Works in Chrome, Firefox, Safari (latest 2 versions)
- [ ] Falls back to CPU inference when WebGPU unavailable
- [ ] Matches current MOD platform deployment behavior

## Implementation Notes
- Current WASM deployment uses DPF's Emscripten build with custom Makefile in `src/standalone/`
- Audio: `cpal` has WASM support via `webaudio` backend, or use `wasm-bindgen` for direct Web Audio API
- SIMD: WebAssembly SIMD instruction set available in modern browsers (-msimd128)
- No file system access: models/IRs must be loaded as typed arrays from user upload
- MOD platform variant has special constraints (no keyboard/mouse wheel input)

## Checklist
- [ ] Code implemented
- [ ] Tests written
- [ ] Self-review passed
- [ ] PR created
- [ ] Merged
