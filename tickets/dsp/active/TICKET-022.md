---
id: TICKET-022
status: active
assignee:
priority: medium
created: 2026-11-07
updated: 2026-11-07
---

# Golden-reference tests for biquad/tone control against C++

## Description
Biquad port is verified only at formula level, not against real C++ output.

## Acceptance Criteria
- [ ] Small C++ harness (built with MSVC/cl or cc) runs the original `src/` biquad + tone control and dumps reference output for a fixed set of parameters and signals
- [ ] Reference data committed as test fixtures; Rust tests compare within 1e-5
- [ ] Covers all filter types, EQ pre/post, mid type, DC blocker, smoother ramps
- [ ] Any mismatch fixed in Rust

## Implementation Notes
SIMD optimisation out of scope (separate ticket if profiling shows need).

## Checklist
- [ ] Code implemented
- [ ] Tests written
- [ ] Self-review passed
- [ ] PR created
- [ ] Merged
