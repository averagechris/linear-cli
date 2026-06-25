---
name: linear-sprint
description: Sprint planning and analytics - status, progress, burndown, velocity, carry-over. Use when managing sprints.
allowed-tools: Bash
---

# Sprint Planning

Plan and analyze the current or next cycle.

## Start here

```bash
linear sp status -t ENG
linear sp progress -t ENG
linear sp plan -t ENG --output json --compact
linear sp carry-over -t ENG --force
linear sp velocity -t ENG -n 10
```

## Agent notes
- Full syntax and less-common flags: `linear sp status --help`, `linear sp velocity --help`.
