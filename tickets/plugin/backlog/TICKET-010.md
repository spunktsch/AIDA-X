---
id: TICKET-010
status: backlog
assignee:
priority: high
created: 2026-11-01
updated: 2026-11-07
---

# Implement cross-platform standalone application

## Description
Build a standalone AIDA-X desktop application with embedded UI window, audio device selection, and file loading. This replaces the current DPF Jack standalone target.

## Acceptance Criteria
- [ ] Standalone binary compiles for Windows, macOS, Linux
- [ ] Embedded egui window with full pedalboard UI (reuses `aidadx_ui`)
- [ ] Audio device enumeration and selection (input/output)
- [ ] Buffer size configuration (64, 128, 256, 512, 1024 samples)
- [ ] "Enable Audio Input" button with OS permission handling (macOS microphone permission)
- [ ] File dialog for loading model JSON and IR files (native per platform)
- [ ] Loads default bundled model and IR on startup
- [ ] Window title: "AIDA-X Standalone v1.2.0"
- [ ] Cross-compiled Windows from Linux (GitHub Actions)

## Implementation Notes
- Current standalone uses DPF's `Blendish` widgets for standalone-only controls (mic input, buffer size)
- `cpal` for audio device enumeration and streaming
- `egui-native` file dialog integration or native `rfd` crate
- macOS: requires app bundle with Info.plist and correct entitlements for audio input

## Audit 2026-11-07
Work in worktree `ticket-010-standalone-app` is **uncommitted**: `standalone/` crate (Rust egui/eframe + cpal + rfd UI) calling a C++ DSP engine (`standalone/src/dsp/engine.cpp`) over C FFI, plus edits to CMakeLists.txt, build.yml, src/DistrhoPluginCommon.hpp. Never produced a binary. A duplicate of this ticket in `.kilo/tickets/` had all boxes checked — that was not accurate and was removed. Hybrid C++ engine conflicts with PRD; re-implement on `aidadx_core` once pipeline (TICKET-005) exists.
