---
name: linear-update
description: Update Linear issues. Use when changing status, priority, assignee, or labels.
allowed-tools: Bash
---

# Update Issues

Change status, priority, assignee, labels, estimates, and dates.

## Start here

```bash
{{CLI_PROGRAM}} i update LIN-123 -s Done
{{CLI_PROGRAM}} i update LIN-123 -a me -p 2 --due tomorrow
{{CLI_PROGRAM}} i update LIN-123 -l bug -l urgent
{{CLI_PROGRAM}} i update LIN-123 --data - --dry-run
```

## Agent notes
- Use `--dry-run` before broad or destructive updates.
- Use `--data -` for structured JSON edits.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} i update --help`.
