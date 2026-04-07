---
name: linear-labels
description: Manage Linear labels. Use when creating, listing, or deleting labels.
allowed-tools: Bash
---

# Labels

```bash
# List labels
linear l list                    # Project labels
linear l list --type issue       # Issue labels

# Create label
linear l create "Feature" --color "#10B981"
linear l create "Bug" --color "#EF4444" --id-only

# Delete label
linear l delete LABEL_ID
linear l delete LABEL_ID --force

# Agent-optimized
linear l list --output json --compact
```

## Flags

| Flag | Purpose |
|------|---------|
| `--id-only` | Return ID only |
| `--output json` | JSON output |
| `--force` | Skip confirmation |
