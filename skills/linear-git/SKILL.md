---
name: linear-git
description: Git operations with Linear. Use for branches, checkout, and PRs.
allowed-tools: Bash
---

# Git Integration

Create branches/bookmarks and discover branch context for issues.

## Start here

```bash
linear g branch LIN-123
linear g checkout LIN-123
linear g checkout LIN-123 --vcs jj
linear context --output json --compact
```

## Agent notes
- Use `--vcs jj` in jj workspaces when needed.
- Use `linear g --help` for branch naming options.
- Full syntax and less-common flags: `linear g checkout --help`.
