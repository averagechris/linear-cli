---
name: linear-update
description: Update Linear issues. Use when changing status, priority, assignee, or labels.
allowed-tools: Bash
---

# Update Issues

```bash
# Status
{{CLI_PROGRAM}} i update LIN-123 -s Done
{{CLI_PROGRAM}} i update LIN-123 -s "In Progress"

# Priority
{{CLI_PROGRAM}} i update LIN-123 -p 1    # 1=urgent, 2=high, 3=normal, 4=low

# Assignee
{{CLI_PROGRAM}} i update LIN-123 -a me
{{CLI_PROGRAM}} i update LIN-123 -a "John Doe"

# Labels
{{CLI_PROGRAM}} i update LIN-123 -l bug
{{CLI_PROGRAM}} i update LIN-123 -l bug -l urgent

# Due date
{{CLI_PROGRAM}} i update LIN-123 --due tomorrow
{{CLI_PROGRAM}} i update LIN-123 --due +3d

# Agent patterns
{{CLI_PROGRAM}} i update LIN-123 -s Done --id-only
```

## Comments

```bash
{{CLI_PROGRAM}} cm list LIN-123
{{CLI_PROGRAM}} cm create LIN-123 -b "Fixed in commit abc"
```

## Flags

| Flag | Purpose |
|------|---------|
| `--id-only` | Return ID only |
| `--output json` | JSON output |
