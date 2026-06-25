---
name: linear-import
description: Import issues from CSV or JSON files. Use when bulk-creating issues from external data.
allowed-tools: Bash
---

# Import

Bulk-create issues from CSV or JSON.

## Start here

```bash
linear import csv issues.csv -t ENG --dry-run
linear import json issues.json -t ENG
linear import json issues.json -t ENG --dry-run
```

## Agent notes
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `linear import csv --help`, `linear import json --help`.
