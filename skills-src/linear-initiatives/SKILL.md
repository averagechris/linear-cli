---
name: linear-initiatives
description: View Linear initiatives. Use for high-level tracking across projects.
allowed-tools: Bash
---

# Initiatives

View and manage high-level initiatives.

## Start here

```bash
{{CLI_PROGRAM}} init list
{{CLI_PROGRAM}} init get INIT_ID --output json --compact
{{CLI_PROGRAM}} init create "Platform Migration"
{{CLI_PROGRAM}} init update INIT_ID --name "Renamed"
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} init --help`.
