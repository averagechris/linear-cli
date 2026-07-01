---
name: linear-git
description: Git and GitHub integration for Linear - create branches, checkout issues, resolve the current branch issue, and open linked pull requests. Use when branching for an issue or creating a PR.
allowed-tools: Bash
---

# Git & Pull Requests

Branch/bookmark creation, issue checkout, branch-to-issue resolution, and linked GitHub PRs.

## Start with context

```bash
{{CLI_PROGRAM}} context --output json --compact   # includes the issue detected from the current branch
```

## Command map

```bash
{{CLI_PROGRAM}} g branch LIN-123            # print the branch name for an issue
{{CLI_PROGRAM}} g checkout LIN-123          # create/switch branch (--vcs jj in jj workspaces)
{{CLI_PROGRAM}} g pr LIN-123                # create linked GitHub PR (--draft, --base main)
{{CLI_PROGRAM}} i start LIN-123 --checkout  # assign + branch in one step
```

## Agent notes

- PR creation requires `gh` and a branch/bookmark pushed to GitHub.
- When the user says "this issue" or "my current issue", resolve it from the branch with `{{CLI_PROGRAM}} context` instead of asking.
- Full syntax: `{{CLI_PROGRAM}} g checkout --help`, `{{CLI_PROGRAM}} g pr --help`.
