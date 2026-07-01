---
name: linear-organization
description: Linear workspace structure - teams, users, labels, workflow statuses, issue templates, saved views, and favorites. Use when listing teams or members, managing labels or statuses, applying templates, or working with saved views.
allowed-tools: Bash
---

# Organization

Teams, users, labels, workflow states, templates, views, and favorites.

## Start with context

```bash
{{CLI_PROGRAM}} context --output json --compact                 # default team and label policies
{{CLI_PROGRAM}} context options labels --output json --compact  # cached label options (--refresh if stale)
```

## Command map

```bash
{{CLI_PROGRAM}} t list                                   # teams (get/members/create)
{{CLI_PROGRAM}} u me                                     # users (u list)
{{CLI_PROGRAM}} l list --type issue                      # labels (create --color, delete --force)
{{CLI_PROGRAM}} st list -t ENG                           # workflow statuses (get/update)
{{CLI_PROGRAM}} tpl list                                 # templates (show NAME; i create --template NAME)
{{CLI_PROGRAM}} views list                               # saved views (i list --view "My Sprint")
{{CLI_PROGRAM}} fav list                                 # favorites (add/remove)
```

## Agent notes

- For parsing, add `--output json --compact`; trim payloads with `--fields a,b.c`.
- `--dry-run` (preview) and `--id-only` (print just the resulting ID) work where a command's `--help` documents them.
- Statuses are team-scoped: pass `-t TEAM` (or rely on the context default team).
- Full syntax: `{{CLI_PROGRAM}} t --help`, `{{CLI_PROGRAM}} l --help`, `{{CLI_PROGRAM}} st --help`, etc.
