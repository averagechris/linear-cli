---
name: linear-projects
description: Manage Linear projects - full CRUD with labels, members, archive. Use when managing projects.
allowed-tools: Bash
---

# Projects

List, inspect, create, update, label, archive, and delete projects.

## Start here

```bash
{{CLI_PROGRAM}} p list
{{CLI_PROGRAM}} p get PROJECT_ID --output json --compact
{{CLI_PROGRAM}} p create "Q1 Roadmap" -t ENG
{{CLI_PROGRAM}} p members PROJECT_ID
{{CLI_PROGRAM}} p archive PROJECT_ID
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} p list --help`, `{{CLI_PROGRAM}} p create --help`.
