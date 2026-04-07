---
name: linear-notifications
description: Manage Linear notifications. Use for viewing, reading, and archiving notifications.
allowed-tools: Bash
---

# Notifications

```bash
# List unread notifications
{{CLI_PROGRAM}} n list
{{CLI_PROGRAM}} n list --output json

# Get unread count
{{CLI_PROGRAM}} n count

# Mark as read
{{CLI_PROGRAM}} n read NOTIFICATION_ID

# Mark all as read
{{CLI_PROGRAM}} n read-all

# Archive a notification
{{CLI_PROGRAM}} n archive NOTIFICATION_ID

# Archive all notifications
{{CLI_PROGRAM}} n archive-all
```

## Flags

| Flag | Purpose |
|------|---------|
| `--output json` | JSON output |
