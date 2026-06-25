---
name: linear-roadmaps
description: View Linear roadmaps. Use when viewing roadmap planning.
allowed-tools: Bash
---

# Roadmaps

View and manage Linear roadmaps.

## Start here

```bash
{{CLI_PROGRAM}} rm list
{{CLI_PROGRAM}} rm get ROADMAP_ID --output json --compact
{{CLI_PROGRAM}} rm create "2026 Plan"
{{CLI_PROGRAM}} rm update ROADMAP_ID --name "H1"
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} rm --help`.
