---
name: linear-metrics
description: View Linear metrics. Use for velocity, burndown, and progress tracking.
allowed-tools: Bash
---

# Metrics

```bash
# Cycle metrics (velocity, burndown)
{{CLI_PROGRAM}} mt cycle CYCLE_ID
{{CLI_PROGRAM}} mt cycle CYCLE_ID --output json

# Project progress
{{CLI_PROGRAM}} mt project PROJECT_ID
{{CLI_PROGRAM}} mt project PROJECT_ID --output json

# Team velocity over time
{{CLI_PROGRAM}} mt velocity TEAM_KEY
{{CLI_PROGRAM}} mt velocity ENG --cycles 5    # Last 5 cycles
```

## Flags

| Flag | Purpose |
|------|---------|
| `--cycles N` | Number of cycles |
| `--output json` | JSON output |
