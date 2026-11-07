---
id: TICKET-011
status: backlog
assignee:
priority: high
created: 2026-11-01
updated: 2026-11-07
---

# Implement iOS AUv3 plugin wrapper and standalone app

## Description
Create the iOS deployment targets: an AUv3 plugin for use in hosts like Cubasis/GarageBand, and a standalone app for direct guitar input processing.

## Acceptance Criteria
- [ ] iOS AUv3 plugin (`.appex`) compiles for device (arm64)
- [ ] iOS standalone app (`.ipa`) with SwiftUI interface
- [ ] Audio session management (input enable, category: PlayAndRecord)
- [ ] Low-latency audio processing (< 10ms round-trip on iPhone)
- [ ] Model and IR loading via `UIDocumentPickerViewController`
- [ ] Touch-optimized UI: larger knobs (44pt minimum hit target)
- [ ] Portrait and landscape layout support
- [ ] Built with `cargo-lipo` for universal binary (device + simulator)

## Implementation Notes
- `aidadx_ios` crate wraps `aidadx_core` for iOS audio
- SwiftUI for standalone app: settings view, file picker, main pedalboard view with embedded egui
- AUv3 requires `AudioComponent` registration in Info.plist
- Metal acceleration via `candle/metal` feature flag for GPU inference
- Current `Info.plist` from C++ project must be ported to Xcode project

## Audit 2026-11-07
Worktree `ticket-011-ios-auv3` was created but contains no work. Blocked on aidadx_core pipeline + aidadx_plugin.

## Checklist
- [ ] Code implemented
- [ ] Tests written
- [ ] Self-review passed
- [ ] PR created
- [ ] Merged
