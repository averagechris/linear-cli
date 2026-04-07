---
name: linear-projects
description: Manage Linear projects - full CRUD with labels, members, archive. Use when managing projects.
allowed-tools: Bash
---

# Projects

```bash
# List projects
linear p list                    # All projects
linear p list --archived         # Include archived
linear p list --view "Active"    # Apply saved view

# Get project
linear p get PROJECT_ID
linear p open PROJECT_ID        # Open in browser

# Create project (full API fields)
linear p create "Q1 Roadmap" -t ENG
linear p create "Feature" -t ENG --icon "🚀" --priority 1 \
  --start-date 2025-01-01 --target-date 2025-03-31 \
  --lead USER_ID --status planned --content "Project description"

# Update project
linear p update PROJECT_ID --name "New Name" --status completed
linear p update PROJECT_ID --lead USER_ID --priority 2

# Archive/unarchive
linear p archive PROJECT_ID
linear p unarchive PROJECT_ID

# Labels
linear p add-labels PROJECT_ID -l label1 -l label2
linear p remove-labels PROJECT_ID -l label1
linear p set-labels PROJECT_ID -l label1 -l label2

# Members
linear p members PROJECT_ID

# Delete
linear p delete PROJECT_ID --force
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
