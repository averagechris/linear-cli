---
name: linear-attachments
description: Manage issue attachments - link URLs, create, update, delete. Use when attaching files or links to issues.
allowed-tools: Bash
---

# Attachments

List, link, create, update, and delete issue attachments.

## Start here

```bash
linear att list LIN-123
linear att get ATTACHMENT_ID --output json --compact
linear att link-url LIN-123 https://example.com
linear att delete ATTACHMENT_ID --force
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `linear att list --help`, `linear att create --help`.
