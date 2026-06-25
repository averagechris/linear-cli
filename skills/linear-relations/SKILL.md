---
name: linear-relations
description: Manage Linear issue relationships. Use for blocking, parent/child, duplicates.
allowed-tools: Bash
---

# Issue Relations

Manage blocking, parent/child, and duplicate relationships.

## Start here

```bash
linear rel list LIN-123
linear rel add LIN-123 -r blocks LIN-456
linear rel parent LIN-123 LIN-100
linear rel remove RELATION_ID
```

## Agent notes
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `linear rel add --help`.
