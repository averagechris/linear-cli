---
name: linear-triage
description: Manage Linear triage inbox. Use for unassigned issues needing attention.
allowed-tools: Bash
---

# Triage

Work the triage inbox.

## Start here

```bash
linear triage list -t ENG
linear triage list -t ENG --output json --compact
linear triage claim LIN-123
linear triage snooze LIN-123 -d 1w
```

## Agent notes
- Full syntax and less-common flags: `linear triage list --help`, `linear triage claim --help`.
