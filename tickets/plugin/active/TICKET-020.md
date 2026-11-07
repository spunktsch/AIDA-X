---
id: TICKET-020
status: active
assignee:
priority: medium
created: 2026-11-07
updated: 2026-11-07
---

# DPF compatibility: plugin IDs, parameter IDs, state

## Description
The Rust plugin uses its own VST3 class ID and nih-plug hashed parameter IDs, so projects saved with the C++ DPF build do not load.

## Acceptance Criteria
- [ ] VST3 class ID reproduced from DPF (`modules/dpf`) for the AIDA-X unique/brand IDs
- [ ] CLAP id identical to the DPF CLAP build
- [ ] Parameter IDs/indices map to the DPF ones where nih-plug allows; document what is impossible
- [ ] State: read DPF-format state keys (`json`, `cabinet`) on load
- [ ] Findings documented in the ticket

## Implementation Notes


## Checklist
- [ ] Code implemented
- [ ] Tests written
- [ ] Self-review passed
- [ ] PR created
- [ ] Merged
