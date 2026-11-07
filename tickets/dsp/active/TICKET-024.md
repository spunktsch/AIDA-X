---
id: TICKET-024
status: active
assignee:
priority: high
created: 2026-11-07
updated: 2026-11-07
---

# AtomicModel may free a retired model on the audio thread

## Description
Found during TICKET-017: if two models are published before the loader collects garbage, `AtomicModel::take_pending` frees the previously retired model on the audio thread (non-realtime-safe deallocation). The plugin now uses its own `Handoff<T>` (`crates/aidadx_plugin/src/handoff.rs`) which postpones the swap while the retired slot is full.

## Acceptance Criteria
- [ ] `aidadx_core` provides one generic lock-free hand-off (move `Handoff<T>` into core or fix `AtomicModel` with the same rule)
- [ ] Audio-side take never allocates or frees in any interleaving
- [ ] Plugin uses the core type; duplicate implementation removed
- [ ] Tests: interleaving/stress test + counting-allocator test on the take path

## Implementation Notes
See TICKET-017 "## Results".

## Checklist
- [ ] Code implemented
- [ ] Tests written
- [ ] Self-review passed
- [ ] PR created
- [ ] Merged
