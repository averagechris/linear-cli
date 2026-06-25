---
name: linear-views
description: Manage custom views - create, list, apply saved views. Use when working with saved issue filters.
allowed-tools: Bash
---

# Views

Use saved Linear views and manage CLI custom views.

## Start here

```bash
linear views list
linear views get "My Sprint" --output json --compact
linear i list --view "My Sprint"
linear views create "Team Bugs" --team ENG --filter-json filters.json
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `linear views create --help`, `linear i list --help`.
