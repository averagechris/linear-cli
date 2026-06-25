---
name: linear-search
description: Search Linear issues and projects. Use when finding issues, looking up bugs, or searching the backlog.
allowed-tools: Bash
---

# Search

Find issues and projects by text.

## Start here

```bash
{{CLI_PROGRAM}} s issues "authentication bug"
{{CLI_PROGRAM}} s issues "oauth" --output json --compact --fields identifier,title,state.name
{{CLI_PROGRAM}} s projects "roadmap" --limit 10
{{CLI_PROGRAM}} i get LIN-123 --output json --compact
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} s issues --help`, `{{CLI_PROGRAM}} s projects --help`.
