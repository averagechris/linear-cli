---
name: linear-metrics
description: View Linear metrics. Use for velocity, burndown, and progress tracking.
allowed-tools: Bash
---

# Metrics

Inspect velocity, burndown, and progress metrics.

## Start here

```bash
{{CLI_PROGRAM}} metrics velocity ENG
{{CLI_PROGRAM}} metrics cycle CYCLE_ID
{{CLI_PROGRAM}} metrics project PROJECT_ID --output json --compact
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} metrics --help`.
