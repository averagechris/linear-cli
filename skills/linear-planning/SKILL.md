---
name: linear-planning
description: Linear planning surfaces - projects, project status updates, milestones, initiatives, roadmaps, cycles, and sprint analytics. Use when managing projects, milestones, sprints, cycles, roadmaps, or posting project health updates.
allowed-tools: Bash
---

# Planning

Projects, milestones, initiatives, roadmaps, cycles, and sprint planning/analytics.

## Start with context

```bash
linear context --output json --compact              # default team, project policies
linear context options projects --output json --compact
linear context options initiatives --output json --compact
```

## Command map

```bash
linear p list                                       # projects (get/create/update/members/archive)
linear p update ID --status Done -l platform        # project status + project labels (add/remove/set-labels too)
linear pu create PROJECT_ID -b "On track" -H onTrack # project updates (list/get/archive)
linear ms list -p PROJECT_ID                        # milestones (create/update/delete)
linear init list                                    # initiatives (get/create/update; --target-date/--owner)
linear iu create INITIATIVE_ID -b "On track" -H onTrack # initiative updates (list/get/archive)
linear rm list                                      # roadmaps (get/create/update)
linear c current -t ENG --output json --compact     # cycles (list/create/complete)
linear sp status -t ENG                             # sprint (progress/plan/velocity/carry-over)
```

## Agent notes

- For parsing, add `--output json --compact`; trim payloads with `--fields a,b.c`.
- `--dry-run` (preview) and `--id-only` (print just the resulting ID) work where a command's `--help` documents them.
- `sp carry-over` moves issues between cycles; confirm with the user before `--force`.
- Full syntax: `linear p --help`, `linear sp --help`, `linear c --help`, etc.
