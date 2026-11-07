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
- [x] `apply_model` uses `aidadx_core::model::DynamicModel` (RTNeural backend), PARAM1/PARAM2 conditioning, NETBYPASS
- [x] Model swap via `AtomicModel` (no alloc/free on audio thread), reset + 2048-sample prewarm on activate
  (uses `AtomicModel`'s pattern generalised as `aidadx_plugin::handoff::Handoff<T>`, see Results)
- [x] `apply_cabinet` uses `TwoStageConvolver` with the C++ -12 dB compensated crossfade, CABSIMBYPASS audible
- [x] `model_input_size` output reports the real value
- [x] Tests: model changes output, cabinet bypass crossfade, no allocations in process

## Implementation Notes
Depends on TICKET-016 and TICKET-018.

## Checklist
- [x] Code implemented
- [x] Tests written
- [x] Self-review passed
- [ ] PR created
- [ ] Merged

## Results

### Signal chain (`crates/aidadx_plugin/src/audio.rs`)
- Model stage = C++ `applyModel`: input gain, `in_skip`, output gain via
  `DynamicModel::process`. PARAM1/PARAM2 (`LinearValueSmoother`, 100 ms) are fed per sample
  to 2-/3-input models; a smoother only advances when the model consumes it (as in C++).
  While the consumed smoothers are settled the block is processed in one call with constant
  conditioning (bit-identical, avoids per-sample FFI); during a ramp it runs sample by sample.
  `paramFirstRun` is reproduced (smoothers jump to target when a new model is installed).
  NETBYPASS skips the stage.
- Cabinet stage = C++: copy to `cab_in` (`cabsimInplaceBuffer`), `TwoStageConvolver::process`,
  then `out = wet*b + dry*((0.251f - b)/0.251f)` with `b` = CABSIMBYPASS smoother
  (`ExponentialValueSmoother`, T60 100 ms, target 0.251f or 0). Skipped entirely while no IR
  is loaded (smoother not advanced), like `cabsim == nullptr`.
- `reset()` (= `activate()`): clears smoothers/filters and runs `DynamicModel::prewarm`
  (reset + 2048 zeros, stack buffer, no allocation) with the PARAM targets.
- `model_input_size` is updated from the active model at the start of every block
  (0 = "ERROR" while no model is loaded).
- Constants are now float-rounded like C++ (`COMMON_Q = 0.707_f32 as f64`,
  `INLPF_*_CO = (x_f32 * 0.5) as f64`, `CABINET_MAX_GAIN_F32 = 0.251`). The equivalent
  constants in `aidadx_core::biquad` are private, so they are mirrored rather than reused
  (no core change).

### Lock-free hand-off (`crates/aidadx_plugin/src/handoff.rs`)
`Handoff<T>` generalises `aidadx_core::model::AtomicModel` (pending + retired `AtomicPtr`
slots) to any `T` and is used for both `DynamicModel` and `TwoStageConvolver`.
Difference from `AtomicModel`: `AtomicModel::take_pending` frees the previously retired
object *on the audio thread* if two swaps happen before the loader collects. `Handoff::take`
instead defers the swap while the retired slot is occupied, so the audio thread never
allocates or frees under any interleaving. Ownership:
- the loader thread `publish`es (`Box::into_raw` into `pending`; a never-taken pending value is
  freed by the publisher);
- the audio thread `take`s at the start of `process()`; the replaced object goes into
  `retired`;
- the loader thread frees `retired` after each request and every 100 ms (TICKET-019);
  `Drop` frees whatever is left.
`AtomicModel` itself is no longer used by the plugin; it could adopt the deferral rule.

### Tests
- `audio.rs`: engine output equals `DynamicModel::process` bit for bit; NETBYPASS is
  transparent; 3-input model reacts to PARAM1 and PARAM2; a PARAM1 jump is applied as a
  per-sample ramp (bit-identical to a reference loop); `reset()` pre-rolls like `activate()`;
  bypassed cabinet is bit-transparent, enabled cabinet = `conv(x) * 0.251` (< 1e-6), toggling
  CABSIMBYPASS crossfades without a step and settles on the dry signal; `model_input_size`
  0 without a model.
- `handoff.rs`: retire/defer/collect ordering, latest publish wins, 2000-swap
  producer/consumer stress test with drop counting.
- `tests/no_alloc.rs`: counting `#[global_allocator]` (per-thread flag) proves 0 allocations
  and 0 frees in `process()` while: first model + IR are taken, a model/IR swap retires the
  old ones, a swap is deferred (retired slot full) and then completed, PARAM ramps
  (per-sample path), CABSIMBYPASS / NETBYPASS toggles, `reset()` (pre-roll) and an
  oversized host block.
- `tests/realtime_bench.rs` (ignored): full chain, default model + IR, 48 kHz, 128-sample
  blocks, release build: ~40-54 us per block = 1.5-2 % of the 2667 us budget (~50-68x real
  time) on the dev machine (Windows/MSVC).
- `cargo test --workspace`, `cargo build -p aidadx_plugin --release` and
  `cargo clippy -p aidadx_plugin --all-targets` pass.
