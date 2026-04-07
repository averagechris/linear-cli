---
name: linear-notifications
description: Manage Linear notifications. Use for viewing, reading, and archiving notifications.
allowed-tools: Bash
---

# Notifications

```bash
# List unread notifications
linear n list
linear n list --output json

# Get unread count
linear n count

# Mark as read
linear n read NOTIFICATION_ID

# Mark all as read
linear n read-all

# Archive a notification
linear n archive NOTIFICATION_ID

# Archive all notifications
linear n archive-all
```

## Flags

| Flag | Purpose |
|------|---------|
| `--output json` | JSON output |
