---
id: TICKET-021
status: active
assignee:
priority: medium
created: 2026-11-07
updated: 2026-11-07
---

# Validate plugin with clap-validator and pluginval

## Description
The plugin has never been loaded by a host or validator.

## Acceptance Criteria
- [ ] clap-validator run on the CLAP build, all failures fixed or documented
- [ ] pluginval (strictness 5+) on the VST3 bundle
- [ ] Bundle layout (`.vst3` folder, `.clap`) produced by a script or xtask
- [ ] Smoke test in at least one DAW documented

## Implementation Notes
Depends on TICKET-017/019. Feeds TICKET-013 (CI).

## Checklist
- [ ] Code implemented
- [ ] Tests written
- [ ] Self-review passed
- [ ] PR created
- [ ] Merged
