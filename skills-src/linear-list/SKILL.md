---
name: linear-list
description: List and get Linear issues. Use when viewing issues, checking status, or fetching issue details.
allowed-tools: Bash
---

# List/Get Issues

```bash
# List issues
{{CLI_PROGRAM}} i list                    # All
{{CLI_PROGRAM}} i list -t ENG             # By team
{{CLI_PROGRAM}} i list -s "In Progress"   # By status
{{CLI_PROGRAM}} i list --assignee me      # My issues

# Get issue(s)
{{CLI_PROGRAM}} i get LIN-123
{{CLI_PROGRAM}} i get LIN-1 LIN-2 LIN-3   # Multiple

# Agent-optimized
{{CLI_PROGRAM}} i list --output json --compact --fields identifier,title,state.name
```

## Flags

| Flag | Purpose |
|------|---------|
| `--output json` | JSON output |
| `--compact` | No formatting |
| `--fields a,b` | Select fields |
| `--sort field` | Sort results |

## Exit Codes

`0`=Success, `1`=Error, `2`=Not found, `3`=Auth error
