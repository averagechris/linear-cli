---
name: linear-time
description: Track time on Linear issues. Use for logging and viewing time entries.
allowed-tools: Bash
---

# Time Tracking

```bash
# Log time
{{CLI_PROGRAM}} tm log LIN-123 2h             # Log 2 hours
{{CLI_PROGRAM}} tm log LIN-123 30m            # Log 30 minutes
{{CLI_PROGRAM}} tm log LIN-123 1h30m          # Log 1.5 hours

# List time entries
{{CLI_PROGRAM}} tm list --issue LIN-123
{{CLI_PROGRAM}} tm list --output json

# Delete entry
{{CLI_PROGRAM}} tm delete ENTRY_ID
```

## Duration Format

`30m`, `1h`, `2h30m`, `1d` (8 hours)

## Flags

| Flag | Purpose |
|------|---------|
| `--issue ID` | Filter by issue |
| `--output json` | JSON output |
