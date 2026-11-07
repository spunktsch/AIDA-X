---
id: TICKET-019
status: active
assignee:
priority: high
created: 2026-11-07
updated: 2026-11-07
---

# Non-realtime file loading, state restore and default model/IR

## Description
`load_files_from_state` only logs the paths.

## Acceptance Criteria
- [x] Loading model/IR on a background thread, lock-free hand-off to audio thread
- [x] State restore loads the saved `json`/`cabinet` paths
- [x] Bundled default model + IR (from `files/`) embedded via `include_bytes!` and used for `"default"`/missing files
- [x] Load errors are reported, never crash or block audio

## Implementation Notes
Depends on TICKET-017. Coordinate with TICKET-014 (asset bundle).

## Checklist
- [x] Code implemented
- [x] Tests written
- [x] Self-review passed
- [ ] PR created
- [ ] Merged

## Results

### Loader (`crates/aidadx_plugin/src/loader.rs`)
- `FileLoader` owns one `std::thread` ("aidax-loader") fed by an `mpsc` channel. nih-plug's
  `BackgroundTask` was not used: its tasks must not carry heap data (paths), and
  `InitContext::execute` runs synchronously.
- The thread parses/decodes/resamples (`DynamicModel::from_json`, `load_ir_file`/
  `load_ir_bytes` + `build_convolver`), pre-warms models (reset + 2048 zeros with the
  current PARAM1/PARAM2, as `loadModelFromStream`), and publishes into the engine's
  `Handoff` slots (TICKET-017). It frees objects retired by the audio thread after every
  request and every 100 ms; on shutdown (`Drop` disconnects the channel and joins).
- Bursts are coalesced (latest model/IR request wins). Non-forced requests equal to the last
  one are skipped, so repeated `initialize()` calls are cheap: the model reloads only when
  its path changes, the IR when its path or the sample rate changes.
- Bundled defaults: `DEFAULT_MODEL_JSON` / `DEFAULT_CABINET_WAV` via `include_bytes!` from
  `files/`. `""`/`"default"` select them (`FileSource::from_state`, same rule as C++
  `setState`).
- Errors (missing file, bad JSON, unsupported architecture, undecodable IR) are logged with
  `nih_error!` and kept in `FileLoader::last_error()` for a future editor. Like C++, a failed
  load keeps the active object; if there is none yet (model), or the active IR was prepared
  for a different sample rate (cabinet), the bundled default is loaded instead.

### Plugin glue (`crates/aidadx_plugin/src/lib.rs`)
- `initialize()` (nih-plug calls it after a state restore and on sample-rate/buffer
  changes) prepares the engine and queues the persisted `json`/`cabinet` paths at the host
  rate: state restore and sample-rate re-preparation of the IR (C++ `sampleRateChanged`
  reloading `cabsimFilename`). It never waits for the loader.
- `AidaX::set_model_file` / `set_cabinet_file` (= `setState("json"/"cabinet")` for a
  future editor) persist the path and queue a forced load.
- Until the first objects arrive the corresponding stage is skipped (dry), as in C++ when
  `model`/`cabsim` is null.

### Tests
- `loader.rs`: defaults decode; real files load in the background and are installed by
  `process()`; missing model/IR on first load -> error reported + defaults; failing later
  load keeps the current model/IR; duplicate requests skipped; sample-rate change reloads
  only the IR; missing IR at a new rate -> default at that rate; forced reload; retired
  objects freed by the loader thread.
- `lib.rs`: state restore with real paths, `"default"`/empty and nonexistent paths through
  `load_files_from_state`; `set_*_file` persist + load + report.
- `cargo test --workspace`, `cargo build -p aidadx_plugin --release`, clippy clean.

### Open points
- No editor yet: `set_*_file` and `last_error()` have no UI caller.
- Paths are stored as given; relative paths / moved projects are not resolved (TICKET-014).
