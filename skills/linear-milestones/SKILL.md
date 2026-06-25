---
name: linear-milestones
description: Manage project milestones - create, update, track target dates. Use when planning project deliverables.
allowed-tools: Bash
---

# Milestones

Manage project milestones and target dates.

## Start here

```bash
linear ms list -p PROJECT_ID
linear ms create "Beta" -p PROJECT_ID
linear ms update MILESTONE_ID --name "GA"
linear ms delete MILESTONE_ID
```

## Agent notes
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `linear ms create --help`, `linear ms update --help`.
