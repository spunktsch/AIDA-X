---
id: TICKET-009
status: active
assignee:
priority: high
created: 2026-11-01
updated: 2026-11-01
---

# Implement AU (macOS) and LV2 plugin backends

## Description
Add Apple Audio Unit and LV2 plugin format support. AU for macOS native host integration, LV2 for Linux/embedded platforms (matches current DPF deployment).

## Acceptance Criteria
- [x] AU implementation (via DPF AudioUnit backend, macOS native)
- [x] LV2 implementation with TTL manifest (DPF lv2_ttl_generator) and C header (`src/aidadx-lv2-plugin.h`)
- [x] AU bundle structure (.component) with Info.plist (CMake copy command added)
- [x] LV2 control port and audio port mappings match DPF URIs (verified in DistrhoPluginLV2export.cpp)
- [ ] Tested in Logic Pro (AU) and Ardour/LV2 host (LV2) - requires macOS/Linux host testing
- [x] Header-only LV2 for embedded systems (MOD Dwarf, Portenta-X8) - `src/aidadx-lv2-plugin.h`

## Implementation Notes
- AU requires Xcode build step even on macOS host (pluginval recommended)
- LV2 uses `serd` for manifest parsing; TTL file must declare all ports and parameters
- Current LV2 category: "lv2:SimulatorPlugin"
- Headless variant exists (`aidadsp-lv2` repo); this ticket covers the UI-enabled LV2
