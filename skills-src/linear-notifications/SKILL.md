---
name: linear-notifications
description: Manage Linear notifications. Use for viewing, reading, and archiving notifications.
allowed-tools: Bash
---

# Notifications

Read, mark, and archive Linear notifications.

## Start here

```bash
{{CLI_PROGRAM}} notifications list
{{CLI_PROGRAM}} n list --output json --compact
{{CLI_PROGRAM}} n read NOTIFICATION_ID
{{CLI_PROGRAM}} n archive NOTIFICATION_ID
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} n list --help`, `{{CLI_PROGRAM}} n archive --help`.
