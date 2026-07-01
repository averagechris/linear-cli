---
name: linear-git
description: Git and GitHub integration for Linear - create branches, checkout issues, resolve the current branch issue, and open linked pull requests. Use when branching for an issue or creating a PR.
allowed-tools: Bash
---

# Git & Pull Requests

Branch/bookmark creation, issue checkout, branch-to-issue resolution, and linked GitHub PRs.

## Start with context

```bash
linear context --output json --compact   # includes the issue detected from the current branch
```

## Command map

```bash
linear g branch LIN-123            # print the branch name for an issue
linear g checkout LIN-123          # create/switch branch (--vcs jj in jj workspaces)
linear g pr LIN-123                # create linked GitHub PR (--draft, --base main)
linear i start LIN-123 --checkout  # assign + branch in one step
```

## Agent notes

- PR creation requires `gh` and a branch/bookmark pushed to GitHub.
- When the user says "this issue" or "my current issue", resolve it from the branch with `linear context` instead of asking.
- Full syntax: `linear g checkout --help`, `linear g pr --help`.
