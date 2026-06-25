---
name: linear-templates
description: Manage issue templates - local templates and Linear API templates. Use when creating or using templates.
allowed-tools: Bash
---

# Templates

List and inspect local/API issue templates.

## Start here

```bash
linear tpl list
linear tpl list --output json --compact
linear tpl show bug
linear i create "Bug" -t ENG --template bug --dry-run
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- Full syntax and less-common flags: `linear tpl list --help`, `linear tpl show --help`.
