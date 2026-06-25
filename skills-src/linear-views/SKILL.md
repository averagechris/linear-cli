---
name: linear-views
description: Manage custom views - create, list, apply saved views. Use when working with saved issue filters.
allowed-tools: Bash
---

# Views

Use saved Linear views and manage CLI custom views.

## Start here

```bash
{{CLI_PROGRAM}} views list
{{CLI_PROGRAM}} views get "My Sprint" --output json --compact
{{CLI_PROGRAM}} i list --view "My Sprint"
{{CLI_PROGRAM}} views create "Team Bugs" --team ENG --filter-json filters.json
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} views create --help`, `{{CLI_PROGRAM}} i list --help`.
