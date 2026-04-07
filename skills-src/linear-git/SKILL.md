---
name: linear-git
description: Git operations with Linear. Use for branches, checkout, and PRs.
allowed-tools: Bash
---

# Git Operations

```bash
# Checkout branch for issue (creates if needed)
{{CLI_PROGRAM}} g checkout LIN-123

# Show branch name
{{CLI_PROGRAM}} g branch LIN-123

# Create branch without checkout
{{CLI_PROGRAM}} g create LIN-123

# Create GitHub PR from Linear issue
{{CLI_PROGRAM}} g pr LIN-123
{{CLI_PROGRAM}} g pr LIN-123 --draft      # Draft PR
{{CLI_PROGRAM}} g pr LIN-123 --base main  # Specify base branch

# jj (Jujutsu) - show commits with Linear trailers
{{CLI_PROGRAM}} g commits
```

## Context

```bash
# Get issue from current branch
{{CLI_PROGRAM}} context
{{CLI_PROGRAM}} context --output json
```

## Flags

| Flag | Purpose |
|------|---------|
| `--draft` | Create draft PR |
| `--base BRANCH` | Base branch |
| `--output json` | JSON output |
