---
id: TICKET-017
status: active
assignee:
priority: high
created: 2026-11-07
updated: 2026-11-07
---

# Wire neural model and cabinet IR stages into the plugin

## Description
`AidaEngine::apply_model` and `apply_cabinet` in `aidadx_plugin` are pass-through placeholders.

## Acceptance Criteria
- [ ] `apply_model` uses `aidadx_core::model::DynamicModel` (RTNeural backend), PARAM1/PARAM2 conditioning, NETBYPASS
- [ ] Model swap via `AtomicModel` (no alloc/free on audio thread), reset + 2048-sample prewarm on activate
- [ ] `apply_cabinet` uses `TwoStageConvolver` with the C++ -12 dB compensated crossfade, CABSIMBYPASS audible
- [ ] `model_input_size` output reports the real value
- [ ] Tests: model changes output, cabinet bypass crossfade, no allocations in process

## Implementation Notes
Depends on TICKET-016 and TICKET-018.

## Checklist
- [ ] Code implemented
- [ ] Tests written
- [ ] Self-review passed
- [ ] PR created
- [ ] Merged
