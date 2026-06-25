---
name: linear-export
description: Export Linear data to CSV, Markdown, or JSON. Use when exporting issues or projects.
allowed-tools: Bash
---

# Export

Export Linear data for reports, backups, or scripts.

## Start here

```bash
linear export csv -t ENG -f issues.csv
linear export json -t ENG -f issues.json --pretty
linear export projects-csv -f projects.csv
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- Full syntax and less-common flags: `linear export csv --help`, `linear export json --help`.
