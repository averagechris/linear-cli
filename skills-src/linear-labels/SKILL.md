---
name: linear-labels
description: Manage Linear labels. Use when creating, listing, or deleting labels.
allowed-tools: Bash
---

# Labels

Manage issue/project labels.

## Start here

```bash
{{CLI_PROGRAM}} l list
{{CLI_PROGRAM}} l list --type issue --output json --compact
{{CLI_PROGRAM}} l create "bug" --color "#EF4444"
{{CLI_PROGRAM}} l delete LABEL_ID --force
```

## Agent notes
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} l list --help`, `{{CLI_PROGRAM}} l create --help`.
