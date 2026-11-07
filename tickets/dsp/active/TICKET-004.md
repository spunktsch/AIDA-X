---
id: TICKET-004
status: active
assignee:
priority: critical
created: 2026-11-01
updated: 2026-11-01
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

## Implementation Notes
- `model_variant.hpp` contains 54 model type definitions using RTNeural templates
- Current approach: compile-time type dispatch via `std::visit` on `std::variant`
- Rust approach: `enum ModelVariant { Gru8, Gru12, ..., Lstm80 }` with dynamic dispatch via `Box<dyn InferenceEngine>`
- `nlohmann::json` -> `serde_json::Value` for parsing
- Parameter inputs PARAM1/PARAM2 must be forwarded to model forward pass
