---
name: linear-data
description: Move Linear data in bulk - bulk issue updates, CSV/JSON import and export, documents, attachments, and downloading uploads/images. Use when updating many issues at once, importing or exporting issues, managing documents, or fetching attachment files and screenshots.
allowed-tools: Bash Read
---

# Data & Bulk Operations

Bulk edits, import/export, documents, attachments, and upload downloads.

## Start with context

```bash
{{CLI_PROGRAM}} context --output json --compact   # default team and field policies for imports/creates
```

## Command map

```bash
{{CLI_PROGRAM}} b update-state Done -i LIN-1,LIN-2        # bulk (assign/unassign/label)
{{CLI_PROGRAM}} import csv issues.csv -t ENG --dry-run    # import (also: import json)
{{CLI_PROGRAM}} export csv -t ENG -f issues.csv           # export (json --pretty, projects-csv)
{{CLI_PROGRAM}} d list                                    # documents (get/create/update/delete)
{{CLI_PROGRAM}} att list LIN-123                          # attachments (link-url/create/update/delete)
{{CLI_PROGRAM}} up fetch URL -f /tmp/shot.png             # download uploads/images for inspection
```

## Agent notes

- Bulk commands execute immediately; verify the comma-separated `-i` issue list before running.
- Run imports with `--dry-run` first to validate the file; apply the same context defaults and field policies as `i create`.
- Upload URLs live in issue bodies and comments: `{{CLI_PROGRAM}} cm list LIN-123 --output json --compact --fields body,url`.
- Use `up fetch -f PATH` when the next tool needs a file path to an image.
- Full syntax: `{{CLI_PROGRAM}} b --help`, `{{CLI_PROGRAM}} import csv --help`, `{{CLI_PROGRAM}} att --help`, etc.
