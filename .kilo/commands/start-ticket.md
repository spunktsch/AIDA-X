/kilo command start-ticket <TICKET-ID>

Move a ticket from backlog to active and create an Agent Manager worktree.

Rules:
1. Read the ticket file at `tickets/<folder>/backlog/<TICKET-ID>.md`.
2. Verify the ticket status is `backlog`. If not, report error.
3. Move ticket file to `tickets/<folder>/active/<TICKET-ID>.md`.
4. Update the `updated` field in the ticket YAML frontmatter.
5. Set status to `active`.
6. Call `agent_manager(action="list")` to discover available worktrees.
7. Start a new Agent Manager worktree via `agent_manager(mode="worktree", tasks=[...])`:
   - name: `"<TICKET-ID>: <ticket title>"`
   - branchName: `"ticket-<TICKET-ID>-<short-description>"` (derive from title)
   - prompt: `"Implement the work described in <TICKET-ID>. See tickets/<folder>/active/<TICKET-ID>.md for acceptance criteria.\n\nRules:\n- Work in this worktree only\n- Commit with <TICKET-ID>: prefix\n- Update ticket checklist as you progress\n- Create PR when ready via /kilo command create-pr <TICKET-ID>"`
8. Confirm to user: worktree created, ticket moved to active.
