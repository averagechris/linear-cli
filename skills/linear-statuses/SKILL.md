---
name: linear-statuses
description: View and manage workflow states. Use when checking or configuring issue statuses for a team.
allowed-tools: Bash
---

# Statuses

Inspect or manage workflow states for a team.

## Start here

```bash
linear st list -t ENG
linear st get "In Progress" -t ENG --output json --compact
linear st update STATUS_ID -t ENG --name "In Review" --dry-run
linear st get "In Progress" -t ENG --output json --compact
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `linear st list --help`, `linear st update --help`.
