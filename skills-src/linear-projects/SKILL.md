---
name: linear-projects
description: Manage Linear projects - full CRUD with labels, members, archive. Use when managing projects.
allowed-tools: Bash
---

# Projects

```bash
# List projects
{{CLI_PROGRAM}} p list                    # All projects
{{CLI_PROGRAM}} p list --archived         # Include archived
{{CLI_PROGRAM}} p list --view "Active"    # Apply saved view

# Get project
{{CLI_PROGRAM}} p get PROJECT_ID
{{CLI_PROGRAM}} p open PROJECT_ID        # Open in browser

# Create project (full API fields)
{{CLI_PROGRAM}} p create "Q1 Roadmap" -t ENG
{{CLI_PROGRAM}} p create "Feature" -t ENG --icon "🚀" --priority 1 \
  --start-date 2025-01-01 --target-date 2025-03-31 \
  --lead USER_ID --status planned --content "Project description"

# Update project
{{CLI_PROGRAM}} p update PROJECT_ID --name "New Name" --status completed
{{CLI_PROGRAM}} p update PROJECT_ID --lead USER_ID --priority 2

# Archive/unarchive
{{CLI_PROGRAM}} p archive PROJECT_ID
{{CLI_PROGRAM}} p unarchive PROJECT_ID

# Labels
{{CLI_PROGRAM}} p add-labels PROJECT_ID -l label1 -l label2
{{CLI_PROGRAM}} p remove-labels PROJECT_ID -l label1
{{CLI_PROGRAM}} p set-labels PROJECT_ID -l label1 -l label2

# Members
{{CLI_PROGRAM}} p members PROJECT_ID

# Delete
{{CLI_PROGRAM}} p delete PROJECT_ID --force
```

## Flags

| Flag | Purpose |
|------|---------|
| `--icon EMOJI` | Project icon |
| `--priority N` | Priority (1=urgent, 4=low) |
| `--start-date DATE` | Start date |
| `--target-date DATE` | Target date |
| `--lead USER` | Project lead |
| `--status STATE` | Project status |
| `--id-only` | Return ID only |
| `--output json` | JSON output |
