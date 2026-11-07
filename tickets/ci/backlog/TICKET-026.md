---
id: TICKET-026
status: backlog
assignee: sesch
priority: high
created: 2026-11-07
updated: 2026-11-07
---

# Repair C++ build.yml (retired runners, failing win64 build)

## Description
The C++/DPF plugin is still the shipping product, but its workflow `.github/workflows/build.yml` no longer works. First observed in run 42407904904 (push of `integration/rust-core`, which does not touch any C++ build input): linux jobs on `ubuntu-20.04` and the macOS job on `macos-13` never start (runner images retired), and `windows (win64)` fails inside `distrho/dpf-cmake-action@v1` (exit code 2, log needs authenticated access). wasm and pluginval jobs pass.

## Acceptance Criteria
- [ ] Linux matrix on a supported Ubuntu image (`ubuntu-24.04`, arm64 via native runner if dpf-cmake-action allows)
- [ ] macOS job on a supported image, still producing the universal 10.15 build
- [ ] Root cause of the win64 failure identified (check whether it also fails on upstream `AidaDSP/AIDA-X` main) and fixed
- [ ] `softprops/action-gh-release@v1` updated; release publishing coordinated with `rust.yml` (both upload to the same tag release)
- [ ] All jobs green on a push

## Implementation Notes
- Last change to build.yml upstream: 41eb988 (Dec 2024); runner retirements since then.
- Keep the C++ build working until the Rust plugin replaces it (TICKET-013).

## Checklist
- [ ] Code implemented
- [ ] Tests written
- [ ] Self-review passed
- [ ] PR created
- [ ] Merged
