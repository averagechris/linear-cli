---
name: linear-views
description: Manage custom views - create, list, apply saved views. Use when working with saved issue filters.
allowed-tools: Bash
---

# Custom Views

```bash
# List all custom views
{{CLI_PROGRAM}} v list
{{CLI_PROGRAM}} v list --shared              # Shared views only

# Get view details
{{CLI_PROGRAM}} v get "My View"

# Create a view
{{CLI_PROGRAM}} v create "Bug Triage" --shared

# Update a view
{{CLI_PROGRAM}} v update VIEW_ID --name "Renamed"

# Delete a view
{{CLI_PROGRAM}} v delete VIEW_ID --force

# Apply view to issue list
{{CLI_PROGRAM}} i list --view "Bug Triage"
{{CLI_PROGRAM}} p list --view "Active Projects"
```

## Flags

| Flag | Purpose |
|------|---------|
| `--shared` | Shared views only |
| `--name NAME` | View name |
| `--view NAME` | Apply view filter (on issues/projects list) |
| `--output json` | JSON output |

## Exit Codes

`0`=Success, `1`=Error, `2`=Not found, `3`=Auth error
