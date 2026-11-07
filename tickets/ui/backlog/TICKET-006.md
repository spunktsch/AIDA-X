---
id: TICKET-006
status: backlog
assignee:
priority: high
created: 2026-11-01
updated: 2026-11-07
---

# Implement wgpu + egui rendering backend with pedalboard layout

## Description
Set up hardware-accelerated rendering. The desktop UI uses egui with wgpu backend. The pedalboard layout matches the current C++ design: dark metallic body with green accent gradient.

## Acceptance Criteria
- [ ] `aidadx_ui` crate with `egui` + `eframe` + `wgpu` dependencies
- [ ] Pedalboard background: gradient from `#1c170c` through `#2a220f` to `#131313`
- [ ] Green accent panel (current: `#8bf700` to `#cdff05` gradient)
- [ ] "AIDA" brand text and "AX" logo rendered via egui fonts/textures
- [ ] "AI CRAFTED TONE" subtitle
- [ ] Version label in top-right corner (format: "AIDA-X <format> v1.2.0")
- [ ] DPI-aware scaling (multiplier from egui context)
- [ ] Responsive resize handling

## Implementation Notes
- Current DPF UI uses NanoVG/OpenGL. Must replace with egui/wgpu.
- Pedalboard dimensions: 900x318 base, margins 20px left/right, 40px top, 20px bottom.
- Background texture from `graphics/png/background.png` -> load as egui texture.
- Knob/scale images from `graphics/png/knob.png`, `graphics/png/scale.png`.
- Logo images from `graphics/png/aida.png`, `graphics/png/ax.png`.

## Audit 2026-11-07
Branch `ticket-006-wgpu-egui-backend` (commit b2c1a9c) changes the **C++/DPF** code (src/wgpu-egui/*.cpp + egui/wgpu-native submodules) instead of the Rust crates required by docs/PRD.md. Not integrated; branch kept for reference. Acceptance criteria reset — must be re-implemented in Rust.
