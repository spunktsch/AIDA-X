---
name: create-pr
description: Create a pull request for a completed AIDA-X ticket and move the ticket to review.
argument-hint: "<TICKET-NNN>"
disable-model-invocation: true
---

Create a PR for: $ARGUMENTS

Read `.kilo/commands/create-pr.md` and follow its rules, with these Claude Code adaptations:

- Locate the ticket via glob `tickets/*/*/$ARGUMENTS.md`.
- Never `git add .`: stage only files belonging to the ticket (local scripts like `*.bat`, `.kilo/worktrees/`, `target/` must not be committed).
- If on `main`, create a branch first. Push only after showing the user what will be pushed.
- If `gh` is not available, push the branch and give the user the compare URL (`https://github.com/<owner>/<repo>/compare/main...<branch>`) plus a ready-to-paste PR title and body instead.
- PR body: link `tickets/<folder>/review/<ID>.md` (the path after the move), and end with the attribution line required by the session.
