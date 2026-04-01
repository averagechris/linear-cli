---
name: upstream-merge-review
description: Evaluate new upstream GitHub changes before merging them into the SourceHut fork. Use when syncing this fork with upstream.
allowed-tools: Bash, Read, Grep
---

# Upstream Merge Review Workflow

This repository treats GitHub as `upstream` and SourceHut as `origin`.

## 1. Refresh upstream refs

```bash
nix run .#fetch-upstream
```

In jj, Git's `refs/remotes/upstream/master` appears as `master@upstream`.

## 2. Review what upstream added

```bash
jj log -r 'main::master@upstream' --no-pager --color=never --no-graph
jj show master@upstream --stat --no-pager --color=never
jj diff --from main --to master@upstream --no-pager --color=never
```

Focus on supply-chain risk before merging:

- new install/update paths
- release, CI, and packaging changes
- network access, auth, and credential handling
- scripts, hooks, and generated artifacts
- dependency and lockfile churn

## 3. Check local divergence

```bash
jj log -r 'master@upstream::main' --no-pager --color=never --no-graph
jj diff --from master@upstream --to main --no-pager --color=never
```

Classify local changes as one of:

1. Fork-only hardening that must be preserved
2. Temporary divergence that should be dropped
3. Conflicting behavior that needs a manual resolution plan

## 4. Merge only after review

Preferred policy:

1. summarize upstream commits and changed files
2. call out risky changes explicitly
3. propose a merge strategy before running mutating jj commands
4. after merge/rebase, rerun tests and inspect the resulting diff against `master@upstream`

## 5. Suggested prompts

- "Review upstream changes since our last sync"
- "Is it safe to merge upstream/master into our fork?"
- "Preserve our hardening and propose the cleanest jj merge plan"
