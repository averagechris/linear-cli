---
name: release-process
description: Review commits since the last SourceHut tag, choose the next semver version from Conventional Commit messages, and publish a release tag for this fork.
allowed-tools: Bash, Read, Grep
---

# Release Process

Use this skill when cutting a new release for this hardened fork.

## 1. Find the last release tag

```bash
jj log -r 'tags()' --no-pager --color=never --no-graph -T 'ref_names ++ "\n"'
# or, if needed:
git tag --sort=-version:refname
```

Use the latest `vX.Y.Z` tag as the release baseline.

## 2. Review commits since that tag

```bash
jj log -r '<last-tag>::@-' --no-pager --color=never --no-graph -T 'description.first_line() ++ "\n"'
# or:
git log --format=%s <last-tag>..HEAD
```

Commit messages in this fork should follow Conventional Commits.

## 3. Choose the next version from commit messages

Use the highest bump implied by commits since the last tag:

- **major** — any commit with `!` after type/scope, or a `BREAKING CHANGE:` footer
- **minor** — any `feat:` commit when there is no breaking change
- **patch** — `fix:`, `perf:`, `refactor:`, `build:`, `docs:`, `test:`, `chore:` when there is no higher bump

If no releasable commits exist, stop and explain why rather than tagging anyway.

## 4. Update `Cargo.toml`

Set `[package].version` in `Cargo.toml` to the chosen next version.

```bash
grep '^version = ' Cargo.toml
```

The release tag helper reads the version directly from `Cargo.toml`, so the file must be updated first.

## 5. Validate before tagging

```bash
nix flake check --no-write-lock-file
nix run .#ci-test
nix run .#ci-clippy
```

## 6. Publish the tag

```bash
nix run .#release-tag
```

This creates and pushes an annotated SourceHut tag matching `Cargo.toml`'s version, normalized to `vX.Y.Z`.

## 7. Suggested prompts

- "Review commits since the last tag and tell me the next release version"
- "Choose the next semver bump from Conventional Commit messages"
- "Prepare this fork for release and tell me whether `Cargo.toml` is ready for `nix run .#release-tag`"
