---
name: linear-create
description: Create Linear issues. Use when creating bugs, tasks, or feature requests.
allowed-tools: Bash
---

# Create Issues

Create bugs, tasks, and feature requests.

## Start here

```bash
linear i create "Title" -t TEAM
linear i create "Bug" -t ENG -p 1 -l bug
linear i create "Task" -t ENG --dry-run
linear i create "Title" -t ENG -d - --id-only
```

## Agent notes
- Priorities: `1` urgent, `2` high, `3` normal, `4` low.
- Use `-d -` or `--data -` to pass longer input via stdin.
- Full syntax and less-common flags: `linear i create --help`.
