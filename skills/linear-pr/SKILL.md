---
name: linear-pr
description: Create GitHub PRs linked to Linear issues. Use when creating pull requests, pushing code for review, or linking PRs to Linear issues.
allowed-tools: Bash
---

# Pull Requests

Create GitHub PRs linked to Linear issues.

## Start here

```bash
linear g pr LIN-123
linear g pr LIN-123 --draft
linear g pr LIN-123 --base main
linear context --output json --compact
```

## Agent notes
- Requires `gh` and a branch/bookmark pushed to GitHub.
- Use `linear context` first if the issue ID is implied by the branch.
- Full syntax and less-common flags: `linear g pr --help`.
