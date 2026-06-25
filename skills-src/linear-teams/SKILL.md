---
name: linear-teams
description: Manage Linear teams and users. Use when listing teams, inspecting members, or viewing user profiles.
allowed-tools: Bash
---

# Teams

List teams, inspect members, manage team metadata, and view users.

## Start here

```bash
{{CLI_PROGRAM}} t list
{{CLI_PROGRAM}} t get ENG --output json --compact
{{CLI_PROGRAM}} t members ENG
{{CLI_PROGRAM}} u list --output json --compact
{{CLI_PROGRAM}} u me
{{CLI_PROGRAM}} t create "Platform" -k PLT
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} t list --help`, `{{CLI_PROGRAM}} t members --help`, `{{CLI_PROGRAM}} u --help`.
