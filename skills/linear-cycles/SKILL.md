---
name: linear-cycles
description: Manage Linear sprint cycles - list, create, update, delete, complete. Use when managing cycles.
allowed-tools: Bash
---

# Cycles

```bash
# List cycles
linear c list -t ENG             # Team cycles
linear c list -t ENG --output json

# Current cycle
linear c current -t ENG
linear c current -t ENG --output json

# Create cycle
linear c create -t ENG --name "Sprint 5"
linear c create -t ENG --name "Sprint 5" --starts-at 2024-01-01 --ends-at 2024-01-14

# Get cycle details
linear c get CYCLE_ID

# Update cycle
linear c update CYCLE_ID --name "Sprint 5b"
linear c update CYCLE_ID --description "Updated goals" --dry-run

# Complete a cycle
linear c complete CYCLE_ID

# Delete cycle
linear c delete CYCLE_ID --force
```

## Flags

| Flag | Purpose |
|------|---------|
| `--output json` | JSON output |
| `--compact` | No formatting |
| `--dry-run` | Preview without updating |
| `--force` | Skip delete confirmation |
