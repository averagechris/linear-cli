---
name: linear-planning
description: Linear planning surfaces - projects, project status updates, milestones, initiatives, roadmaps, cycles, and sprint analytics. Use when managing projects, milestones, sprints, cycles, roadmaps, or posting project health updates.
allowed-tools: Bash
---

# Planning

Projects, milestones, initiatives, roadmaps, cycles, and sprint planning/analytics.

## Start with context

```bash
{{CLI_PROGRAM}} context --output json --compact              # default team, project policies
{{CLI_PROGRAM}} context options projects --output json --compact
{{CLI_PROGRAM}} context options initiatives --output json --compact
```

## Command map

```bash
{{CLI_PROGRAM}} p list                                       # projects (get/create/update/members/archive)
{{CLI_PROGRAM}} pu create PROJECT_ID -b "On track"           # project updates (list/get/archive)
{{CLI_PROGRAM}} ms list -p PROJECT_ID                        # milestones (create/update/delete)
{{CLI_PROGRAM}} init list                                    # initiatives (get/create/update)
{{CLI_PROGRAM}} rm list                                      # roadmaps (get/create/update)
{{CLI_PROGRAM}} c current -t ENG --output json --compact     # cycles (list/create/complete)
{{CLI_PROGRAM}} sp status -t ENG                             # sprint (progress/plan/velocity/carry-over)
```

## Agent notes

- For parsing, add `--output json --compact`; trim payloads with `--fields a,b.c`.
- `--dry-run` (preview) and `--id-only` (print just the resulting ID) work where a command's `--help` documents them.
- `sp carry-over` moves issues between cycles; confirm with the user before `--force`.
- Full syntax: `{{CLI_PROGRAM}} p --help`, `{{CLI_PROGRAM}} sp --help`, `{{CLI_PROGRAM}} c --help`, etc.
