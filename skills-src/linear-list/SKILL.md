---
name: linear-list
description: List and get Linear issues. Use when viewing issues, checking status, or fetching issue details.
allowed-tools: Bash
---

# List/Get Issues

List, inspect, and batch-fetch issues.

## Start here

```bash
{{CLI_PROGRAM}} i list --mine
{{CLI_PROGRAM}} i list -t ENG -s "In Progress"
{{CLI_PROGRAM}} i get LIN-123 --output json --compact
{{CLI_PROGRAM}} i get LIN-1 LIN-2 LIN-3 --output json --fields identifier,title,state.name
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} i list --help`, `{{CLI_PROGRAM}} i get --help`.
