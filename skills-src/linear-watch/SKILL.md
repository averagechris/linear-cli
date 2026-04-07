---
name: linear-watch
description: Watch Linear issues for changes. Use for monitoring updates.
allowed-tools: Bash
---

# Watch

```bash
# Watch single issue (polls for changes)
{{CLI_PROGRAM}} watch LIN-123

# Custom interval
{{CLI_PROGRAM}} watch LIN-123 --interval 30   # Poll every 30 seconds
{{CLI_PROGRAM}} watch LIN-123 -i 60           # Poll every 60 seconds

# JSON output for scripting
{{CLI_PROGRAM}} watch LIN-123 --output json
```

## Flags

| Flag | Purpose |
|------|---------|
| `-i, --interval` | Seconds between polls (default: 10) |
| `--output json` | JSON output |
