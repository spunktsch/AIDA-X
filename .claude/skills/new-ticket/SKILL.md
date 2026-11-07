---
name: new-ticket
description: Create a new AIDA-X ticket in tickets/<folder>/backlog/ and bump tickets/_meta.json. Use when the user wants to record a new task, bug or follow-up as a ticket.
argument-hint: "<title>"
---

Create a ticket titled: $ARGUMENTS

Read `.kilo/commands/new-ticket.md` and follow its rules exactly, with these Claude Code adaptations:

- Ask for folder and priority with AskUserQuestion only if they cannot be inferred from the title/context; otherwise pick and state them.
- Use the ticket file format from AGENTS.md ("Ticket File Format"); `created`/`updated` = today.
- Fill Description, Acceptance Criteria and Implementation Notes with real content when the context provides it, not placeholders.
- Commit the new ticket and `_meta.json` together only if the user asked to commit.
