# Upstream review: October 9, 2026

Reviewed the upstream `nesszer/linear-cli` history after `51af446a` through
`e100db11a3959a528cf449f2befb505cf5d1a342` (the current upstream `master`
reported by GitHub on 2026-10-09). The fork's remote metadata identifies the
upstream repository as `nesszer/linear-cli`, default branch `master`.

This was a selective diff review, focused on behavior that overlaps the fork's
dry-run, pagination, branch generation, issue creation, schema queries, and
release policies. Release tooling and unrelated command/doc changes were
screened by changed paths and excluded; they were not reviewed as implementation
ports.

## Commit dispositions

- [`ddcf35f0967d1d23a4507f7720cc671cefd6dacb`](https://github.com/nesszer/linear-cli/commit/ddcf35f0967d1d23a4507f7720cc671cefd6dacb), “feat: integrate release recovery and safety fixes”: adopted the fail-closed dry-run policy for operations without previews, the mutation-boundary guard, and the Unicode-safe, nonempty branch slug behavior. Also adopted the forward cursor progress invariant in the fork's existing paginators; the upstream implementation introduced this guard in a feature-specific paginator, while the fork applies it to both existing accumulated and streaming pagination paths. Deferred upstream `issues create --project` as a separate product feature. The review-URL feature is deferred because it adds a large independent workflow. Kept the fork's existing live-schema query adaptations instead of copying upstream GraphQL selections wholesale.
- [`8bc07ef4c41b84ec582a9ed28701675455b218c4`](https://github.com/nesszer/linear-cli/commit/8bc07ef4c41b84ec582a9ed28701675455b218c4), “fix(ci): align release contexts with CircleCI setup”: excluded CircleCI configuration, release credentials, and publication lanes. The fork retains local validation and its manual publication process.
- [`e100db11a3959a528cf449f2befb505cf5d1a342`](https://github.com/nesszer/linear-cli/commit/e100db11a3959a528cf449f2befb505cf5d1a342), “fix: harden release and pagination gates (#52)”: adopted the unsupported-dry-run and cursor-progress safeguards in fork-specific form. Excluded CircleCI release tooling, release manifests, hosted publication changes, and their tests.

The upstream self-update and GitHub Release installer paths, along with env-based
auth/profile overrides, remain excluded under the fork's keyring-only and
package-manager update policies. No upstream source files were copied verbatim;
ports were adapted to the fork's existing preview handlers, schema, and tests.

## Checks

Behavioral tests in `tests/cli_tests.rs` and `tests/mock_api_tests.rs` cover
before-auth dry-run rejection, zero-mutation global import preview, and bounded
pagination with stalled forward/backward cursors. `src/vcs.rs` has unit coverage
for multibyte truncation and empty/trailing-separator slugs.

Validation on 2026-10-09 passed `nix flake check` (host system
`x86_64-linux`), `nix run .#ci-fmt`, `nix run .#ci-clippy`, and
`nix run .#ci-test`. The final package build passed with `nix build .#linear`.
