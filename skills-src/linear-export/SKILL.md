---
name: linear-export
description: Export Linear data to CSV, Markdown, or JSON. Use when exporting issues or projects.
allowed-tools: Bash
---

# Export

Export Linear data for reports, backups, or scripts.

## Start here

```bash
{{CLI_PROGRAM}} export csv -t ENG -f issues.csv
{{CLI_PROGRAM}} export json -t ENG -f issues.json --pretty
{{CLI_PROGRAM}} export projects-csv -f projects.csv
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} export csv --help`, `{{CLI_PROGRAM}} export json --help`.
