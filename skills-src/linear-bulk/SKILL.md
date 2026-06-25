---
name: linear-bulk
description: Bulk operations on Linear issues. Use when updating multiple issues at once.
allowed-tools: Bash
---

# Bulk Operations

Apply one change to many issues.

## Start here

```bash
{{CLI_PROGRAM}} b update-state Done -i LIN-1,LIN-2
{{CLI_PROGRAM}} b assign me -i LIN-1,LIN-2
{{CLI_PROGRAM}} b label bug -i LIN-1,LIN-2
{{CLI_PROGRAM}} b unassign -i LIN-1,LIN-2
```

## Agent notes
- Issue lists are comma-separated after `-i`.
- Bulk commands execute immediately; verify the comma-separated issue list before running.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} b update-state --help`.
