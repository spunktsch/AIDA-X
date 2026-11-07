---
id: TICKET-014
status: backlog
assignee:
priority: medium
created: 2026-11-01
updated: 2026-11-01
---

# Migrate graphics assets and default model/IR bundle

## Description
Port all graphical assets (knobs, switches, backgrounds, logos) and default model/IR files from the current project into the Rust crate structure. Assets must be embedded as compiled-in resources (Rust `include_bytes!`).

## Acceptance Criteria
- [ ] `graphics/` directory reorganized: knobs/, switches/, meters/, logos/, png/
- [ ] All PNG assets converted to embedded byte arrays in Rust source
- [ ] Default model JSON embedded: `tw40_california_clean_deerinkstudios.json`
- [ ] Default IR WAV embedded: `V30-P2-audix-i5_deerinkstudios.wav`
- [ ] SVG originals preserved in `graphics/` for reference
- [ ] Resource loading tested: default model loads and renders at startup
- [ ] File sizes documented and optimized (PNG compression, model JSON minification)

## Implementation Notes
- Current C++ uses DPF's `res2c.py` build script to convert PNGs/WAVs to C arrays
- Rust alternative: `include_bytes!` at compile time, or `embed-resource` crate
- Model JSON: `tw40_california_clean_deerinkstudios.json` (~500KB for 80-hidden GRU)
- IR WAV: `V30-P2-audix-i5_deerinkstudios.wav` (~2-10MB depending on length)
- Consider splitting large IR into multiple presets for on-demand loading
