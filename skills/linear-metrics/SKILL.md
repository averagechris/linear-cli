---
name: linear-metrics
description: View Linear metrics. Use for velocity, burndown, and progress tracking.
allowed-tools: Bash
---

# Metrics

Inspect velocity, burndown, and progress metrics.

## Start here

```bash
linear metrics velocity ENG
linear metrics cycle CYCLE_ID
linear metrics project PROJECT_ID --output json --compact
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- Full syntax and less-common flags: `linear metrics --help`.
