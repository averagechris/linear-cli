---
name: linear-export
description: Export Linear data to CSV, Markdown, or JSON. Use when exporting issues or projects.
allowed-tools: Bash
---

# Export

```bash
# Export issues to CSV
{{CLI_PROGRAM}} exp csv -t ENG                     # Export team issues
{{CLI_PROGRAM}} exp csv -t ENG -f issues.csv       # Export to file
{{CLI_PROGRAM}} exp csv --all -t ENG               # All pages

# Export to Markdown
{{CLI_PROGRAM}} exp markdown -t ENG
{{CLI_PROGRAM}} exp markdown -t ENG -f issues.md

# Export to JSON (round-trip compatible with import)
{{CLI_PROGRAM}} exp json -t ENG -f backup.json

# Export projects to CSV
{{CLI_PROGRAM}} exp projects-csv -f projects.csv

# With filters
{{CLI_PROGRAM}} exp csv -t ENG -s "In Progress"
{{CLI_PROGRAM}} exp csv -t ENG --assignee me
```

## Flags

| Flag | Purpose |
|------|---------|
| `-f FILE` | Output to file |
| `--all` | Export all pages |
| `-t TEAM` | Filter by team |
