---
id: TICKET-016
status: active
assignee:
priority: critical
created: 2026-11-07
updated: 2026-11-07
---

# RTNeural FFI backend for real-time neural inference

## Description
Candle inference (TICKET-004) runs at ~1x realtime and allocates per op, unusable for audio. No RTNeural crate exists for Rust, so wrap the C++ RTNeural already vendored in `modules/rtneural` (same engine as the C++ plugin) via a small C ABI shim.

## Acceptance Criteria
- [ ] New crate `crates/aidadx_rtneural` with `build.rs` (cc crate) compiling a C++ shim against `modules/rtneural` (Eigen backend, as CMakeLists.txt) and reusing `src/model_variant.hpp` (54 compile-time model types)
- [ ] C ABI: create from model JSON, process block (input + param1/param2), reset, input size, destroy; no allocation in process
- [ ] Safe Rust wrapper (`Send`, Drop) in the crate
- [ ] `aidadx_core::model` uses RTNeural as default backend; candle stays behind `candle` feature (offline/CLI)
- [ ] `DynamicModel` API unchanged (`process`, `reset`, `prewarm`, gain/skip semantics)
- [ ] Tests: all 54 variants match the existing scalar RTNeural reference; real model matches its `output_batch`
- [ ] Throughput >= 20x realtime for LSTM_12_1 (release), numbers recorded in ticket

## Implementation Notes
Builds on Windows (MSVC) and Linux; macOS/iOS/WASM noted as follow-up. Submodule must be initialised (`git submodule update --init --recursive modules/rtneural`).

## Checklist
- [ ] Code implemented
- [ ] Tests written
- [ ] Self-review passed
- [ ] PR created
- [ ] Merged
