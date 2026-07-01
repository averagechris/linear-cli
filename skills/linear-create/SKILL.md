---
name: linear-create
description: Create Linear issues. Use when creating bugs, tasks, or feature requests.
allowed-tools: Bash
---

# Create Issues

Create bugs, tasks, and feature requests.

## Start here

```bash
linear context --output json --compact
linear context options labels --group domain --output json --compact
linear context options labels --group domain --refresh --output json --compact
linear i create "Title"
linear i create "Bug" -p 1 -l bug
linear i create "Task" --dry-run
linear i create "Title" -d - --id-only
```

## Agent notes
- Respect `linear context` safe defaults for team/status and follow returned field policies for labels, projects, initiatives, and estimates.
- If a required label group is ambiguous, run `linear context options labels --group GROUP --output json --compact` and ask the user. Add `--refresh` if options are missing/stale.
- If no default team is configured, ask for one or suggest `linear config set default-team TEAM`.
- Priorities: `1` urgent, `2` high, `3` normal, `4` low.
- Use `-d -` or `--data -` to pass longer input via stdin.
- Full syntax and less-common flags: `linear i create --help`.
