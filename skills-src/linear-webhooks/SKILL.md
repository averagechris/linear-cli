---
name: linear-webhooks
description: Manage webhooks - create, listen for events, rotate secrets. Use when setting up integrations or event listeners.
allowed-tools: Bash
---

# Webhooks

```bash
# List all webhooks
{{CLI_PROGRAM}} wh list

# Create a webhook
{{CLI_PROGRAM}} wh create https://example.com/hook --events Issue

# Get webhook details
{{CLI_PROGRAM}} wh get WEBHOOK_ID

# Update a webhook
{{CLI_PROGRAM}} wh update WEBHOOK_ID --url https://new-url.com

# Delete a webhook
{{CLI_PROGRAM}} wh delete WEBHOOK_ID --force

# Rotate signing secret
{{CLI_PROGRAM}} wh rotate-secret WEBHOOK_ID

# Listen for events locally (dev/testing)
{{CLI_PROGRAM}} wh listen --port 9000
{{CLI_PROGRAM}} wh listen --port 9000 --secret SIGNING_SECRET
```

## Subcommands

| Command | Purpose |
|---------|---------|
| `list` | List all webhooks |
| `get` | View webhook details |
| `create` | Create webhook |
| `update` | Update webhook |
| `delete` | Delete webhook |
| `rotate-secret` | Rotate signing secret |
| `listen` | Local event listener with HMAC verification |

## Flags

| Flag | Purpose |
|------|---------|
| `--events TYPE` | Event types to subscribe |
| `--port N` | Local listener port |
| `--secret KEY` | HMAC signing secret |
| `--output json` | JSON output |

## Exit Codes

`0`=Success, `1`=Error, `2`=Not found, `3`=Auth error
