---
name: linear-git
description: Git operations with Linear. Use for branches, checkout, and PRs.
allowed-tools: Bash
---

# Git Operations

```bash
# Checkout branch for issue (creates if needed)
linear g checkout LIN-123

# Show branch name
linear g branch LIN-123

# Create branch without checkout
linear g create LIN-123

# Create GitHub PR from Linear issue
linear g pr LIN-123
linear g pr LIN-123 --draft      # Draft PR
linear g pr LIN-123 --base main  # Specify base branch

# jj (Jujutsu) - show commits with Linear trailers
linear g commits
```

## Context

```bash
# Get issue from current branch
linear context
linear context --output json
```

## Flags

| Flag | Purpose |
|------|---------|
| `--draft` | Create draft PR |
| `--base BRANCH` | Base branch |
| `--output json` | JSON output |
