---
name: linear-pr
description: Create GitHub PRs linked to Linear issues. Use when creating pull requests, pushing code for review, or linking PRs to Linear issues.
allowed-tools: Bash
---

# Pull Requests

Create GitHub PRs linked to Linear issues.

## Start here

```bash
{{CLI_PROGRAM}} g pr LIN-123
{{CLI_PROGRAM}} g pr LIN-123 --draft
{{CLI_PROGRAM}} g pr LIN-123 --base main
{{CLI_PROGRAM}} context --output json --compact
```

## Agent notes
- Requires `gh` and a branch/bookmark pushed to GitHub.
- Use `{{CLI_PROGRAM}} context` first if the issue ID is implied by the branch.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} g pr --help`.
