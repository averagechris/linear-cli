---
name: linear-teams
description: Manage Linear teams and users. Use when listing teams, inspecting members, or viewing user profiles.
allowed-tools: Bash
---

# Teams

List teams, inspect members, manage team metadata, and view users.

## Start here

```bash
linear t list
linear t get ENG --output json --compact
linear t members ENG
linear u list --output json --compact
linear u me
linear t create "Platform" -k PLT
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `linear t list --help`, `linear t members --help`, `linear u --help`.
