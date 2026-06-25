---
name: linear-git
description: Git operations with Linear. Use for branches, checkout, and PRs.
allowed-tools: Bash
---

# Git Integration

Create branches/bookmarks and discover branch context for issues.

## Start here

```bash
{{CLI_PROGRAM}} g branch LIN-123
{{CLI_PROGRAM}} g checkout LIN-123
{{CLI_PROGRAM}} g checkout LIN-123 --vcs jj
{{CLI_PROGRAM}} context --output json --compact
```

## Agent notes
- Use `--vcs jj` in jj workspaces when needed.
- Use `{{CLI_PROGRAM}} g --help` for branch naming options.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} g checkout --help`.
