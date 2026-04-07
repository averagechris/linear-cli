---
name: linear-metrics
description: View Linear metrics. Use for velocity, burndown, and progress tracking.
allowed-tools: Bash
---

# Metrics

```bash
# Cycle metrics (velocity, burndown)
linear mt cycle CYCLE_ID
linear mt cycle CYCLE_ID --output json

# Project progress
linear mt project PROJECT_ID
linear mt project PROJECT_ID --output json

# Team velocity over time
linear mt velocity TEAM_KEY
linear mt velocity ENG --cycles 5    # Last 5 cycles
```

## Flags

| Flag | Purpose |
|------|---------|
| `--cycles N` | Number of cycles |
| `--output json` | JSON output |
