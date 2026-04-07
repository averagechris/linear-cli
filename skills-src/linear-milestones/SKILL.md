---
name: linear-milestones
description: Manage project milestones - create, update, track target dates. Use when planning project deliverables.
allowed-tools: Bash
---

# Milestones

```bash
# List milestones for a project
{{CLI_PROGRAM}} ms list -p "My Project"
{{CLI_PROGRAM}} ms list -p "My Project" --output json

# Get milestone details
{{CLI_PROGRAM}} ms get MILESTONE_ID

# Create a milestone
{{CLI_PROGRAM}} ms create "Beta Release" -p "My Project"
{{CLI_PROGRAM}} ms create "GA" -p PROJ --target-date 2025-06-01

# Update a milestone
{{CLI_PROGRAM}} ms update MILESTONE_ID --target-date +2w
{{CLI_PROGRAM}} ms update MILESTONE_ID --name "Renamed"

# Delete a milestone
{{CLI_PROGRAM}} ms delete MILESTONE_ID --force
```

## Flags

| Flag | Purpose |
|------|---------|
| `-p PROJECT` | Project name/ID |
| `--target-date DATE` | Target date (YYYY-MM-DD or +Nw) |
| `--name NAME` | Milestone name |
| `--output json` | JSON output |

## Exit Codes

`0`=Success, `1`=Error, `2`=Not found, `3`=Auth error
