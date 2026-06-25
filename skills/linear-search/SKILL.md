---
name: linear-search
description: Search Linear issues and projects. Use when finding issues, looking up bugs, or searching the backlog.
allowed-tools: Bash
---

# Search

Find issues and projects by text.

## Start here

```bash
linear s issues "authentication bug"
linear s issues "oauth" --output json --compact --fields identifier,title,state.name
linear s projects "roadmap" --limit 10
linear i get LIN-123 --output json --compact
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- Full syntax and less-common flags: `linear s issues --help`, `linear s projects --help`.
