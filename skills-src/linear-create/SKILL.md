---
name: linear-create
description: Create Linear issues. Use when creating bugs, tasks, or feature requests.
allowed-tools: Bash
---

# Create Issues

Create bugs, tasks, and feature requests.

## Start here

```bash
{{CLI_PROGRAM}} context --output json --compact
{{CLI_PROGRAM}} context options labels --group domain --output json --compact
{{CLI_PROGRAM}} context options labels --group domain --refresh --output json --compact
{{CLI_PROGRAM}} i create "Title"
{{CLI_PROGRAM}} i create "Bug" -p 1 -l bug
{{CLI_PROGRAM}} i create "Task" --dry-run
{{CLI_PROGRAM}} i create "Title" -d - --id-only
```

## Agent notes
- Respect `{{CLI_PROGRAM}} context` safe defaults for team/status and follow returned field policies for labels, projects, initiatives, and estimates.
- If a required label group is ambiguous, run `{{CLI_PROGRAM}} context options labels --group GROUP --output json --compact` and ask the user. Add `--refresh` if options are missing/stale.
- If no default team is configured, ask for one or suggest `{{CLI_PROGRAM}} config set default-team TEAM`.
- Priorities: `1` urgent, `2` high, `3` normal, `4` low.
- Use `-d -` or `--data -` to pass longer input via stdin.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} i create --help`.
