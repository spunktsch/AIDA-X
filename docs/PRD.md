# PRD: AIDA-X Multi-Platform Rust Rewrite

## 1. Overview

AIDA-X is a guitar amp/cabinet neural-model audio plugin (currently C++/DPF/RTNeural). This project rewrites the entire codebase in Rust to achieve true multi-platform deployment including desktop (Windows, macOS, Linux) and mobile (iOS, Android), with a modern reactive UI, while preserving full backward compatibility with existing `.json` neural models and `.wav`/`.flac` IR files.

Current state: C++17, DPF framework, OpenGL/NanoVG UI, RTNeural inference, GRU/LSTM models up to 80 hidden units, 3 input channels, FFT convolution cabinet simulation. Target: Rust, single unified binary family, iOS-native rendering.

## 2. Goals

1. **Multi-platform binary targets**: Windows (x86_64, ARM64), macOS (x86_64, Apple Silicon), Linux (x86_64, ARM64, RISC-V), iOS (ARM64), Android (ARM64, ARM32), WebAssembly (WASM + WebGPU).
2. **All plugin formats**: VST3, AU, AUv3 (iOS), CLAP, LV2, AAX (via AAX SDK), Standalone, Web.
3. **Rust audio engine**: Realtime-safe DSP pipeline with zero-cost abstractions, SIMD auto-vectorization, and safe memory through Rust ownership.
4. **Modern UI**: Reactive framework with hardware-accelerated rendering, dark/light themes, resizable pedalboard layout, per-format native look (NSView on macOS, Win32/DX on Windows, UIKit on iOS).
5. **Neural model compatibility**: Full support for all current RTNeural model architectures (GRU/LSTM, hidden 8-80, input 1-3) plus extensible architecture for future layer types.
6. **IR loading**: WAV, FLAC, MP3 (decode at load time) with sample-rate conversion via `ringbuf` + `rubato`-style resampler in Rust.
7. **CI/CD automation**: GitHub Actions matrix builds all targets, runs pluginval, signs macOS binaries, produces distributable packages.

## 3. Non-Goals

- Rewriting the neural model training pipeline (Colab notebooks remain unchanged).
- AAX support on iOS (AAX does not target mobile).
- Replacing the default bundled models/IRs.
- MIDI control surface integration (future enhancement).

## 4. Architecture

### 4.1 High-Level Component Map

```
aidadx_core/           -- Rust library crate: audio engine, DSP, model inference
aidadx_ui/             -- Rust library crate: UI abstraction (wgpu rendering)
aidadx_plugin/         -- Rust binary crate: plugin entry points per format (vst3, au, clap, lv2, standalone)
aidadx_ios/            -- iOS native binary: AUv3 host + standalone app
aidadx_cli/            -- CLI tool: model validation, batch processing, export utilities
```

### 4.2 Core Crate (`aidadx_core`)

**DSP Engine**
- `pipeline/` -- Audio processing graph builder. Chains: input LPF -> pre-gain -> tone controls -> EQ -> neural model -> DC blocker -> cabinet IR -> post-EQ -> output gain -> bypass crossfade.
- `biquad/` -- Biquad filter cascade (all current filter types: HPF, LPF, BPF, notch, peak, low-shelf, high-shelf). Uses `realfft` for SIMD-safe SIMD.
- `convolver/` -- Two-stage FFT convolution engine. Supports mono/stereo IRs with runtime resampling. Uses `candle`-compatible FFT backend or `rustfft`.
- `resampler/` -- Sample-rate conversion using `rubato` crate for high-quality offline/online conversion.

**Neural Engine**
- `model/` -- Dynamic model loader. Parses JSON from `serde_json`, dispatches to correct architecture via runtime type detection.
- `inference/` -- GRU/LSTM inference using `candle` (HuggingFace Rust ML library) for cross-platform neural execution with Metal/NPU/AVX support.
- `arch/` -- Architecture registry. Each layer type is a registered module. Extensible via compile-time feature flags.

**Model File I/O**
- `loader/` -- Loads models from embedded resources (Rust `include_bytes!`) or filesystem paths. Thread-safe swap with lock-free atomic pointer for real-time model switching.

**State Management**
- `state/` -- Plugin state serialization/deserialization. Supports JSON format matching current DPF state schema for backward compatibility.
- `params/` -- Parameter system with automatable values, smooth value interpolation via exponential/linear smoothers.

### 4.3 UI Crate (`aidadx_ui`)

**Rendering Backend**
- `renderer/` -- `wgpu`-based hardware-accelerated rendering. Single draw call per pedalboard frame with batched geometry.
- `shaders/` -- GLSL/HLSL/Metal shader files for pedalboard rendering, knob interpolation, meter visualization.

**Widget Library**
- `widgets/knob/` -- Interactive rotary knob with drag, scroll, double-click reset. Scales to DPI.
- `widgets/switch/` -- Toggles with on/off states (bypass, eqpos, midtype).
- `widgets/meter/` -- Peak meters with color gradient (green -> yellow -> red). 60fps update from audio thread via crossbeam channel.
- `widgets/filebtn/` -- File loader buttons with toggle icon and filename display.
- `widgets/splitter/` -- Decorative divider lines.

**Layout System**
- `layout/pedalboard/` -- Fixed pedalboard shape (900x318 base) with margin-aware positioning. Responsive scaling per DPI.
- `layout/responsive/` -- Mobile layout adapter for iOS/Android screens (portrait mode, touch-optimized sizes).

**Theming**
- `theme/` -- Dark/light mode support with per-format styling. Configurable via state file.

### 4.4 Plugin Crate (`aidadx_plugin`)

- `src/vst3.rs` -- Steinberg VST3 SDK bindings via `aurix` or manual `pluginterfaces`.
- `src/au.rs` -- Apple Audio Unit framework via `objc2` bindings.
- `src/auv3.rs` -- AUv3 for iOS (wrapped in standalone app).
- `src/clap.rs` -- CLAP plugin format via `clap-rs` or manual host integration.
- `src/lv2.rs` -- LV2 format via `serd` + `turtle` for TTL manifest.
- `src/standalone.rs` -- Cross-platform standalone app with embedded UI (using `egui` + `eframe` or `iced` for windowing, `portmedia`/`rodio`/`cpal` for audio).
- `src/common.rs` -- Shared plugin infrastructure: parameter callbacks, state I/O, activation/deactivation, sample-rate/buffer-size change handlers.

### 4.5 iOS Crate (`aidadx_ios`)

- `AidaX/` -- Xcode project with AUv3 plugin and standalone app targets.
- `AidaX/AidaXAUv3/` -- AUv3 plugin implementation wrapping `aidadx_core`.
- `AidaX/AidaXApp/` -- Standalone app using SwiftUI for device settings (buffer size, input enable, audio session management).
- `AidaX/Shared/` -- SwiftUI views for settings, file picker (UIDocumentPickerViewController), model/IR loading.

## 5. Plugin Parameters (Preserved from Current)

| # | Name | Min | Max | Default | Unit |
|---|------|-----|-----|---------|------|
| 0 | ANTIALIASING | 0 | 100 | 66.22 | % |
| 1 | INPUT (PREGAIN) | -12 | 12 | 0 | dB |
| 2 | NETBYPASS | 0 | 1 | 0 | bool |
| 3 | EQBYPASS | 0 | 1 | 0 | bool |
| 4 | EQPOS | POST / PRE | - | POST | enum |
| 5 | BASS | -8 | 8 | 0 | dB |
| 6 | BFREQ | 75 | 600 | 305 | Hz |
| 7 | MID | -8 | 8 | 0 | dB |
| 8 | MFREQ | 150 | 5000 | 750 | Hz |
| 9 | MIDQ | 0.2 | 5 | 0.707 | - |
| 10 | MTYPE | PEAK / BANDPASS | - | PEAK | enum |
| 11 | TREBLE | -8 | 8 | 0 | dB |
| 12 | TFREQ | 1000 | 4000 | 2000 | Hz |
| 13 | DEPTH | -8 | 8 | 0 | dB |
| 14 | PRESENCE | -8 | 8 | 0 | dB |
| 15 | OUTPUT (MASTER) | -15 | 15 | 0 | dB |
| 16 | CABSIMBYPASS | 0 | 1 | 0 | bool |
| 17 | Bypass | ON / OFF | - | ON | enum |
| 18 | PARAM1 | 0 | 1 | 0 | - |
| 19 | PARAM2 | 0 | 1 | 0 | - |
| 20 | DCBLOCKER | 0 | 1 | 1 | bool |
| 21 | ModelInSize | OUTPUT | - | - | - |
| 22 | Meter In | OUTPUT | - | - | - |
| 23 | Meter Out | OUTPUT | - | - | - |

## 6. Model Architecture Support

### 6.1 Supported Current Architectures

All existing GRU and LSTM variants with hidden sizes {8, 12, 16, 20, 24, 32, 40, 64, 80} and input sizes {1, 2, 3}.

### 6.2 Future Extensibility

- **Layer types**: GRU, LSTM, Dense (via `candle`), with extensible registry pattern.
- **Input sizes**: 1, 2, 3 currently; parameterized in JSON loader for future growth.
- **Custom layers**: Compile-time feature flags to add Conv1D, attention, or other architectures.

### 6.3 Inference Backend Strategy

- **Desktop**: `candle` with CUDA (NVIDIA), Metal (Apple), AVX2/AVX-512 (x86), NEON (ARM) automatic selection.
- **iOS**: Metal via `candle-metal`.
- **Android**: NNAPI fallback or Vulkan compute via `candle-vulkan`.
- **WASM**: WebGPU via `candle-wasmtarget` or CPU-only fallback.

## 7. UI/UX Specifications

### 7.1 Desktop UI

- **Framework**: `egui` (immediate-mode GUI) or `iced` (reactive). Preference: `egui` for zero-allocation rendering, easy wgpu integration.
- **Rendering**: `wgpu` with native windowing via `eframe` (egui) or `smithay-clipboard` (iced).
- **Pedalboard**: SVG-like gradient pedal body with metallic texture, green accent highlights matching current branding.
- **Typography**: `noto-sans` or `inter` font family, scaled per DPI.
- **Controls**: Knobs with tick marks and value readout on drag; switches with smooth toggle animation; meters with color-coded fill.
- **File loading**: Native file picker per platform (OpenFileDialog on Win, NSSavePanel on macOS, GTK/FileChooser on Linux).

### 7.2 Mobile UI (iOS / Android)

- **iOS**: SwiftUI native app with `egui` embedded via `egui_kittest` for plugin UI. UIKit file picker.
- **Android**: Jetpack Compose for app shell. `egui` embedded via `egui-android` or `slint` for native rendering.
- **Touch-optimized**: Larger knob hit targets (min 44pt), swipe gestures for parameter tweaks, haptic feedback on switch toggles.
- **Portrait/landscape**: Responsive layout reflow for pedalboard on narrow screens.

### 7.3 Web UI

- **Framework**: `egui` + `eframe` compiled to WASM with WebGPU canvas.
- **Audio**: `cpal` -> `web-audio-api` via `wasm-bindgen` for low-latency streaming.
- **File loading**: Drag-and-drop file upload + `<input type="file">` fallback.

### 7.4 Theming

- **Dark mode**: Default. Pedal body: `#1c170c` -> `#2a220f` -> `#131313` gradient.
- **Light mode**: Pedal body: `#f5f0e8` -> `#e8e0d0` gradient. Accent: `#53f700`.
- **Per-host colors**: Plugin reads host background color (VST3/AU API) and adapts pedalboard shadow accordingly.

## 8. Build System

### 8.1 Cargo Workspace

```
Cargo.toml                    -- Workspace root
Cargo.lock
crates/
  aidadx_core/                -- DSP engine, model inference, state
    Cargo.toml
    src/
      lib.rs
      pipeline/
      biquad/
      convolver/
      model/
      inference/
      loader/
      state/
      params/
  aidadx_ui/                  -- UI abstraction and widget library
    Cargo.toml
    src/
      lib.rs
      renderer/
      shaders/
      widgets/
      layout/
      theme/
  aidadx_plugin/              -- Plugin format implementations
    Cargo.toml
    src/
      main.rs
      vst3.rs
      au.rs
      clap.rs
      lv2.rs
      standalone.rs
      common.rs
  aidadx_ios/                 -- iOS native app + AUv3
    Cargo.toml
    src/
      main.rs
      auv3_plugin.rs
      app/
  aidadx_cli/                 -- CLI utilities
    Cargo.toml
    src/
      main.rs
      validate.rs
      batch.rs
      export.rs
```

### 8.2 Feature Flags

```toml
[features]
default = ["vst3", "au", "clap", "lv2", "standalone"]
vst3 = ["dep:vst3-sys"]
au = ["dep:objc2", "dep:core-audio"]
clap = ["dep:clap-rs"]
lv2 = ["dep:lv2"]
ios = ["dep:objc2", "dep:core-audio", "dep:avfaudio"]
android = ["dep:cpal-android"]
wasm = ["dep:wasm-bindgen", "dep:js-sys"]
metal = ["candle?/metal"]
cuda = ["candle?/cuda"]
```

### 8.3 Cross-Compilation

- **macOS/iOS**: `rustup target add aarch64-apple-darwin aarch64-apple-ios` + Xcode toolchain for AU/AUv3.
- **Windows**: `rustup target add x86_64-pc-windows-gnu x86_64-pc-windows-msvc` + MSVC build.
- **Linux**: System packages (`libasound2-dev`, `libx11-dev`).
- **iOS**: `cargo build --target aarch64-apple-ios --features ios` + `cargo lipo` for simulator + device universal binary.
- **WASM**: `cargo build --target wasm32-unknown-unknown --release` + `wasm-opt` for size.

## 9. CI/CD Pipeline

### 9.1 GitHub Actions Matrix

| Job | OS | Target | Artifacts |
|-----|-----|--------|-----------|
| `linux-x64` | Ubuntu 24.04 | VST3, AU (none), CLAP, LV2, Standalone | `.vst3`, `.clap`, `.so`, tarball |
| `linux-arm64` | Ubuntu 24.04 (QEMU) | VST3, CLAP, LV2 | `.vst3`, `.clap`, `.so` |
| `linux-riscv64` | Ubuntu 24.04 (QEMU) | Standalone | binary tarball |
| `macos` | macOS 14+ | VST3, AU, CLAP, Standalone, dSYM | `.vst3`, `.component`, `.clap` |
| `macos-sign` | macOS 14+ | Code-signed AU/VST3 | notarized binaries |
| `windows` | Windows 2022 | VST3, CLAP, Standalone, installer | `.vst3`, `.dll`, `.exe` (Inno Setup) |
| `ios` | macOS 14+ (Xcode) | AUv3 plugin + standalone app | `.appex` (AUv3), `.ipa` (standalone) |
| `android` | Android NDK | AAR with native libs | `.aar`, APK test bundle |
| `wasm` | Ubuntu 24.04 | Standalone WASM + MOD | `.wasm`, `.js`, `.zip` |
| `pluginval` | Ubuntu 24.04 | VST3, CLAP validation | pass/fail report |
| `source` | Ubuntu 24.04 | Source tarball | `.tar.xz` |

### 9.2 Release Workflow

1. Tag push (`v*`) triggers full release pipeline.
2. macOS binaries: codesign + notarize + staple.
3. iOS: build .ipa, submit to TestFlight (optional).
4. Windows: generate Inno Setup installer.
5. Linux: build AppImage + Flatpak manifest.
6. All artifacts uploaded to GitHub Releases with SHA256 checksums.

## 10. Dependencies

### 10.1 Rust Crate Selection

| Purpose | Crate | Notes |
|---------|-------|-------|
| Audio I/O | `cpal` | Cross-platform audio, supports all targets |
| Audio I/O (iOS) | `avfaudio` | CoreAudio/AVAudioSession wrapper |
| FFT | `rustfft` | SIMD-accelerated, cross-platform |
| Neural Inference | `candle` | HuggingFace, supports Metal/CUDA/WASM |
| JSON parsing | `serde` + `serde_json` | Derive-based, zero-copy option |
| GUI (desktop) | `egui` + `eframe` | Immediate-mode, wgpu backend |
| GUI (web) | `egui` + `wasm-bindgen` | WASM target |
| Concurrency | `crossbeam`, `parking_lot` | RT-safe channels and locks |
| Memory | `rtrb` (ring buffer) | Lock-free audio ring buffers |
| Image loading | `image` crate | For sprite/knob textures |
| File I/O | `std::fs` + `xdg` | Platform-aware paths |
| MIDI (future) | `portmidi` or `rust-midi` | Optional |
| Resampling | `rubato` | High-quality sample rate conversion |
| Plugin SDK bindings | `vst3-sys`, `lv2-sys` | FFI bindings to host SDKs |

## 11. File/Directory Structure

```
AIDA-X/
├── Cargo.toml                  -- Workspace manifest
├── Cargo.lock
├── README.md
├── LICENSE
├── .gitignore
├── .github/
│   └── workflows/
│       ├── build.yml           -- CI: matrix builds all targets
│       ├── release.yml         -- Release pipeline: sign, notarize, publish
│       └── pluginval.yml       -- Plugin validation jobs
├── crates/
│   ├── aidadx_core/            -- DSP engine, model inference, state
│   ├── aidadx_ui/              -- UI abstraction, widgets, rendering
│   ├── aidadx_plugin/          -- Plugin format implementations
│   ├── aidadx_ios/             -- iOS native app + AUv3
│   └── aidadx_cli/             -- CLI utilities
├── files/                      -- Default model and IR bundles
│   ├── default-model.json
│   └── default-ir.wav
├── graphics/                   -- UI assets (knobs, icons, backgrounds)
│   ├── knobs/
│   ├── switches/
│   ├── meters/
│   ├── logos/
│   └── png/                    -- Raster versions of SVG assets
├── docs/                       -- Documentation
│   ├── screenshots/            -- UI screenshots per platform
│   └── architecture.md         -- System architecture diagram
├── scripts/                    -- Build/helper scripts
│   ├── build-ios.sh            -- iOS cross-compilation
│   ├── sign-macos.sh           -- macOS codesign + notarize
│   ├── package-windows.sh      -- Inno Setup packaging
│   └── package-linux.sh        -- AppImage/Flatpak packaging
└── tests/                      -- Test suites
    ├── dsp-tests/              -- Unit tests for biquad, convolver, resampler
    ├── model-tests/            -- Neural model loading and inference tests
    └── ui-tests/               -- Widget rendering tests (egui snapshot tests)
```

## 12. Migration Strategy

### Phase 1: Core Engine (`aidadx_core`)

1. Port biquad filters from `Biquad.h` -> `crates/aidadx_core/src/biquad/`.
2. Port FFT convolver from `FFTConvolver` -> `crates/aidadx_core/src/convolver/`.
3. Implement model JSON loader with `serde` + `candle` backend.
4. Implement the full audio processing pipeline graph.
5. Write unit tests for all DSP modules.

### Phase 2: UI Framework (`aidadx_ui`)

1. Set up `wgpu` + `egui` rendering pipeline.
2. Recreate all current widgets (knob, switch, meter, filebtn, splitter).
3. Implement pedalboard layout engine with DPI scaling.
4. Recreate current visual design (pedalboard gradient, green accents).
5. Implement dark/light theming.

### Phase 3: Plugin Backends (`aidadx_plugin`)

1. Implement VST3 plugin (Linux headless first, then native).
2. Implement CLAP plugin.
3. Implement LV2 plugin.
4. Implement standalone app with desktop windowing.
5. Implement AU plugin (macOS only).

### Phase 4: Mobile (`aidadx_ios`)

1. Set up iOS build with Xcode + cargo-lipo for universal binary.
2. Implement AUv3 plugin wrapper.
3. Build SwiftUI standalone app with audio session management.
4. Implement touch-optimized UI variant.
5. Build and test on iOS simulator + physical device.

### Phase 5: Web (`aidadx_web`)

1. Build WASM target with `egui` + `wgpu`.
2. Integrate Web Audio API for audio I/O.
3. Implement drag-and-drop file loading.
4. Optimize bundle size with `wasm-opt`.

### Phase 6: Polish & Release

1. CI/CD pipeline for all platforms.
2. macOS code signing and notarization.
3. Windows installer (Inno Setup).
4. Linux packaging (AppImage, Flatpak).
5. iOS App Store submission preparation.
6. Pluginval conformance testing.
7. Documentation and user manual updates.

## 13. Performance Targets

| Metric | Target |
|--------|--------|
| Latency (standalone, 256 sample buffer) | < 10ms round-trip |
| CPU usage (single model, 44.1kHz) | < 3% per core (modern x86) |
| Memory footprint (model loaded) | < 50MB |
| Model switch time | < 50ms (no audio dropouts) |
| UI frame rate | 60fps minimum |
| Binary size (standalone, release) | < 50MB (macOS), < 30MB (Windows) |

## 14. Risk Assessment

| Risk | Impact | Mitigation |
|------|--------|------------|
| `candle` not supporting all model types | High | Fallback to manual inference engine for unsupported layers |
| iOS AUv3 plugin restrictions | Medium | Standalone app as primary iOS target; AUv3 for compatible hosts |
| Wasm audio latency | Medium | WebAudio API with script processor fallback for older browsers |
| Plugin SDK complexity (VST3, AAX) | Medium | Leverage existing FFI bindings; focus on VST3/AU/CLAP first |
| Cross-compilation toolchains | Medium | Use pre-built CI runners; test on native targets where possible |
| GPU shader compatibility | Low | Fallback to CPU rendering in wgpu with `render-standalone` feature |

## 15. Open Questions

1. Should we replace `candle` with a lighter-weight inference engine (`burn` or custom) to reduce binary size?
2. Do we need MIDI control for parameter mapping, or stick to current parameter-only interface?
3. Should the iOS app support running as a standalone guitar effects processor (with internal audio input/output), or only as an AUv3 plugin?
4. What level of backward compatibility is required with the current DPF state format? (Full JSON schema match vs. migration tool.)
5. Should we support multi-model chaining (run multiple models in series)?
