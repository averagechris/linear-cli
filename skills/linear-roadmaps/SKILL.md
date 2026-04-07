---
name: linear-roadmaps
description: View Linear roadmaps. Use when viewing roadmap planning.
allowed-tools: Bash
---

# Roadmaps

```bash
# List roadmaps
linear rm list
linear rm list --output json

# Get roadmap details
linear rm get ROADMAP_ID
linear rm get ROADMAP_ID --output json
```

## Flags

| Flag | Purpose |
|------|---------|
| `--output json` | JSON output |
| `--compact` | No formatting |
