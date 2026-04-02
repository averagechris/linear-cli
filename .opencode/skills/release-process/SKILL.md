---
name: release-process
description: Review commits since the last SourceHut tag, choose the next semver version from Conventional Commit messages, and publish a release tag for this fork.
allowed-tools: Bash, Read, Grep
---

# Release Process

Use this skill when cutting a new release for this hardened fork.

## 1. Find the last release tag

```bash
jj tag list --no-pager --color=never
```

Use the latest `vX.Y.Z` tag as the release baseline.

## 2. Review commits since that tag

```bash
jj log -r '<last-tag>::@-' --no-pager --color=never --no-graph
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

### Release friction notes

- `nix run .#release-tag` creates an annotated tag from `Cargo.toml`'s version but does **not** push it. The agent should present the push command for the user to run.
- The helper reads `Cargo.toml` directly, so if `package.version` changes, refresh `Cargo.lock` by running the validation commands before tagging.
- If local git tag signing blocks automation, stop and show the user the exact fallback commands printed by `nix run .#release-tag`; the user must finish those manually because signing approval/repair is local-machine state.
- In jj repos, if you are sitting on a fresh empty child change, tag the intended release revision explicitly instead of trusting `@`. Common fallback:

```bash
jj tag create vX.Y.Z --revision @-
```

Use `@-` only when the release commit is the parent of your current empty working-copy change; otherwise specify the actual release revision.

## 6. Create the tag

```bash
nix run .#release-tag
```

This creates an annotated tag matching `Cargo.toml`'s version, normalized to `vX.Y.Z`.

## 7. Push the tag

The agent should **not** push automatically. Present the command for the user to run:

```bash
jj git push --remote origin --tag-pattern 'vX.Y.Z'
```

## 8. Suggested prompts

- "Review commits since the last tag and tell me the next release version"
- "Choose the next semver bump from Conventional Commit messages"
- "Prepare this fork for release and tell me whether `Cargo.toml` is ready for `nix run .#release-tag`"
