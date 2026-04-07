---
name: linear-export
description: Export Linear data to CSV, Markdown, or JSON. Use when exporting issues or projects.
allowed-tools: Bash
---

# Export

```bash
# Export issues to CSV
linear exp csv -t ENG                     # Export team issues
linear exp csv -t ENG -f issues.csv       # Export to file
linear exp csv --all -t ENG               # All pages

# Export to Markdown
linear exp markdown -t ENG
linear exp markdown -t ENG -f issues.md

# Export to JSON (round-trip compatible with import)
linear exp json -t ENG -f backup.json

# Export projects to CSV
linear exp projects-csv -f projects.csv

# With filters
linear exp csv -t ENG -s "In Progress"
linear exp csv -t ENG --assignee me
```

## Flags

| Flag | Purpose |
|------|---------|
| `-f FILE` | Output to file |
| `--all` | Export all pages |
| `-t TEAM` | Filter by team |
