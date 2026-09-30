---
name: release-process
description: Review commits, choose the next semver version, publish trusted annotated GitHub refs, and hand verified artifacts to a person for draft release publication.
allowed-tools: Bash, Read, Grep, Edit, Write
---

# Release process

Use this skill for a future linear-cli release. Read `docs/release.md` before acting.

## Policy

- Keep credentials in the OS keyring. Do not add environment auth or profile support.
- Never add self-update or an installer to `linear update`; it only warns from canonical GitHub tags.
- Do not add hosted PR CI, an automatic release publisher, repository release secrets, or a Pages workflow/dispatch.
- Fleet is pinned in `flake.lock` and the workflow caller. Do not float either reference.
- Never push a tag, publish a release, dispatch a workflow, or update Pages without explicit user approval.

## 1. Choose the version

Find the latest `vX.Y.Z` tag with `jj tag list`. Review `<last-tag>::@-` with `jj log`. Use the highest Conventional Commit bump: breaking change for major, `feat` for minor, and fixes or maintenance for patch. Stop if there is no releasable change.

## 2. Read-only preflight

From an empty `@` whose parent, local `main`, and `main@origin` agree:

```bash
nix run .#release -- --version X.Y.Z --check
```

The GitHub backend accepts a version, plus the existing downgrade escape hatch. It does not accept SourceHut build flags.

## 3. Publish trusted refs

Only with explicit approval:

```bash
nix run .#release -- --version X.Y.Z
```

Fleet prepares `Cargo.toml`, `Cargo.lock`, and `CHANGELOG.md`; runs fmt, clippy, tests, and `ci-skills-render`; then atomically publishes leased `main` and an annotated tag. GitHub Actions builds `aarch64-darwin` and `x86_64-linux` artifacts with read-only repository permission.

## 4. Manual publication

Follow `docs/release.md`. Verify both checksum sidecars and both tag identity files. Create a draft GitHub Release, upload only Fleet tarballs and sidecars, download and compare the published bytes, verify digests again, then remove draft status. Update Pages manually with `project=linear-cli`.

The Darwin `homebrew-artifact` is a separate output and formula contract. It is not a Fleet public release asset unless a separate reviewed decision says otherwise.
