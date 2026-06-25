---
name: linear-comments
description: Manage issue comments - list, create, update, delete. Use when reading or posting comments on issues.
allowed-tools: Bash
---

# Comments

Read and manage issue comments.

## Start here

```bash
{{CLI_PROGRAM}} cm list LIN-123
{{CLI_PROGRAM}} cm list LIN-123 --output json --compact
{{CLI_PROGRAM}} cm create LIN-123 -b "Update posted"
{{CLI_PROGRAM}} cm update COMMENT_ID -b "Edited"
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} cm list --help`, `{{CLI_PROGRAM}} cm create --help`.
