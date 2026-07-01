---
name: linear-organization
description: Linear workspace structure - teams, users, labels, workflow statuses, issue templates, saved views, and favorites. Use when listing teams or members, managing labels or statuses, applying templates, or working with saved views.
allowed-tools: Bash
---

# Organization

Teams, users, labels, workflow states, templates, views, and favorites.

## Start with context

```bash
linear context --output json --compact                 # default team and label policies
linear context options labels --output json --compact  # cached label options (--refresh if stale)
```

## Command map

```bash
linear t list                                   # teams (get/members/create)
linear u me                                     # users (u list)
linear l list --type issue                      # labels (create --color, delete --force)
linear st list -t ENG                           # workflow statuses (get/update)
linear tpl list                                 # templates (show NAME; i create --template NAME)
linear views list                               # saved views (i list --view "My Sprint")
linear fav list                                 # favorites (add/remove)
```

## Agent notes

- For parsing, add `--output json --compact`; trim payloads with `--fields a,b.c`.
- `--dry-run` (preview) and `--id-only` (print just the resulting ID) work where a command's `--help` documents them.
- Statuses are team-scoped: pass `-t TEAM` (or rely on the context default team).
- Full syntax: `linear t --help`, `linear l --help`, `linear st --help`, etc.
