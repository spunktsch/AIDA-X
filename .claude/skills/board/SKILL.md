---
name: board
description: Show the AIDA-X ticket board (all tickets in tickets/ grouped by status and folder, with worktrees). Use when asked about tickets, ticket status, what is open/active, or before starting work.
---

Read `.kilo/commands/board.md` and follow its rules exactly, with these Claude Code adaptations:

- Ticket files are `tickets/<folder>/<status>/TICKET-NNN.md`; glob `tickets/*/*/TICKET-*.md`.
- Instead of `agent_manager(action="list")`, run `git worktree list` and match branch names containing the ticket number (`ticket-NNN-*`) to fill the Worktree column.
- Flag inconsistencies: frontmatter `status` differing from its folder, duplicate IDs, IDs above `ticket_counter` in `tickets/_meta.json`.
