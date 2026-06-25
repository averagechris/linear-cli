---
name: linear-cycles
description: Manage Linear sprint cycles - list, create, update, delete, complete. Use when managing cycles.
allowed-tools: Bash
---

# Cycles

Manage team cycles/sprints.

## Start here

```bash
linear c list -t ENG
linear c current -t ENG --output json --compact
linear c create -t ENG --starts-at 2026-03-01 --ends-at 2026-03-14
linear c complete CYCLE_ID
```

## Agent notes
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `linear c create --help`, `linear c current --help`.
