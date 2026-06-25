---
name: linear-project-updates
description: Manage project status updates - create, list, archive. Use when posting or viewing project health updates.
allowed-tools: Bash
---

# Project Updates

Post and review project health/status updates.

## Start here

```bash
{{CLI_PROGRAM}} pu list PROJECT_ID
{{CLI_PROGRAM}} pu get UPDATE_ID --output json --compact
{{CLI_PROGRAM}} pu create PROJECT_ID -b "On track"
{{CLI_PROGRAM}} pu archive UPDATE_ID
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} pu create --help`.
