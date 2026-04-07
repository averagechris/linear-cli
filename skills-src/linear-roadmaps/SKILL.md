---
name: linear-roadmaps
description: View Linear roadmaps. Use when viewing roadmap planning.
allowed-tools: Bash
---

# Roadmaps

```bash
# List roadmaps
{{CLI_PROGRAM}} rm list
{{CLI_PROGRAM}} rm list --output json

# Get roadmap details
{{CLI_PROGRAM}} rm get ROADMAP_ID
{{CLI_PROGRAM}} rm get ROADMAP_ID --output json
```

## Flags

| Flag | Purpose |
|------|---------|
| `--output json` | JSON output |
| `--compact` | No formatting |
