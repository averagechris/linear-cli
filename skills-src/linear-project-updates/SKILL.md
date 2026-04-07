---
name: linear-project-updates
description: Manage project status updates - create, list, archive. Use when posting or viewing project health updates.
allowed-tools: Bash
---

# Project Updates

```bash
# List updates for a project
{{CLI_PROGRAM}} pu list "My Project"
{{CLI_PROGRAM}} pu list "My Project" --output json

# Get update details
{{CLI_PROGRAM}} pu get UPDATE_ID

# Create a project update
{{CLI_PROGRAM}} pu create "My Project" -b "On track this sprint"
{{CLI_PROGRAM}} pu create "My Project" -b "Blocked on API" --health atRisk

# Update an existing update
{{CLI_PROGRAM}} pu update UPDATE_ID -b "Updated status"

# Archive/unarchive
{{CLI_PROGRAM}} pu archive UPDATE_ID
{{CLI_PROGRAM}} pu unarchive UPDATE_ID
```

## Health Status

| Value | Meaning |
|-------|---------|
| `onTrack` | Project is on track (green) |
| `atRisk` | Project is at risk (yellow) |
| `offTrack` | Project is off track (red) |

## Flags

| Flag | Purpose |
|------|---------|
| `-b BODY` | Update body text |
| `--health STATUS` | Health status |
| `--output json` | JSON output |

## Exit Codes

`0`=Success, `1`=Error, `2`=Not found, `3`=Auth error
