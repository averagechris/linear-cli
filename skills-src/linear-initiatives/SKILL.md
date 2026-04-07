---
name: linear-initiatives
description: View Linear initiatives. Use for high-level tracking across projects.
allowed-tools: Bash
---

# Initiatives

```bash
# List initiatives
{{CLI_PROGRAM}} init list
{{CLI_PROGRAM}} init list --output json

# Get initiative details
{{CLI_PROGRAM}} init get INITIATIVE_ID
{{CLI_PROGRAM}} init get INITIATIVE_ID --output json
```

## Flags

| Flag | Purpose |
|------|---------|
| `--output json` | JSON output |
| `--compact` | No formatting |
