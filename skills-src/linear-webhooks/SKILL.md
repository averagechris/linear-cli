---
name: linear-webhooks
description: Manage webhooks - create, listen for events, rotate secrets. Use when setting up integrations or event listeners.
allowed-tools: Bash
---

# Webhooks

Manage Linear webhooks and local event listening.

## Start here

```bash
{{CLI_PROGRAM}} webhooks list
{{CLI_PROGRAM}} wh create https://example.com/linear --events Issue
{{CLI_PROGRAM}} wh rotate-secret WEBHOOK_ID
{{CLI_PROGRAM}} wh listen --port 3000
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} webhooks --help`.
