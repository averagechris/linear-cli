---
name: release-process
description: Review commits since the last SourceHut tag, choose the next semver version from Conventional Commit messages, update changelog/download pages, build artifacts, and publish a release tag for this fork.
allowed-tools: Bash, Read, Grep, Edit, Write
---

# Release Process

Use this skill when cutting a new release for this hardened fork.

## Fast path

From an empty jj working-copy commit whose parent, local `main`, and
`main@origin` agree, first run the Tiny-safe, read-only preflight:

```bash
nix run .#release -- --version X.Y.Z --check
```

Then run the one normal deterministic release command:

```bash
nix run .#release -- --version X.Y.Z
```

This prepares `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, and `builds/release-linux-x86_64.yml`; runs the standard gates and `ci-skills-render` on the prepared tree; builds and verifies the release artifact and checksum before refs; atomically publishes leased `main` plus annotated `vX.Y.Z`; uploads the artifact; and submits the central Pages refresh. The separate Homebrew artifact remains available but is not substituted for the fleet release artifact.

If refs were published but upload or refresh failed, rerun the exact same
command. A fully matching tag, refs, checkout, and version resumes
idempotently; any mismatch fails closed. The empty `@` is required for a new
release. Do not use obsolete skip or pages-publication flags.

- `--submit-linux-build` submits `builds/release-linux-x86_64.yml`; the build creates and uploads the Linux release artifact.

The Linux build manifest lives in `builds/` (not `.builds/`), so SourceHut does **not** auto-submit it on every push. The release app triggers the central Pages publisher instead of publishing pages from this repo.

Use the manual steps below only to understand the components; recover from a
partial normal release by rerunning the exact orchestrator command.

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
nix run .#prepare-release -- --version X.Y.Z
```

This sets `[package].version`, updates the `linear-cli` entry in `Cargo.lock`, updates `CHANGELOG.md`, and rewrites the Linux SourceHut build manifest artifact names for the new version.

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
3. Checks that the tag doesn't already exist on origin
4. Creates/updates the annotated local tag for the selected revision
5. Pushes the tag to origin

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

Build the local platform tarball and checksum. The tarball contains `linear` plus the README, changelog, and license files:

```bash
nix build .#release-artifact --out-link result-release-artifact
mkdir -p dist/downloads
cp -p result-release-artifact/* dist/downloads/
```

For Linux, submit a SourceHut build after `builds/release-linux-x86_64.yml` has the release version:

```bash
srht builds submit builds/release-linux-x86_64.yml \
  --secrets \
  --note 'linear-cli vX.Y.Z linux release' \
  --tag 'linear-cli/vX.Y.Z/release'
```

Use `--secrets` for release manifests with `oauth:` grants; sr.ht only provisions those tokens when the submit explicitly enables secrets. `srht` defaults builds to unlisted visibility.

The release flow uploads artifacts to SourceHut tag artifacts and triggers the central Pages publisher; this repo should not publish Pages directly as part of the normal release path.

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
