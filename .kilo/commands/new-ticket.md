/kilo command new-ticket <title>

Create a new ticket in the backlog.

Rules:
1. Read `tickets/_meta.json` to get the next ticket number (increment counter).
2. Prompt user: "Which folder?" with options: dsp, ui, plugin, mobile, web, ci, misc. Default to `misc`.
3. Create ticket file: `tickets/<folder>/backlog/TICKET-NNN.md`
4. Set status to `backlog`, assignee to current user, priority to `medium` (ask user if needed).
5. Use the `<title>` as the ticket title (first line after `#`).
6. Write a brief description section.
7. Add standard acceptance criteria placeholders.
8. Add a checklist section with all items unchecked.
9. Update `tickets/_meta.json` with new counter and timestamp.
10. Confirm ticket creation to user with path and ID.

Example output:
```
Created TICKET-NNN in tickets/dsp/backlog/
Path: tickets/dsp/backlog/TICKET-NNN.md
```
