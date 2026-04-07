---
name: linear-bulk
description: Bulk operations on Linear issues. Use when updating multiple issues at once.
allowed-tools: Bash
---

# Bulk Operations

```bash
# Update status for multiple issues
{{CLI_PROGRAM}} b update-state -s Done LIN-1 LIN-2 LIN-3

# Assign multiple issues
{{CLI_PROGRAM}} b assign --user me LIN-1 LIN-2
{{CLI_PROGRAM}} b assign --user "John Doe" LIN-1 LIN-2

# Unassign multiple issues
{{CLI_PROGRAM}} b unassign LIN-1 LIN-2

# Add label to multiple issues
{{CLI_PROGRAM}} b label --add bug LIN-1 LIN-2 LIN-3

# Pipe issue IDs from stdin
{{CLI_PROGRAM}} i list -t ENG --id-only | {{CLI_PROGRAM}} b assign --user me -
```

## Flags

| Flag | Purpose |
|------|---------|
| `--dry-run` | Preview changes |
| `--output json` | JSON output |
