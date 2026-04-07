---
name: linear-favorites
description: Manage Linear favorites. Use for quick access to issues and projects.
allowed-tools: Bash
---

# Favorites

```bash
# List favorites
linear fav list
linear fav list --output json

# Add to favorites
linear fav add LIN-123           # Add issue
linear fav add PROJECT_ID        # Add project

# Remove from favorites
linear fav remove LIN-123
```

## Flags

| Flag | Purpose |
|------|---------|
| `--output json` | JSON output |
