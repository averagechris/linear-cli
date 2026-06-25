---
name: linear-cycles
description: Manage Linear sprint cycles - list, create, update, delete, complete. Use when managing cycles.
allowed-tools: Bash
---

# Cycles

Manage team cycles/sprints.

## Start here

```bash
{{CLI_PROGRAM}} c list -t ENG
{{CLI_PROGRAM}} c current -t ENG --output json --compact
{{CLI_PROGRAM}} c create -t ENG --starts-at 2026-03-01 --ends-at 2026-03-14
{{CLI_PROGRAM}} c complete CYCLE_ID
```

## Agent notes
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} c create --help`, `{{CLI_PROGRAM}} c current --help`.
