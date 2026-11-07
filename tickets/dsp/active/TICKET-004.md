---
id: TICKET-004
status: active
assignee:
priority: critical
created: 2026-11-01
updated: 2026-11-07
---

# Implement neural model JSON loader with candle inference

## Description
Port the dynamic model loader from `src/model_variant.hpp` and `src/aidadsp-plugin.cpp` to `aidadx_core/src/model/`. Supports all 54 existing architectures (GRU/LSTM x 9 hidden sizes x 3 input sizes).

## Acceptance Criteria
- [x] `model/` module with `DynamicModel` wrapper type (std::variant in Rust = enum)
- [x] JSON schema parser matching current format: `in_shape`, `in_skip`, `in_gain`, `out_gain`, `layers`
- [x] Runtime model type detection via `custom_model_creator` equivalent
- [x] Candle-backed inference for GRU and LSTM layers
- [x] Input skip support (model output added to bypass signal)
- [x] Input/output gain compensation
- [x] Thread-safe model swap via `AtomicPtr<DynamicModel>`
- [x] Unit tests: load all 54 model variants from JSON, verify forward pass matches baseline
- [x] Real-time capable, allocation-free inference (RTNeural backend, 42.6x realtime, TICKET-016)

## Implementation Notes
- `model_variant.hpp` contains 54 model type definitions using RTNeural templates
- Current approach: compile-time type dispatch via `std::visit` on `std::variant`
- Rust approach: `enum ModelVariant { Gru8, Gru12, ..., Lstm80 }` with dynamic dispatch via `Box<dyn InferenceEngine>`
- `nlohmann::json` -> `serde_json::Value` for parsing
- Parameter inputs PARAM1/PARAM2 must be forwarded to model forward pass

## Audit 2026-11-07
Commit 32b4247 never compiled with `--features candle` (65 errors) and its math was wrong (state reset every sample, wrong gate order/weight layout for GRU+LSTM, wrong Dense orientation, broken skip/gain). Rewritten in a9e71e2 and integrated on `integration/rust-core`: matches a scalar RTNeural transcription for all 54 variants (1e-4) and the real model `files/tw40_california_clean_deerinkstudios.json` against its exported output_batch (1e-3).
**Blocker:** candle runs at only ~1x realtime (~15-20 us/sample fixed overhead) and allocates per op. Needs a hand-written allocation-free matvec backend (RTNeural `*LayerT` style); public API (`DynamicModel::process`, `reset`) can stay. No real GRU model file available for external GRU validation.

## Checklist
- [x] Code implemented
- [x] Tests written
- [x] Self-review passed
- [ ] PR created
- [ ] Merged

## Review 2026-11-07
All criteria met; candle is now an optional offline backend.
