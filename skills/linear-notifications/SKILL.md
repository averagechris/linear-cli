---
name: linear-notifications
description: Manage Linear notifications. Use for viewing, reading, and archiving notifications.
allowed-tools: Bash
---

# Notifications

Read, mark, and archive Linear notifications.

## Start here

```bash
linear notifications list
linear n list --output json --compact
linear n read NOTIFICATION_ID
linear n archive NOTIFICATION_ID
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `linear n list --help`, `linear n archive --help`.
