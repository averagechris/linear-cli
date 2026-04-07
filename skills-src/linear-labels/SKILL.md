---
name: linear-labels
description: Manage Linear labels. Use when creating, listing, or deleting labels.
allowed-tools: Bash
---

# Labels

```bash
# List labels
{{CLI_PROGRAM}} l list                    # Project labels
{{CLI_PROGRAM}} l list --type issue       # Issue labels

# Create label
{{CLI_PROGRAM}} l create "Feature" --color "#10B981"
{{CLI_PROGRAM}} l create "Bug" --color "#EF4444" --id-only

# Delete label
{{CLI_PROGRAM}} l delete LABEL_ID
{{CLI_PROGRAM}} l delete LABEL_ID --force

# Agent-optimized
{{CLI_PROGRAM}} l list --output json --compact
```

## Flags

| Flag | Purpose |
|------|---------|
| `--id-only` | Return ID only |
| `--output json` | JSON output |
| `--force` | Skip confirmation |
