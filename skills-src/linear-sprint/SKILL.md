---
name: linear-sprint
description: Sprint planning and analytics - status, progress, burndown, velocity, carry-over. Use when managing sprints.
allowed-tools: Bash
---

# Sprint Planning

```bash
# Current sprint status
{{CLI_PROGRAM}} sp status -t ENG

# Sprint progress (completion bar)
{{CLI_PROGRAM}} sp progress -t ENG

# Plan next sprint (view planned issues)
{{CLI_PROGRAM}} sp plan -t ENG

# Carry over incomplete issues to next cycle
{{CLI_PROGRAM}} sp carry-over -t ENG --force

# ASCII burndown chart
{{CLI_PROGRAM}} sp burndown -t ENG

# Velocity across recent sprints
{{CLI_PROGRAM}} sp velocity -t ENG
{{CLI_PROGRAM}} sp velocity -t ENG --cycles 10    # Last 10 sprints
{{CLI_PROGRAM}} sp velocity -t ENG --output json
```

## Subcommands

| Command | Purpose |
|---------|---------|
| `status` | Current sprint overview |
| `progress` | Visual completion bar |
| `plan` | Next cycle's planned issues |
| `carry-over` | Move incomplete to next cycle |
| `burndown` | ASCII burndown chart |
| `velocity` | Sprint velocity + trend analysis |

## Flags

| Flag | Purpose |
|------|---------|
| `-t TEAM` | Team key (required) |
| `--cycles N` | Number of past cycles for velocity |
| `--force` | Skip carry-over confirmation |
| `--output json` | JSON output |

## Exit Codes

`0`=Success, `1`=Error, `2`=Not found, `3`=Auth error
