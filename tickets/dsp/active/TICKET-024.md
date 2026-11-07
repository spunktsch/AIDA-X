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
- [x] `aidadx_core` provides one generic lock-free hand-off (move `Handoff<T>` into core or fix `AtomicModel` with the same rule)
- [x] Audio-side take never allocates or frees in any interleaving
- [x] Plugin uses the core type; duplicate implementation removed
- [x] Tests: interleaving/stress test + counting-allocator test on the take path

## Implementation Notes
See TICKET-017 "## Results".

## Checklist
- [x] Code implemented
- [x] Tests written
- [x] Self-review passed
- [ ] PR created
- [ ] Merged

## Results
- New `aidadx_core::handoff::Handoff<T>` (moved from `aidadx_plugin/src/handoff.rs`, now deleted). API: `new`, `publish(impl Into<Box<T>>)` (collects garbage first, latest wins), `collect_garbage`, `take(&mut Option<Box<T>>) -> bool` (consumer, never allocates/frees; defers the swap while the retired slot is full), `has_pending`, `has_retired`, `#[doc(hidden)] publish_without_gc`.
- `aidadx_core::model::AtomicModel` is now `pub type AtomicModel = Handoff<DynamicModel>` with a `take_pending` alias for `take`; `publish(DynamicModel)` still compiles via `Into<Box<T>>`. The audio-thread free on two publishes before GC is gone.
- **Behaviour change:** a swap deferred behind a full retired slot only completes after the producer side calls `collect_garbage`/`publish`, so producers must collect periodically (the plugin loader already does every `GC_INTERVAL`). `atomic_model_swap_across_threads` now collects in its loop.
- Plugin `audio.rs`/`loader.rs` use the core type; `lib.rs` re-exports `aidadx_core::handoff` as `aidadx_plugin::handoff` so existing paths (e.g. `tests/no_alloc.rs`) keep working.
- Memory ordering reviewed and documented in the `handoff` module docs (table per atomic op): Release on every non-null store / Acquire on every RMW that takes a pointer out; `take`'s pending swap relaxed from AcqRel to Acquire, retired guard load is Relaxed (only the consumer writes non-null to `retired`, coherence guarantees it cannot read older than its own store), retired store is Release.
- Tests: `handoff::tests::stress_many_publishers_one_consumer` (4 publishers x 3000, dedicated collector, one consumer; asserts each value dropped exactly once, no drop on the consumer thread, per-producer order; release run: ~6k swaps, ~4.7k deferred takes); new `aidadx_core/tests/handoff_no_alloc.rs` (counting global allocator: single-thread take/defer/collect, `AtomicModel` two-publishes-before-GC with real model, and a 3-publisher concurrent take loop -> 0 allocs / 0 frees on the consumer).
- `cargo test --workspace` passes; clippy clean for `aidadx_core` + `aidadx_plugin` (`--all-targets`).
