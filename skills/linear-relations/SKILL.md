---
name: linear-relations
description: Manage Linear issue relationships. Use for blocking, parent/child, duplicates.
allowed-tools: Bash
---

# Issue Relations

```bash
# List relations
linear rel list LIN-123

# Add relation
linear rel add LIN-1 -r blocks LIN-2     # LIN-1 blocks LIN-2
linear rel add LIN-1 -r related LIN-2    # Related issues
linear rel add LIN-1 -r duplicate LIN-2  # Duplicate

# Remove relation
linear rel remove LIN-1 -r blocks LIN-2

# Parent/child
linear rel parent LIN-2 LIN-1            # Set LIN-1 as parent
linear rel unparent LIN-2                # Remove parent
```

## Relation Types

`blocks`, `blocked-by`, `related`, `duplicate`

## Flags

| Flag | Purpose |
|------|---------|
| `--output json` | JSON output |
