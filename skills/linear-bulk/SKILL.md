---
name: linear-bulk
description: Bulk operations on Linear issues. Use when updating multiple issues at once.
allowed-tools: Bash
---

# Bulk Operations

Apply one change to many issues.

## Start here

```bash
linear b update-state Done -i LIN-1,LIN-2
linear b assign me -i LIN-1,LIN-2
linear b label bug -i LIN-1,LIN-2
linear b unassign -i LIN-1,LIN-2
```

## Agent notes
- Issue lists are comma-separated after `-i`.
- Bulk commands execute immediately; verify the comma-separated issue list before running.
- Full syntax and less-common flags: `linear b update-state --help`.
