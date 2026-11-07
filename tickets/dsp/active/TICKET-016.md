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
- [x] New crate `crates/aidadx_rtneural` with `build.rs` (cc crate) compiling a C++ shim against `modules/rtneural` (Eigen backend, as CMakeLists.txt) and reusing `src/model_variant.hpp` (54 compile-time model types)
- [x] C ABI: create from model JSON, process block (input + param1/param2), reset, input size, destroy; no allocation in process
- [x] Safe Rust wrapper (`Send`, Drop) in the crate
- [x] `aidadx_core::model` uses RTNeural as default backend; candle stays behind `candle` feature (offline/CLI)
- [x] `DynamicModel` API unchanged (`process`, `reset`, `prewarm`, gain/skip semantics)
- [x] Tests: all 54 variants match the existing scalar RTNeural reference; real model matches its `output_batch`
- [x] Throughput >= 20x realtime for LSTM_12_1 (release), numbers recorded in ticket

## Implementation Notes
Builds on Windows (MSVC) and Linux; macOS/iOS/WASM noted as follow-up. Submodule must be initialised (`git submodule update --init --recursive modules/rtneural`).

## Checklist
- [x] Code implemented
- [x] Tests written
- [x] Self-review passed
- [ ] PR created
- [ ] Merged

## Results

### Design
- `crates/aidadx_rtneural`: `build.rs` (cc, C++17, `/bigobj /EHsc` on MSVC) compiles
  `cpp/shim.cpp` + `cpp/parts/part{0..5}.cpp`. Include paths: `modules/rtneural`,
  `modules/rtneural/modules/Eigen`, repo `src/`. Defines `RTNEURAL_USE_EIGEN=1`,
  `RTNEURAL_DEFAULT_ALIGNMENT=16` (as CMake with XSIMD/AVX off); `EIGEN_DONT_VECTORIZE=1`
  on wasm32 (as CMake for Emscripten). C++ is always built at `-O2`, also in debug profiles.
  `RTNEURAL_DIR` env var overrides the RTNeural location (default: in-repo submodule).
- `src/model_variant.hpp` is **included, not copied** (`ModelVariantType`, `custom_model_creator`).
  The heavy per-model templates (`parseJson` + forward loop) are declared in `cpp/shim_impl.hpp`
  and explicitly instantiated 9 models per `parts/*.cpp`, compiled in parallel
  (single TU: 6 min 13 s cold build; split: ~2 min 50 s debug / ~2 min 10 s release on this machine).
- C ABI (`extern "C"`, all `noexcept`, every exception caught in the shim):
  `aidadx_rt_create(json, len, err_buf, err_len) -> handle|NULL`, `aidadx_rt_variant_index`,
  `aidadx_rt_input_size`, `aidadx_rt_reset`, `aidadx_rt_process(h, in, out, n, p1, p2)`
  (raw forward only, `in` may alias `out`, no allocation), `aidadx_rt_destroy`.
  The shim validates all weight shapes before calling RTNeural's loaders, which trust the JSON.
- Safe wrapper `aidadx_rtneural::Model` (`Send`, not `Sync`, `Drop`): `from_json`, `input_size`,
  `variant_index`, `reset`, `process`, `process_in_place`, `forward`.
- `aidadx_core`: feature `rtneural` (default) -> `DynamicModel` runs on RTNeural; `candle` only
  -> candle; neither -> parsing/detection/`validate_network` only (DynamicModel/AtomicModel not
  compiled). `model::BACKEND` names the active backend. Gain/skip stay in Rust (skip path uses a
  128-sample stack scratch, no allocation). Shape validation moved from `inference.rs` to
  `loader::validate_network` (shared by both backends). New `ModelError::Backend(String)`.

### Tests (Windows 11, MSVC, rustc 1.99)
- `cargo test --workspace`: pass (aidadx_core 79 + 2 ignored, aidadx_rtneural 3, aidadx_plugin 10).
- `cargo test -p aidadx_core --features candle`: 80 pass. `--no-default-features`: 6 model tests pass.
  `--no-default-features --features candle`: 15 model tests pass.
- All 54 variants vs scalar RTNeural reference: < 1e-4 per sample and as one block.
- Real model `tw40_california_clean_deerinkstudios.json`: max abs diff 2.4e-7 vs reference,
  2.2e-7 vs its `output_batch`.
- Clippy: no warnings in `aidadx_rtneural` or `model/` (pre-existing warnings in `biquad/` only).

### Throughput (release, block 128, 5 s @ 48 kHz, `model::tests::backend::throughput`)
| model | RTNeural smp/s | x realtime | scalar Rust ref x RT |
|---|---:|---:|---:|
| LSTM_12_1 (tw40 real) | 2,042,717 | **42.6** | 15.7 |
| LSTM_12_1 | 1,879,069 | 39.1 | 18.8 |
| GRU_8_1 | 2,858,909 | 59.6 | 41.5 |
| GRU_40_1 | 829,929 | 17.3 | 4.1 |
| LSTM_40_1 | 557,891 | 11.6 | 2.9 |
| GRU_80_1 | 320,973 | 6.7 | 1.2 |
| LSTM_80_1 | 196,475 | 4.1 | 0.8 |

The test asserts >= 20x for LSTM_12_1 in release builds with the RTNeural backend.

### Deviations / open points
- `DynamicModel: Clone` re-creates the network from its (re-serialised) JSON, so on the RTNeural
  backend a clone starts with **reset** state (RTNeural `ModelT` holds `Eigen::Map`s into itself and
  is not safely copyable). Fine for current uses (cloning before running).
- `DynamicModel.inference` (public field, candle) is replaced by a private backend field; no
  in-tree users.
- Only verified on Windows/MSVC. Linux should work (no platform code) but is untested here.
  macOS/iOS need a C++17 toolchain via cc (Xcode). WASM needs a C++ cross toolchain for
  `wasm32-*` (e.g. wasi-sdk/emscripten) since cc can't use MSVC/clang-cl there; untested.
- Default build of `aidadx_core` (and thus `aidadx_plugin`) now needs a C++17 compiler and the
  `modules/rtneural` submodule (clear panic message from `build.rs` otherwise).
