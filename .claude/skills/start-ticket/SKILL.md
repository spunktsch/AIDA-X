---
name: start-ticket
description: Start an AIDA-X ticket - move it from backlog to active and launch a subagent in an isolated worktree to implement it.
argument-hint: "<TICKET-NNN>"
disable-model-invocation: true
---

Start ticket: $ARGUMENTS

Read `.kilo/commands/start-ticket.md` and follow its rules exactly, with these Claude Code adaptations:

- Locate the ticket via glob `tickets/*/*/$ARGUMENTS.md`.
- Commit the ticket move (`git mv`) before spawning, so the worktree contains it; note the commit hash.
- Steps 6-7: instead of `agent_manager`, run `git worktree list`, then launch the `Agent` tool with `isolation: "worktree"`. The prompt must contain: the base commit and the instruction to `git reset --hard <commit>` first, `git submodule update --init --recursive modules/rtneural` if Rust crates are built, the ticket path `tickets/<folder>/active/<ID>.md`, an explicit file scope, the commit message format from AGENTS.md (with the Co-Authored-By trailer), "do not push", and what to report back.
- When the agent reports back, review its result, merge its branch into the current integration branch with `--no-ff`, run `cargo test --workspace`, then remove the agent worktree and branch.
