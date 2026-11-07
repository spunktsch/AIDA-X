/kilo command merge-ticket <TICKET-ID>

Close a ticket that has been merged.

Rules:
1. Read the ticket file at `tickets/<folder>/review/<TICKET-ID>.md`.
2. Verify status is `review`. If not, report error.
3. Move ticket file to `tickets/<folder>/done/<TICKET-ID>.md`.
4. Update the `updated` field in the ticket YAML frontmatter.
5. Set status to `done`.
6. Check off all remaining checklist items.
7. Confirm to user: ticket closed, moved to done.
