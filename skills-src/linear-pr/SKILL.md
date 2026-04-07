---
name: linear-pr
description: Create GitHub PRs linked to Linear issues. Use when creating pull requests, pushing code for review, or linking PRs to Linear issues.
allowed-tools: Bash
---

# Linear PR Creation

Create GitHub pull requests linked to Linear issues using `linear`.

## Create PR

```bash
# Create PR linked to Linear issue
{{CLI_PROGRAM}} g pr LIN-123

# Draft PR
{{CLI_PROGRAM}} g pr LIN-123 --draft

# Specify base branch
{{CLI_PROGRAM}} g pr LIN-123 --base main

# Open in browser after creation
{{CLI_PROGRAM}} g pr LIN-123 --web
```

## Git Branch Operations

```bash
# Create and checkout branch for issue
{{CLI_PROGRAM}} g checkout LIN-123

# Custom branch name
{{CLI_PROGRAM}} g checkout LIN-123 -b my-custom-branch

# Just show branch name (don't create)
{{CLI_PROGRAM}} g branch LIN-123

# Create branch without checkout
{{CLI_PROGRAM}} g create LIN-123
```

## Complete Workflow

```bash
# 1. Start work (assigns, sets In Progress, creates branch)
{{CLI_PROGRAM}} i start LIN-123 --checkout

# 2. Make changes and commit
git add . && git commit -m "Fix the bug"

# 3. Create PR
{{CLI_PROGRAM}} g pr LIN-123

# 4. Mark done
{{CLI_PROGRAM}} i update LIN-123 -s Done
```

## Get Current Issue

```bash
# Get issue from current git branch
{{CLI_PROGRAM}} context
{{CLI_PROGRAM}} context --output json
```

## Tips

- PR title/description auto-generated from issue
- Use `--draft` for work-in-progress
- Branch pattern: `username/lin-123-issue-title`
- Requires `gh` CLI for GitHub operations
