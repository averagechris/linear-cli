---
name: linear-favorites
description: Manage Linear favorites. Use for quick access to issues and projects.
allowed-tools: Bash
---

# Favorites

```bash
# List favorites
{{CLI_PROGRAM}} fav list
{{CLI_PROGRAM}} fav list --output json

# Add to favorites
{{CLI_PROGRAM}} fav add LIN-123           # Add issue
{{CLI_PROGRAM}} fav add PROJECT_ID        # Add project

# Remove from favorites
{{CLI_PROGRAM}} fav remove LIN-123
```

## Flags

| Flag | Purpose |
|------|---------|
| `--output json` | JSON output |
