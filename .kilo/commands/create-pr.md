/kilo command create-pr <TICKET-ID>

Create a pull request for a completed ticket.

Rules:
1. Read the ticket file at `tickets/<folder>/active/<TICKET-ID>.md`.
2. Verify checklist items: "Code implemented" and "Tests written" should be checked.
3. If PR already exists (branch already pushed), report status and skip.
4. Call `git status` and `git log --oneline -5` to inspect changes.
5. Run `git add .` to stage all changes.
6. Run `git commit -m "<TICKET-ID>: <conventional commit type> <short description>"`.
7. Run `git push --set-upstream origin ticket-<TICKET-ID>-<description>`.
8. Run `gh pr create --base main --title "<TICKET-ID>: <short description>" --body "Closes <TICKET-ID>. See tickets/<folder>/review/<TICKET-ID>.md for details."`.
9. Move ticket file from `tickets/<folder>/active/` to `tickets/<folder>/review/`.
10. Update status to `review`, set PR URL in ticket YAML if possible.
11. Confirm to user with PR URL.
