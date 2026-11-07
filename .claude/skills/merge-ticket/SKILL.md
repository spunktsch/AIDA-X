---
name: merge-ticket
description: Close an AIDA-X ticket whose PR was merged - move it from review to done.
argument-hint: "<TICKET-NNN>"
disable-model-invocation: true
---

Close ticket: $ARGUMENTS

Read `.kilo/commands/merge-ticket.md` and follow its rules exactly, with these Claude Code adaptations:

- Locate the ticket via glob `tickets/*/*/$ARGUMENTS.md`; use `git mv` for the move.
- Before checking off remaining items, confirm the PR is actually merged (`git branch -r --merged origin/main` or `gh pr view`); if not, report and stop.
- Do not tick acceptance criteria that were documented as not met; leave them and mention them.
