# Changelog

## Unreleased





## v1.6.2 - 2026-08-05

### Changed

- Maintenance release.
## v1.6.1 - 2026-07-03

### Fixed

- Repair Linear schema drift in planning surfaces.

## v1.6.0 - 2026-07-03

### Added

- Add configurable workflow-hygiene rule engine and commands.

### Changed

- Dual-license crate.

### Fixed

- Fix live-trial bugs in fetch complexity, display, and priority semantics.

### Documentation

- Add design doc for the hygiene rule engine.

## v1.5.1 - 2026-07-01

### Changed

- Consolidate 38 skills into 7 context-driven areas.

## v1.5.0 - 2026-07-01

### Added

- Policy-driven agent context: `linear context` now resolves non-secret user
  defaults (`linear config set default-team/default-status/default-labels`)
  merged with optional repo-local `.linear.toml` policy, and returns an
  issue-creation operating contract for agents (safe defaults, required label
  groups, field policies, estimation guidance, discovery commands, and agent
  instructions).
- New context subcommands: `context init`, `context suggest`,
  `context options RESOURCE`, `context refresh [RESOURCE...]`, and
  `context cache-status`, with local caching of labels, projects,
  initiatives, statuses, and teams (configurable TTLs).
- `linear i create` applies the configured default team, default status, and
  explicit safe default labels when not otherwise specified.
- Hermetic mock-API integration tests via a hidden `--api-url` flag that
  requires an explicit `--api-key`, plus an offline/online fixture checker
  wired into `jj lint`.

### Changed

- `linear context` (plain output) now prints a resolved context summary
  instead of only the branch issue ID, and no longer errors outside a git
  repo. Scripts should use `linear context --id-only`, which preserves the
  previous ID-only output and non-zero exit when no issue is detected.
- Status-name resolution for `i create`/`i update` now uses the shared
  team-UUID statuses cache; updates retry once with fresh statuses after a
  failure, while creates only invalidate the cache (never auto-retry) to
  avoid duplicate issues.

### Fixed

- Context resource refreshes paginate through all pages instead of
  truncating at the first page (teams 50, projects/initiatives 100,
  labels 250); `docs/json/context.json` regenerated to match real output.
- A configured default status no longer overrides an explicit
  `--data state/status/stateId` during issue creation; precedence is
  CLI `-s` > `--data state/status` > `--data stateId` > context default.
- `context refresh` exits non-zero when every requested resource fails,
  keeping stdout a single JSON document.


## v1.4.0 - 2026-06-30

### Added

- Add project flag to create.

### Changed

- Bump Rust dependency set.
- Add Rust dependency maintenance tools.
- Add clippy to jj lint.

## v1.3.0 - 2026-06-25

### Added

- Stream issue comment events.

### Changed

- Assert real version output.

### Documentation

- Streamline help and agent skills.

## v1.2.14 - 2026-06-15

### Fixed

- Stabilize release flow bookmark updates.

## v1.2.13 - 2026-06-15

### Fixed

- Group hosted downloads by release.
- Sort hosted downloads by semantic version.

## v1.2.12 - 2026-06-15

### Fixed

- Sort hosted downloads by semantic version.
- Stabilize SourceHut release publishing.

## v1.2.11 - 2026-06-15

### Fixed

- Use service-qualified SourceHut Pages OAuth grant.
- Publish SourceHut Linux downloads automatically.

## v1.2.10 - 2026-06-15

### Fixed

- Publish SourceHut Linux downloads automatically.

## v1.2.9 - 2026-06-15

### Changed

- Update dependencies and resolve audit findings.
- Bump flake inputs.

## v1.2.8 - 2026-06-15

### Changed

- Updated dependencies and resolved audit findings.
- Bumped flake inputs.
