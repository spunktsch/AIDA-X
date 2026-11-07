---
id: TICKET-023
status: backlog
assignee:
priority: medium
created: 2026-11-07
updated: 2026-11-07
---

# nih-plug patch: import DPF state and report real CLAP parameter ranges

## Description
Follow-up of TICKET-020. Plugin/parameter IDs already match DPF, but projects saved with the C++ build lose model/IR paths and parameter values, because nih-plug only accepts its own JSON state. CLAP continuous parameters are exposed as 0..1 while DPF reports real ranges.

## Acceptance Criteria
- [ ] Vendored/forked nih-plug (pinned) with optional hook `deserialize_foreign_state(&[u8]) -> Option<PluginState>` called when JSON parsing fails (`wrapper/state.rs`)
- [ ] AIDA-X parses the DPF blob (`__dpf_state_begin__` json/cabinet ... `__dpf_parameters_begin__` symbol/value pairs ... `\xfe`) for VST3 and CLAP
- [ ] CLAP param info reports plain ranges for continuous params (opt-in in the fork)
- [ ] Tests with real DPF state blobs captured from the C++ build
- [ ] Patch proposed upstream to nih-plug

## Implementation Notes
See "## Findings" in TICKET-020.

## Checklist
- [ ] Code implemented
- [ ] Tests written
- [ ] Self-review passed
- [ ] PR created
- [ ] Merged
