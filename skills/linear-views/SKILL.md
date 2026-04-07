---
name: linear-views
description: Manage custom views - create, list, apply saved views. Use when working with saved issue filters.
allowed-tools: Bash
---

# Custom Views

```bash
# List all custom views
linear v list
linear v list --shared              # Shared views only

# Get view details
linear v get "My View"

# Create a view
linear v create "Bug Triage" --shared

# Update a view
linear v update VIEW_ID --name "Renamed"

# Delete a view
linear v delete VIEW_ID --force

# Apply view to issue list
linear i list --view "Bug Triage"
linear p list --view "Active Projects"
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
