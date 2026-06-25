---
name: linear-history
description: View Linear issue history. Use for activity logs and audit trails.
allowed-tools: Bash
---

# History

View issue activity and audit trails.

## Start here

```bash
{{CLI_PROGRAM}} history issue LIN-123
{{CLI_PROGRAM}} i get LIN-123 --history
{{CLI_PROGRAM}} history issue LIN-123 --output json --compact
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} history issue --help`.
