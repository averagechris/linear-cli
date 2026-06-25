---
name: linear-project-updates
description: Manage project status updates - create, list, archive. Use when posting or viewing project health updates.
allowed-tools: Bash
---

# Project Updates

Post and review project health/status updates.

## Start here

```bash
linear pu list PROJECT_ID
linear pu get UPDATE_ID --output json --compact
linear pu create PROJECT_ID -b "On track"
linear pu archive UPDATE_ID
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `linear pu create --help`.
