---
name: linear-attachments
description: Manage issue attachments - link URLs, create, update, delete. Use when attaching files or links to issues.
allowed-tools: Bash
---

# Attachments

List, link, create, update, and delete issue attachments.

## Start here

```bash
{{CLI_PROGRAM}} att list LIN-123
{{CLI_PROGRAM}} att get ATTACHMENT_ID --output json --compact
{{CLI_PROGRAM}} att link-url LIN-123 https://example.com
{{CLI_PROGRAM}} att delete ATTACHMENT_ID --force
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} att list --help`, `{{CLI_PROGRAM}} att create --help`.
