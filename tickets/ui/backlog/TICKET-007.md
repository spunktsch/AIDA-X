---
id: TICKET-007
status: backlog
assignee:
priority: high
created: 2026-11-01
updated: 2026-11-07
---

# Implement interactive widgets: knobs, switches, meters, file buttons

## Description
Port all UI widgets from `src/Widgets.hpp` to egui. Each widget must support mouse interaction (drag, click, double-click) and render at the correct position on the pedalboard.

## Acceptance Criteria
- [ ] `AidaKnob`: rotary knob with tick markers, rotating image, value display on drag. Supports drag, scroll, double-click reset.
- [ ] `AidaPluginSwitch`: vertical toggle with labeled ON/OFF states. Smooth animation on toggle.
- [ ] `AidaMeter`: peak meter with color-coded fill (green #a4f44d, yellow #f4f14d, red #f44d50). Shows "INPUT"/"OUTPUT" label and dB value.
- [ ] `AidaFilenameButton`: file loader with toggle icon (amp/cab) and filename text. Shows hint text on hover.
- [ ] `AidaSplitter`: thin vertical decorative line (4px wide, gray, 48% opacity).
- [ ] All widgets DPI-scaled and positionable via `egui::Ui` nesting.
- [ ] Widget callback system connects to parameter change events.

## Implementation Notes
- Knob rendering: current C++ draws knob image rotated based on normalized value. egui approach: use `egui::Response::hovered` + `emath::Rot64` for rotation.
- Switch rendering: rounded rect with animated circle position. Can use egui's built-in `toggle_value` with custom style.
- Meter: custom paint callback with gradient fill and color transitions.
- File buttons: composite widget (switch + text button).

## Audit 2026-11-07
Branch `ticket-007-interactive-widgets` (commit 87877ac) changes the **C++/DPF** code (src/wgpu-egui/Widgets.hpp etc.) instead of the Rust crates required by docs/PRD.md. Not integrated; branch kept for reference. Acceptance criteria reset — must be re-implemented in Rust.

## Checklist
- [ ] Code implemented
- [ ] Tests written
- [ ] Self-review passed
- [ ] PR created
- [ ] Merged
