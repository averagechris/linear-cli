---
name: linear-initiatives
description: View Linear initiatives. Use for high-level tracking across projects.
allowed-tools: Bash
---

# Initiatives

```bash
# List initiatives
linear init list
linear init list --output json

# Get initiative details
linear init get INITIATIVE_ID
linear init get INITIATIVE_ID --output json
```

## Flags

| Flag | Purpose |
|------|---------|
| `--output json` | JSON output |
| `--compact` | No formatting |
