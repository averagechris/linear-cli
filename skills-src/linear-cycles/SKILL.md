---
name: linear-cycles
description: Manage Linear sprint cycles - list, create, update, delete, complete. Use when managing cycles.
allowed-tools: Bash
---

# Cycles

```bash
# List cycles
{{CLI_PROGRAM}} c list -t ENG             # Team cycles
{{CLI_PROGRAM}} c list -t ENG --output json

# Current cycle
{{CLI_PROGRAM}} c current -t ENG
{{CLI_PROGRAM}} c current -t ENG --output json

# Create cycle
{{CLI_PROGRAM}} c create -t ENG --name "Sprint 5"
{{CLI_PROGRAM}} c create -t ENG --name "Sprint 5" --starts-at 2024-01-01 --ends-at 2024-01-14

# Get cycle details
{{CLI_PROGRAM}} c get CYCLE_ID

# Update cycle
{{CLI_PROGRAM}} c update CYCLE_ID --name "Sprint 5b"
{{CLI_PROGRAM}} c update CYCLE_ID --description "Updated goals" --dry-run

# Complete a cycle
{{CLI_PROGRAM}} c complete CYCLE_ID

# Delete cycle
{{CLI_PROGRAM}} c delete CYCLE_ID --force
```

## Flags

| Flag | Purpose |
|------|---------|
| `--output json` | JSON output |
| `--compact` | No formatting |
| `--dry-run` | Preview without updating |
| `--force` | Skip delete confirmation |
