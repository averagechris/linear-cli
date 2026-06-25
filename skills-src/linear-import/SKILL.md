---
name: linear-import
description: Import issues from CSV or JSON files. Use when bulk-creating issues from external data.
allowed-tools: Bash
---

# Import

Bulk-create issues from CSV or JSON.

## Start here

```bash
{{CLI_PROGRAM}} import csv issues.csv -t ENG --dry-run
{{CLI_PROGRAM}} import json issues.json -t ENG
{{CLI_PROGRAM}} import json issues.json -t ENG --dry-run
```

## Agent notes
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} import csv --help`, `{{CLI_PROGRAM}} import json --help`.
