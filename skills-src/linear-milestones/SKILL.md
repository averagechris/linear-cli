---
name: linear-milestones
description: Manage project milestones - create, update, track target dates. Use when planning project deliverables.
allowed-tools: Bash
---

# Milestones

Manage project milestones and target dates.

## Start here

```bash
{{CLI_PROGRAM}} ms list -p PROJECT_ID
{{CLI_PROGRAM}} ms create "Beta" -p PROJECT_ID
{{CLI_PROGRAM}} ms update MILESTONE_ID --name "GA"
{{CLI_PROGRAM}} ms delete MILESTONE_ID
```

## Agent notes
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} ms create --help`, `{{CLI_PROGRAM}} ms update --help`.
