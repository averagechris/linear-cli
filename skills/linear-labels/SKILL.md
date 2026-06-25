---
name: linear-labels
description: Manage Linear labels. Use when creating, listing, or deleting labels.
allowed-tools: Bash
---

# Labels

Manage issue/project labels.

## Start here

```bash
linear l list
linear l list --type issue --output json --compact
linear l create "bug" --color "#EF4444"
linear l delete LABEL_ID --force
```

## Agent notes
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `linear l list --help`, `linear l create --help`.
