@AGENTS.md

# Claude Code specifics

The ticket workflow in AGENTS.md was written for Kilo. Claude Code uses the same files and rules, with these mappings:

| Kilo | Claude Code |
|---|---|
| `/kilo command <name>` | Skill `/<name>` in `.claude/skills/` (board, new-ticket, start-ticket, create-pr, merge-ticket) |
| Rules in `.kilo/commands/<name>.md` | Source of truth; the skills read and follow these files |
| `agent_manager(mode="worktree", ...)` | `Agent` tool with `isolation: "worktree"` |
| `agent_manager(action="list")` | `git worktree list` (Kilo worktrees live in `.kilo/worktrees/`, Claude's in `.claude/worktrees/`) |

## Ticket locations

- Ticket files: `tickets/<folder>/<status>/TICKET-NNN.md`, folder is one of `dsp ui plugin mobile web ci misc`, status one of `backlog active review done`.
- Find a ticket by ID: glob `tickets/*/*/TICKET-NNN.md`.
- Counter: `tickets/_meta.json` (`ticket_counter` = highest ID in use).
- Status in the YAML frontmatter must match the status folder.

## Current state

- Rust work is integrated on branch `integration/rust-core` (not yet merged to `main`).
- Old Kilo branches `ticket-005/006/007/009/010` contain C++ work that conflicts with docs/PRD.md and are not integrated.

## Build

- Needs Rust stable, a C++17 compiler (MSVC on Windows) and the submodule: `git submodule update --init --recursive modules/rtneural`.
- `cargo test --workspace`; plugin bundles: `cargo xtask bundle aidadx_plugin --release` (output `target/bundled/`).
- Validation: `scripts/validate-plugin.ps1` / `.sh` downloads third-party validators; only run with the user's approval.

## Rules for subagents in worktrees

- Agent worktrees may start on `main` instead of the current branch: tell each agent the exact base commit and to `git reset --hard <commit>` first.
- `gh` is not installed on this machine; PR creation must be done by the user or after installing it.
