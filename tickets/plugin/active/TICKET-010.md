---
id: TICKET-010
status: active
assignee:
priority: high
created: 2026-11-01
updated: 2026-11-01
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
