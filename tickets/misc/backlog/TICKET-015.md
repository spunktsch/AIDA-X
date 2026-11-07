---
id: TICKET-015
status: backlog
assignee:
priority: medium
created: 2026-11-01
updated: 2026-11-01
---

# Implement CLI utility crate for model validation and batch operations

## Description
Create `aidadx_cli` — a command-line tool for validating neural model JSON files, batch-converting IR formats, and exporting model information. Useful for CI validation and user workflows.

## Acceptance Criteria
- [ ] `aidadx_cli` crate with `validate <model.json>` command: checks JSON schema, prints model architecture
- [ ] `aidadx_cli` with `info <model.json>` command: prints input size, hidden size, layer type, gains
- [ ] `aidadx_cli` with `convert-ir <input.wav> <output.flac>` command: format conversion with resampling
- [ ] `aidadx_cli` with `benchmark <model.json>` command: runs inference benchmark, reports CPU/ms
- [ ] All commands exit with non-zero status on validation failure
- [ ] Built as a separate binary in the workspace (`cargo run --package aidadx_cli`)
- [ ] Help text and man-page style documentation

## Implementation Notes
- Use `clap` crate for CLI argument parsing
- Model validation should parse JSON, detect architecture, and verify all required fields
- Benchmark mode: warm-up run + 100 iterations, report mean/median/stddev
- No GUI dependencies (pure binary, no wgpu/egui)
