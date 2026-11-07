/kilo command board

Show the ticket board — an overview of all tickets grouped by status and folder.

Rules:
1. Read `tickets/_meta.json` for metadata.
2. Recursively list all `.md` files under `tickets/` (backlog/, active/, review/, done/).
3. For each ticket file, read the YAML frontmatter to extract: id, status, folder, priority, assignee, title.
4. Also run `agent_manager(action="list")` to check which tickets have active worktrees.
5. Print a formatted table grouped by status, then by folder within each status:

```
=== AIDA-X Ticket Board ===

## BACKLOG
| Folder | ID        | Priority | Assignee | Title                          | Worktree |
|--------|-----------|----------|----------|--------------------------------|----------|
| dsp    | TICKET-001| high     |          | Implement biquad cascade       |          |
| ui     | TICKET-002| medium   | sesch    | Recreate knob widget in egui   |          |

## ACTIVE
| Folder | ID        | Priority | Assignee | Title                          | Worktree |
|--------|-----------|----------|----------|--------------------------------|----------|
| dsp    | TICKET-003| high     | sesch    | FFT convolver port to Rust     | ses_xxx  |

## REVIEW
| Folder | ID        | Priority | Assignee | Title                          | PR URL |
|--------|-----------|----------|----------|--------------------------------|--------|
| plugin | TICKET-004| medium   | sesch    | CLAP plugin skeleton           | #42    |

## DONE
| Folder | ID        | Title                          |
|--------|-----------|--------------------------------|
| misc   | TICKET-005| Set up Cargo workspace         |
```

6. If no tickets exist in any category, display "No tickets found."
7. Include a summary line at the top: "Total: N tickets | Active: N | Review: N | Done: N"
