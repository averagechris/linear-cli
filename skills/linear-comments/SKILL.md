---
name: linear-comments
description: Manage issue comments - list, create, update, delete. Use when reading or posting comments on issues.
allowed-tools: Bash
---

# Comments

Read and manage issue comments.

## Start here

```bash
linear cm list LIN-123
linear cm list LIN-123 --output json --compact
linear cm create LIN-123 -b "Update posted"
linear cm update COMMENT_ID -b "Edited"
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `linear cm list --help`, `linear cm create --help`.
