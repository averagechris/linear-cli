---
name: release-process
description: Review commits since the last SourceHut tag, choose the next semver version from Conventional Commit messages, update changelog/download pages, build artifacts, and publish a release tag for this fork.
allowed-tools: Bash, Read, Grep, Edit, Write
---

# Release Process

Use this skill when cutting a new release for this hardened fork.

## Fast path

For the normal deterministic release flow, create/use the jj release change and run:

```bash
nix run .#release -- --version X.Y.Z
```

This prepares `Cargo.toml`, `CHANGELOG.md`, and `.builds/release-linux-x86_64.yml`, runs validation, tags/pushes `vX.Y.Z`, builds the local `.#release-artifact`, copies it into `dist/downloads/`, and builds `dist/pages/linear-cli-pages.tar.gz` for SourceHut Pages.

Optional flags:

```bash
nix run .#release -- --version X.Y.Z --publish-pages
nix run .#release -- --version X.Y.Z --submit-linux-build
```

- `--publish-pages` runs `hut pages publish` for `averagechris.srht.site` under `/linear-cli`.
- `--submit-linux-build` submits `.builds/release-linux-x86_64.yml`; the build creates the Linux artifact, merges it with existing hosted downloads, and republishes SourceHut Pages using build-scoped `pages.sr.ht/PAGES:RW` OAuth.

Use the manual steps below when you need more control or are recovering from a partial release.

## 1. Find the last release tag

```bash
jj tag list --no-pager --color=never
```

Use the latest `vX.Y.Z` tag as the release baseline.

## 2. Review commits since that tag

```bash
jj log -r '<last-tag>::@-' --no-pager --color=never --no-graph
```

If `@` is a working-copy change with content, use `<last-tag>::@` instead.

Commit messages in this fork should follow Conventional Commits.

## 3. Choose the next version from commit messages

Use the highest bump implied by commits since the last tag:

- **major** — any commit with `!` after type/scope, or a `BREAKING CHANGE:` footer
- **minor** — any `feat:` commit when there is no breaking change
- **patch** — `fix:`, `perf:`, `refactor:`, `build:`, `docs:`, `test:`, `chore:` when there is no higher bump

If no releasable commits exist, stop and explain why rather than tagging anyway.

## 4. Create a version bump commit

Create a new jj change for the version bump, update `Cargo.toml`, and describe it:

```bash
jj new -m 'chore: bump version to X.Y.Z'
```

Then prepare release metadata:

```bash
nix run .#prepare-release -- --version X.Y.Z --revision @
```

This sets `[package].version`, updates `CHANGELOG.md`, and rewrites the Linux SourceHut build manifest artifact names for the new version.

Verify:

```bash
grep '^version = ' Cargo.toml
grep '^## vX.Y.Z' CHANGELOG.md
```

## 5. Validate before tagging

```bash
nix flake check --no-write-lock-file
nix run .#ci-test
nix run .#ci-clippy
```

All three must pass before proceeding.

## 6. Tag and push

```bash
nix run .#release-tag
```

This script:
1. Reads the version from `Cargo.toml` and normalizes it to `vX.Y.Z`
2. Validates semver format
3. Checks that the tag doesn't already exist locally or on origin
4. In jj repos: runs `jj tag set vX.Y.Z --revision @`
5. Pushes the tag to origin with `git push`

### `--revision` flag

By default the script tags `@`. If you're sitting on a fresh empty child change after the version bump, pass the actual release revision:

```bash
nix run .#release-tag -- --revision @-
```

### If the push fails

The script prints the manual fallback command. Common reasons:
- SSH key not loaded — run `ssh-add` and retry
- Remote tag already exists — check with `jj tag list` and the remote

## 7. Move main bookmark

After tagging, move `main` forward and push it:

```bash
jj bookmark set main --revision vX.Y.Z
jj git push --remote origin --bookmark main
```

## 8. Build artifacts and pages

Build the local platform tarball and checksum:

```bash
nix build .#release-artifact --out-link result-release-artifact
mkdir -p dist/downloads
cp -p result-release-artifact/* dist/downloads/
```

For Linux, submit a SourceHut build after `.builds/release-linux-x86_64.yml` has the release version:

```bash
hut builds submit .builds/release-linux-x86_64.yml \
  --note 'linear-cli vX.Y.Z linux release' \
  --tags 'linear-cli/vX.Y.Z/release' \
  --visibility unlisted
```

The successful build artifacts are also published by SourceHut as short-lived job artifacts. For durable hosted downloads, the build merges its Linux artifact with the current Pages manifest and runs `hut pages publish` automatically.

For a local/manual pages publish, build and publish the SourceHut Pages archive:

```bash
nix run .#build-pages -- --include-existing-downloads
nix run .#publish-pages
```

Pages default to `https://averagechris.srht.site/linear-cli/`. Keep `CHANGELOG.md`, the downloads page, and README links in sync.

## 9. Suggested prompts

- "Review commits since the last tag and tell me the next release version"
- "Choose the next semver bump from Conventional Commit messages"
- "Prepare this fork for release"
