---
name: linear-templates
description: Manage issue templates - local templates and Linear API templates. Use when creating or using templates.
allowed-tools: Bash
---

# Templates

List and inspect local/API issue templates.

## Start here

```bash
{{CLI_PROGRAM}} tpl list
{{CLI_PROGRAM}} tpl list --output json --compact
{{CLI_PROGRAM}} tpl show bug
{{CLI_PROGRAM}} i create "Bug" -t ENG --template bug --dry-run
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} tpl list --help`, `{{CLI_PROGRAM}} tpl show --help`.
