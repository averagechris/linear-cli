---
name: linear-bulk
description: Bulk operations on Linear issues. Use when updating multiple issues at once.
allowed-tools: Bash
---

# Bulk Operations

```bash
# Update status for multiple issues
linear b update-state -s Done LIN-1 LIN-2 LIN-3

# Assign multiple issues
linear b assign --user me LIN-1 LIN-2
linear b assign --user "John Doe" LIN-1 LIN-2

# Unassign multiple issues
linear b unassign LIN-1 LIN-2

# Add label to multiple issues
linear b label --add bug LIN-1 LIN-2 LIN-3

# Pipe issue IDs from stdin
linear i list -t ENG --id-only | linear b assign --user me -
```

## Flags

| Flag | Purpose |
|------|---------|
| `--dry-run` | Preview changes |
| `--output json` | JSON output |
