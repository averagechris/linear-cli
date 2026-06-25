---
name: linear-triage
description: Manage Linear triage inbox. Use for unassigned issues needing attention.
allowed-tools: Bash
---

# Triage

Work the triage inbox.

## Start here

```bash
{{CLI_PROGRAM}} triage list -t ENG
{{CLI_PROGRAM}} triage list -t ENG --output json --compact
{{CLI_PROGRAM}} triage claim LIN-123
{{CLI_PROGRAM}} triage snooze LIN-123 -d 1w
```

## Agent notes
- Full syntax and less-common flags: `{{CLI_PROGRAM}} triage list --help`, `{{CLI_PROGRAM}} triage claim --help`.
