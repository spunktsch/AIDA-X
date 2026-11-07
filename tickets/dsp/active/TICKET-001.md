---
id: TICKET-001
status: active
assignee:
priority: critical
created: 2026-11-01
updated: 2026-11-01
---

# Set up Rust Cargo workspace with aidadx_core crate

## Description
Initialize the Rust monorepo structure. The workspace root Cargo.toml and the first crate `aidadx_core` form the foundation for all subsequent DSP work.

## Acceptance Criteria
- [x] Workspace `Cargo.toml` defines all 5 crates (core, ui, plugin, ios, cli)
- [x] `aidadx_core/Cargo.toml` with dependencies: cpal, rustfft, candle, serde_json, crossbeam, rtrb, rubato, parking_lot
- [x] `aidadx_core/src/lib.rs` with module structure: pipeline, biquad, convolver, model, inference, loader, state, params
- [x] `cargo check` passes for workspace root
- [x] Git commit with TICKET-001 prefix

## Implementation Notes
- Use `candle` as the ML inference backend with feature flags for Metal/CUDA
- `rustfft` for SIMD-accelerated FFT
- `cpal` for cross-platform audio I/O
- `crossbeam` for RT-safe channels between audio thread and UI thread
