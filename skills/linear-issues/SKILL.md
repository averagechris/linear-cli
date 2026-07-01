---
name: linear-issues
description: Work with Linear issues end to end - list, get, search, create, update, comment, triage, relate, and start/stop/close work. Use when viewing, creating, editing, commenting on, or closing Linear issues or bugs.
allowed-tools: Bash
---

# Issues

List, inspect, search, create, update, comment on, triage, relate, and close issues.

## Start with context

Before creating or mutating issues, load the workspace/repo conventions:

```bash
linear context --output json --compact
```

Context returns safe defaults (team, status), field policies (labels, projects, initiatives, estimates), and the current branch issue. Follow returned policies: infer values only when high-confidence, otherwise inspect options and ask the user:

```bash
linear context options labels --group GROUP --output json --compact
linear context refresh labels projects initiatives   # if options are missing/stale
```

## Command map

```bash
linear i list --mine                              # list (-t TEAM, -s STATE, --view NAME)
linear i get LIN-1 LIN-2 --output json --compact  # get one or many (--history for audit trail)
linear s issues "auth bug"                        # full-text search (also: s projects)
linear i create "Title" -p 1 -l bug               # create (-d - for stdin body, --template NAME)
linear i update LIN-123 -s Done -a me             # update status/assignee/labels/dates
linear cm list LIN-123                            # comments (cm create ISSUE -b "text")
linear rel add LIN-123 -r blocks LIN-456          # relations (rel list/parent/remove)
linear triage list -t ENG                         # triage inbox (claim, snooze -d 1w)
linear i start LIN-123 --checkout                 # begin work (assign + branch)
linear done                                       # close the current branch's issue
```

## Agent notes

- Apply `linear context` defaults for unstated fields, but never override a value the user gave explicitly.
- For parsing, add `--output json --compact`; trim payloads with `--fields identifier,title,state.name`.
- `--dry-run` (preview) and `--id-only` (print just the resulting ID) work where a command's `--help` documents them.
- Pass long bodies via `-d -` (stdin); make structured multi-field edits via `--data -` (JSON on stdin).
- Priorities: `1` urgent, `2` high, `3` normal, `4` low.
- If context reports no default team, ask the user which team to use and suggest `linear config set default-team TEAM`.
- Full syntax for any subcommand: `linear i create --help`, `linear triage --help`, etc.
