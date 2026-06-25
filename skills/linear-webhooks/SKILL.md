---
name: linear-webhooks
description: Manage webhooks - create, listen for events, rotate secrets. Use when setting up integrations or event listeners.
allowed-tools: Bash
---

# Webhooks

Manage Linear webhooks and local event listening.

## Start here

```bash
linear webhooks list
linear wh create https://example.com/linear --events Issue
linear wh rotate-secret WEBHOOK_ID
linear wh listen --port 3000
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `linear webhooks --help`.
