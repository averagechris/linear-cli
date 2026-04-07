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
linear s issues "authentication bug"

# Limit results
linear s issues "login" --limit 5

# JSON output for parsing
linear s issues "error" --output json

# With specific fields
linear s issues "crash" --output json --fields identifier,title,state.name
```

## Search Projects

```bash
# Search projects
linear s projects "backend"

# Limit results
linear s projects "api" --limit 10

# JSON output
linear s projects "mobile" --output json
```

## Filter Results

After searching, get details on specific issues:

```bash
# Get issue details
linear i get LIN-123 --output json

# Get comments
linear cm list LIN-123 --output json

# List issues by team
linear i list -t ENG --output json

# List issues by status
linear i list -s "In Progress" --output json
```

## Tips

- Search is case-insensitive
- Searches issue titles and descriptions
- Use `--output json` for programmatic access
- Use `--limit` to control result count
- Combine with `i get` for full details
