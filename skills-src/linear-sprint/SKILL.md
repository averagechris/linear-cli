---
name: linear-sprint
description: Sprint planning and analytics - status, progress, burndown, velocity, carry-over. Use when managing sprints.
allowed-tools: Bash
---

# Sprint Planning

Plan and analyze the current or next cycle.

## Start here

```bash
{{CLI_PROGRAM}} sp status -t ENG
{{CLI_PROGRAM}} sp progress -t ENG
{{CLI_PROGRAM}} sp plan -t ENG --output json --compact
{{CLI_PROGRAM}} sp carry-over -t ENG --force
{{CLI_PROGRAM}} sp velocity -t ENG -n 10
```

## Agent notes
- Full syntax and less-common flags: `{{CLI_PROGRAM}} sp status --help`, `{{CLI_PROGRAM}} sp velocity --help`.
