---
name: linear-initiatives
description: View Linear initiatives. Use for high-level tracking across projects.
allowed-tools: Bash
---

# Initiatives

View and manage high-level initiatives.

## Start here

```bash
linear init list
linear init get INIT_ID --output json --compact
linear init create "Platform Migration"
linear init update INIT_ID --name "Renamed"
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `linear init --help`.
