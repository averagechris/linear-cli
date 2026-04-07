---
name: linear-search
description: Search Linear issues and projects. Use when finding issues, looking up bugs, or searching the backlog.
allowed-tools: Bash
---

# Linear Search

Search Linear.app issues and projects using `linear`.

## Search Issues

```bash
# Search by text
{{CLI_PROGRAM}} s issues "authentication bug"

# Limit results
{{CLI_PROGRAM}} s issues "login" --limit 5

# JSON output for parsing
{{CLI_PROGRAM}} s issues "error" --output json

# With specific fields
{{CLI_PROGRAM}} s issues "crash" --output json --fields identifier,title,state.name
```

## Search Projects

```bash
# Search projects
{{CLI_PROGRAM}} s projects "backend"

# Limit results
{{CLI_PROGRAM}} s projects "api" --limit 10

# JSON output
{{CLI_PROGRAM}} s projects "mobile" --output json
```

## Filter Results

After searching, get details on specific issues:

```bash
# Get issue details
{{CLI_PROGRAM}} i get LIN-123 --output json

# Get comments
{{CLI_PROGRAM}} cm list LIN-123 --output json

# List issues by team
{{CLI_PROGRAM}} i list -t ENG --output json

# List issues by status
{{CLI_PROGRAM}} i list -s "In Progress" --output json
```

## Tips

- Search is case-insensitive
- Searches issue titles and descriptions
- Use `--output json` for programmatic access
- Use `--limit` to control result count
- Combine with `i get` for full details
