---
name: linear-favorites
description: Manage Linear favorites. Use for quick access to issues and projects.
allowed-tools: Bash
---

# Favorites

List and manage favorite issues/projects/views.

## Start here

```bash
linear fav list
linear fav list --output json --compact
linear fav add LIN-123
linear fav remove LIN-123
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `linear fav --help`.
