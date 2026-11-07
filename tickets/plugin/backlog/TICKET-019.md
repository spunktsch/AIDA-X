---
id: TICKET-019
status: backlog
assignee:
priority: high
created: 2026-11-07
updated: 2026-11-07
---

# Non-realtime file loading, state restore and default model/IR

## Description
`load_files_from_state` only logs the paths.

## Acceptance Criteria
- [ ] Loading model/IR on a background thread, lock-free hand-off to audio thread
- [ ] State restore loads the saved `json`/`cabinet` paths
- [ ] Bundled default model + IR (from `files/`) embedded via `include_bytes!` and used for `"default"`/missing files
- [ ] Load errors are reported, never crash or block audio

## Implementation Notes
Depends on TICKET-017. Coordinate with TICKET-014 (asset bundle).

## Checklist
- [ ] Code implemented
- [ ] Tests written
- [ ] Self-review passed
- [ ] PR created
- [ ] Merged
