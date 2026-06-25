---
name: linear-time
description: Track time on Linear issues. Use for logging and viewing time entries.
allowed-tools: Bash
---

# Time Tracking

Log and inspect time entries.

## Start here

```bash
{{CLI_PROGRAM}} time list -i LIN-123
{{CLI_PROGRAM}} time log LIN-123 1h -d "Review"
{{CLI_PROGRAM}} time list -i LIN-123 --output json --compact
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} time list --help`, `{{CLI_PROGRAM}} time log --help`.
