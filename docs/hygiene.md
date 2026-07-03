# Hygiene: Design Document

Status: **implemented** (core engine in `src/hygiene/`, commands in
`src/commands/hygiene.rs`; e2e coverage in `tests/mock_api_tests.rs`)

`linear hygiene` (alias `hy`) is a standalone, configurable rule engine that
detects workflow-hygiene problems across Linear issues, projects, and
initiatives — stale work, missing fields, unhealthy planning artifacts — and
turns them into actionable, agent-consumable findings with executable fixes.

## 1. Motivation

Organizations encode SDLC conventions (required fields, staleness thresholds,
health-update cadences) but Linear does not enforce most of them. Keeping work
flowing requires periodic sweeps: "what's been In Progress too long?", "which
active initiatives have no health update?", "which projects have no lead?".

Today the CLI has the remediation primitives (`bulk`, `triage`, `--dry-run`,
`--yes`, JSON output) but no rule concept and no query-to-action pipeline.
This feature adds both, designed **agents-first**: the primary consumer is a
coding agent that runs `hygiene check --output json`, reasons about findings,
and applies the suggested fixes.

## 2. Goals and non-goals

### Goals

- G1. Detect hygiene violations on issues, projects, and initiatives using
  declarative, user-authored rules.
- G2. Work for **any** organization: rules are data (TOML), not code. No
  coupling to any specific org's conventions repo or external hygiene tool.
- G3. Agent-first ergonomics: stable finding schema, executable fixes with a
  single-action `apply` step, JSON/NDJSON output, deterministic exit codes,
  config validation with tight feedback.
- G4. Safe bulk remediation: query-by-rule → concurrent mutations, always
  honoring `--dry-run` and `--yes`, never inventing values or choosing among
  ambiguous fixes unattended.

### Non-goals (v1)

- N1. No scheduling, daemons, webhooks, or push notifications (users can cron
  `hygiene check` themselves; `watch` exists for polling).
- N2. No business-day calendars. Durations are calendar time only; docs note
  that business-day policies should be approximated (e.g., 5 business days ≈
  `7d`).
- N3. No importing/generating rules from external convention repos. Users (or
  their agents) author `hygiene.toml` themselves.
- N4. No cross-run trend tracking or history. The last-run artifact (§3.6)
  exists only to power `apply`; dedupe keys are stable so external tools can
  diff runs.
- N5. No org-wide reporting server; `report` is a local aggregation of a
  single run.
- N6. No new authentication or API surface: uses the existing keyring-backed
  auth and GraphQL client.

## 3. Requirements

Requirement keywords follow RFC 2119 (MUST/SHOULD/MAY).

### 3.1 Configuration

- R1. Rules MUST be defined in a user-level file
  `~/.config/linear-cli/hygiene.toml` (respecting the platform config dir and
  profile scoping used by `config.toml`).
- R2. A repo-level `.linear.toml` MAY contain a `[hygiene]` table that
  overrides **scope only** (default team/project filters), never rule
  definitions. Merge semantics follow the existing context merge (project
  overrides user).
- R3. The config MUST support two rule kinds:
  - `when`-rules: per-entity declarative predicates (§4.2).
  - `builtin`-rules: opt-in named rules with typed `params` for logic that
    cannot be expressed per-entity (§4.3).
- R4. Every rule MUST have a unique `id` (builtins default `id` to the builtin
  name; duplicates are a config error).
- R5. Rules MUST support `severity` (`high` | `medium` | `low`, default
  `medium`) and `enabled` (default `true`).
- R6. A global `[hygiene.scope]` table MUST support:
  - `exempt_labels` (list; entities carrying any of these labels are skipped
    by all rules; default `["ignore-audit"]`),
  - `teams` (list of team keys to include; default: all accessible),
  - `include_archived` (default `false`).
- R7. Unknown fields, unknown operators, unknown builtins, invalid durations,
  and type errors MUST be rejected at config load with an error naming the
  rule id and offending key. Config errors MUST NOT be silently ignored.
- R8. With no `hygiene.toml` (or zero enabled rules), `hygiene check` MUST
  succeed with zero findings (exit 0) and MUST emit a hint that no rules are
  configured, pointing at `hygiene rules --init`. The hint goes to stderr in
  table mode and to a `hint` field in JSON mode; `--quiet` suppresses it.
- R9. Durations MUST use the existing duration syntax (`90m`, `6h`, `7d`,
  `2w`) parsed by shared helpers; calendar time only (N2).

### 3.2 Rule evaluation

- R10. `when`-rule predicates MUST be evaluated as a conjunction (AND) of all
  conditions in the rule. OR across conditions is expressed as multiple rules.
- R11. The engine MUST evaluate rules against a documented flattened field
  model per entity type (§4.1). Referencing a field not in the model for that
  entity type is a config error (R7).
- R12. Staleness MUST be computed from the entity's `updatedAt` (issues,
  projects, initiatives) or health-update timestamps where specified, compared
  against wall-clock now.
- R13. Server-side filtering MUST be used where the Linear API supports it
  (issue `updatedAt`, team, state); remaining conditions are evaluated
  client-side. Results MUST be identical regardless of where a condition is
  evaluated.
- R14. Entities matching `[hygiene.scope].exempt_labels` MUST produce no
  findings and MUST NOT be fetched into fix pipelines.
- R15. `check`, `rules`, and `report` MUST be read-only. Only `fix` and
  `apply` mutate, under the confirmation semantics of R24–R25.

### 3.3 Findings

- R16. Each finding MUST include: `dedupeKey`, `rule`, `severity`, `entity`
  (`type`, `id`, `identifier`/`slug`, `url`, `title`), `owner` (assignee /
  lead / initiative owner; nullable), `summary` (one human-readable line),
  `evidence` (the field values that triggered the rule, including thresholds
  and the entity's `updatedAt` snapshot), and `fix` (§3.5; nullable when no
  automated fix exists).
- R17. `dedupeKey` MUST be `"<rule-id>:<entity-identifier>"` (builtins that
  group by a non-entity subject, e.g. `wip-limit` per assignee, use
  `"<rule-id>:<subject>"`) and stable across runs for the same violation.
- R18. Findings MUST be sorted deterministically (severity desc, then rule id,
  then entity identifier) in all output formats. Findings suppressed by an
  unexpired snooze (R26) MUST be excluded from output and from `fix`/`apply`.

### 3.4 Commands

- R19. **Scope** is the set of entities fetched and evaluated, composed from
  two axes:
  - *owner axis*: `--mine` (authenticated user) / `--user X` — matches issue
    assignee, project lead, initiative owner;
  - *container axis*: `--team KEY` / `--project X` / `--initiative X` /
    `--all` (org-wide).
  Axes compose (e.g., `--mine --team ENG`). Additional narrowing: `--entity
  issue|project|initiative` (repeatable) and `--rule ID` (repeatable).
- R20. `hygiene check` MUST run enabled rules over the resolved scope and
  print findings. Default scope resolution for the container axis:
  `.linear.toml` `[hygiene]` scope if present, else the context/default team;
  if neither exists and no flags are given, the scope defaults to `--mine`.
  `--fail-if-findings` makes findings exit 1 (default exit 0 so strictness is
  opt-in).
- R21. `hygiene check` MUST honor the global output flags (`--output
  table|json|ndjson`, `--fields`, `--filter`, `--sort`, `--compact`,
  `--quiet`, `--limit`).
- R22. `hygiene rules` MUST list effective rules with their source file,
  enabled state, and severity, and MUST validate the config (exit 1 with all
  errors listed on invalid config). `hygiene rules --schema` MUST print the
  field model and operator vocabulary as JSON for agent consumption.
  `hygiene rules --init` MUST write a commented starter `hygiene.toml`
  (refusing to overwrite without `--force`) demonstrating each operator and
  one builtin.
- R23. `hygiene report` MUST aggregate a check run into counts grouped by
  `--by rule|owner|team|entity` (default `rule`), with the same scoping flags
  as `check`.
- R24. `hygiene fix` MUST select findings via the same scoping flags plus
  `--rule` and remediate them in bulk:
  - `--dry-run` MUST print intended mutations without executing;
  - without `--yes`, it MUST prompt per finding (table mode) or refuse with an
    instructive error (JSON output or non-TTY);
  - with `--yes`, it MUST execute only `command`-kind (deterministic) fixes,
    concurrently, and print a per-finding success/failure summary (reusing the
    `bulk` concurrency pattern);
  - `options`-kind findings MUST be skipped with reason `needs_choice` and
    `needs_input`-kind with reason `needs_input` — the CLI MUST NOT choose
    among ambiguous fixes or invent content. Skipped findings MUST appear in
    the summary so agents know what remains (remediable one-by-one via
    `apply`, R25).
- R25. `hygiene apply DEDUPE_KEY...` MUST execute the stored fix for findings
  from the last check artifact (§3.6):
  - `--option ACTION` selects among an `options`-kind fix (required for that
    kind; error listing valid actions if omitted or invalid);
  - `--input "text"` supplies content for a `needs_input`-kind fix (required
    for that kind);
  - `command`-kind fixes need no extra flags;
  - `--dry-run` prints the resolved mutation(s) without executing.
  `apply` does NOT re-verify entity freshness; the expected workflow is
  check → apply within minutes, and the artifact TTL (R33) guards against
  egregiously old state.
- R26. `hygiene snooze DEDUPE_KEY --for DURATION` MUST suppress that finding
  locally (profile-scoped state file) until expiry; `hygiene snooze --list`
  and `--clear` manage entries. Snoozes are per-machine, not synced.
- R27. Exit codes MUST follow the CLI conventions: 0 success, 1 general error
  (or findings with `--fail-if-findings`), 2 not found (including an unknown
  `DEDUPE_KEY` in `apply`/`snooze`), 3 auth, 4 rate limit. JSON-mode errors
  use the standard error envelope.

### 3.5 Fixes

- R28. A fix MUST be one of:
  - `command` — a deterministic remediation requiring no further input (e.g.,
    set a status; apply a label when the label group has exactly one valid
    candidate);
  - `options` — a list of candidate actions, each named (`action`) and
    executable, when remediation requires a choice;
  - `needs_input` — remediation requires authored content (e.g., a progress
    comment); a template with a placeholder MAY be included.
- R29. Every fix / fix option MUST carry an equivalent `linear …` command
  string valid for the current CLI version, so consumers can execute it
  out-of-band instead of via `apply`. Command strings MUST NOT embed secrets,
  API keys, or profile flags. A fix MAY translate to multiple API calls when
  applied; `apply` presents it as one action.
- R30. **Fix provenance.** Fixes come from three sources, in this order:
  - *Config-declared*: a `when`-rule MAY carry a `[fix]` block declaring the
    org's remedy — either field assignments (`set = { status = "Todo" }`),
    producing a `command`-kind fix, or a list of named `options` (each an
    action with `set = {…}` and/or `comment = true`, the latter marking the
    option `needsInput`), producing an `options`-kind fix. `set` fields MUST
    be valid, settable fields for the rule's entity type (validated per R7).
  - *Auto-derived*: without a `[fix]` block, rules whose predicate is a
    missing-field / missing-group condition MUST get an `options`-kind fix
    enumerating the valid candidates (statuses, priorities, group labels,
    …) from cached metadata. When exactly one candidate exists, the fix
    degrades to `command`-kind. The CLI enumerates; it never selects.
  - *None*: all other rules without a `[fix]` block produce findings with
    `fix: null` (visibility only), excluded from `fix`/`apply`.
  Builtins define their own fixes (or none) as part of their contract (§4.3).
- R31. Unattended execution (`fix --yes`) MUST be limited to `command`-kind
  fixes — i.e., remedies fully determined in advance by config, by a single
  valid candidate, or by a builtin's contract. Every mutation path requires
  an explicit invocation (`fix` or `apply`); `check` never mutates.

### 3.6 Run state and caching

- R32. `hygiene check` MUST persist its findings (including fixes, evidence,
  scope, and a run timestamp) as a **last-run artifact** in the profile- and
  auth-scoped state directory (atomic write, following the existing
  `templates.json` / watch-state precedent). Each `check` replaces the
  artifact.
- R33. `apply` MUST refuse to use an artifact older than a TTL (default
  `30m`, configurable via `[hygiene] apply_ttl`), directing the user to
  re-run `check`. Within the TTL the artifact is trusted as-is; no per-entity
  re-verification is performed (R25).
- R34. Successful `apply` and `fix` executions MUST mark the corresponding
  findings resolved in the artifact so repeated `apply` calls are idempotent
  (re-applying a resolved finding is a no-op reported as `already_resolved`).
- R35. Metadata needed for evaluation and fix construction (labels, label
  groups, statuses, teams) MUST reuse the existing context option caches and
  their TTLs, honoring `--no-cache`, `--cache-ttl`, and `context refresh`.
- R36. Entity data evaluated by `check` MUST always be fetched fresh — never
  served from a cache. Findings are claims about recency; evaluating stale
  snapshots produces false findings. The last-run artifact and the metadata
  caches are the only persisted state besides snoozes.

### 3.7 Quality

- R37. The rule engine (config parsing/validation, predicate evaluation,
  builtins, dedupe keys, sorting, fix derivation) MUST be pure and
  unit-tested without network or keyring access.
- R38. End-to-end tests MUST live in `tests/mock_api_tests.rs` using
  `--api-url`/`--api-key` and fixture rules per repo conventions, covering
  `check`, `rules`, `fix --dry-run/--yes` (including `needs_choice`/
  `needs_input` skips), and `apply` (including the expired-TTL refusal). No
  test may touch the OS keyring or network.
- R39. `hygiene check` on a scoped team (≤ ~500 open issues, ≤ ~50 projects,
  ≤ ~25 initiatives) SHOULD complete in a small number of paginated GraphQL
  requests; `--all` MAY be slow but MUST paginate correctly and respect rate
  limits via the existing retry machinery.
- R40. `linear agent` output and `docs/ai-agents.md` MUST be updated to
  describe the hygiene surface (including the check → fix → apply loop); JSON
  samples added under `docs/json/`.

## 4. Design

### 4.1 Entity field model

Predicates reference flattened fields. v1 model (extensible):

| Field | issue | project | initiative | Type |
|---|---|---|---|---|
| `status` (workflow state name) | ✓ | | | string |
| `state` (lifecycle: planned/started/…) | | ✓ | ✓ | string |
| `priority` (0–4; 0 = none) | ✓ | | | number |
| `estimate` | ✓ | | | number\|null |
| `assignee` | ✓ | | | string\|null |
| `lead` | | ✓ | | string\|null |
| `owner` | | | ✓ | string\|null |
| `title` / `name` | ✓ | ✓ | ✓ | string |
| `description` | ✓ | ✓ | ✓ | string |
| `labels` | ✓ | ✓ | ✓ | string[] |
| `team` | ✓ | | | string |
| `project` | ✓ | | | string\|null |
| `initiative` | | ✓ | | string\|null |
| `cycle` | ✓ | | | string\|null |
| `createdAt` / `updatedAt` | ✓ | ✓ | ✓ | datetime |
| `startDate` / `targetDate` | | ✓ | ✓ | date\|null |
| `dueDate` | ✓ | | | date\|null |
| `health` | | ✓ | ✓ | string\|null |
| `healthUpdatedAt` (latest update timestamp) | | ✓ | ✓ | datetime\|null |
| `linkedProjects` (count) | | | ✓ | number |

`hygiene rules --schema` emits this table as JSON (R22).

### 4.2 `when` operators

| Operator | Applies to | Meaning |
|---|---|---|
| `in` / `not_in` | string | value ∈ / ∉ list (also matches any element for `labels`) |
| `missing` | any nullable | `true` → null/empty; `false` → present |
| `older_than` / `newer_than` | datetime/date | age vs now exceeds / is within duration |
| `past` | date | `true` → date is before today |
| `shorter_than` / `longer_than` | string | character length bound |
| `matches` / `not_matches` | string | regex (Rust `regex` syntax) |
| `lt` / `lte` / `gt` / `gte` / `eq` | number | numeric comparison |
| `missing_group` | `labels` only | no label from the named context label group (resolved via existing label-group config + option cache) |

Example (per-status staleness is N separate rules by design; the first rule
shows a config-declared fix per R30):

```toml
[[hygiene.rules]]
id = "urgent-in-backlog"
entity = "issue"
severity = "high"
[hygiene.rules.when]
priority = { eq = 1 }
status = { in = ["Backlog"] }
[hygiene.rules.fix]
set = { status = "Todo" }

[[hygiene.rules]]
id = "stale-in-review"
entity = "issue"
severity = "high"
[hygiene.rules.when]
status = { in = ["In Review"] }
updatedAt = { older_than = "2d" }

[[hygiene.rules]]
id = "issue-missing-domain"
entity = "issue"
[hygiene.rules.when]
status = { not_in = ["Triage", "Backlog", "Done", "Canceled", "Duplicate"] }
labels = { missing_group = "domain" }

[[hygiene.rules]]
id = "initiative-stale-health"
entity = "initiative"
severity = "high"
[hygiene.rules.when]
state = { in = ["started"] }
healthUpdatedAt = { older_than = "14d" }
```

### 4.3 Builtins (v1 set)

| Builtin | Entity | Params | Detects |
|---|---|---|---|
| `wip-limit` | issue | `max_in_progress_per_assignee` (default 3), `statuses` (default `["In Progress"]`) | one finding per assignee over the limit |
| `initiative-completed-but-active` | initiative | — | all linked projects completed but initiative still active |
| `project-single-issue` | project | `min_issues` (default 2), `min_age` (default `14d`) | anti-pattern: long-lived project wrapping one issue |
| `project-no-target-date-long-lived` | project | `min_age` (default `60d`) | ongoing-maintenance anti-pattern |

```toml
[[hygiene.rules]]
builtin = "wip-limit"
severity = "medium"
[hygiene.rules.params]
max_in_progress_per_assignee = 3
```

Builtins and `when`-rules produce identical finding shapes. New builtins are
additive CLI releases.

### 4.4 Finding shape (JSON)

```json
{
  "dedupeKey": "stale-in-review:ENG-123",
  "rule": "stale-in-review",
  "severity": "high",
  "entity": {
    "type": "issue",
    "id": "uuid",
    "identifier": "ENG-123",
    "title": "Fix login flow",
    "url": "https://linear.app/org/issue/ENG-123"
  },
  "owner": { "id": "uuid", "name": "chris", "displayName": "Chris" },
  "summary": "ENG-123 in 'In Review' for 6d (threshold 2d)",
  "evidence": {
    "status": "In Review",
    "updatedAt": "2026-06-26T09:00:00Z",
    "ageDays": 6,
    "threshold": "2d"
  },
  "fix": {
    "kind": "options",
    "options": [
      { "action": "nudge_review", "command": "linear cm create ENG-123 -b \"<review nudge>\"", "needsInput": true },
      { "action": "move_back", "command": "linear i update ENG-123 -s \"In Progress\"" }
    ]
  }
}
```

The agent loop: `hygiene check --output json` → reason →
`hygiene apply stale-in-review:ENG-123 --option move_back`.

Table mode shows severity, dedupe key, owner, and summary; NDJSON emits one
finding per line.

### 4.5 Fix behavior scenarios

Reference scenarios pinning down R24/R25/R30/R31 behavior:

| Scenario | Fix source | Fix kind | `fix --yes` | `apply` needs |
|---|---|---|---|---|
| Urgent issue in Backlog; rule declares `fix.set = { status = "Todo" }` | config-declared | `command` | executes | nothing |
| Missing priority; valid values 1–4 | auto-derived | `options` | skip → `needs_choice` | `--option p2` |
| Missing domain label; group has 6 labels | auto-derived | `options` | skip → `needs_choice` | `--option payments` |
| Missing domain label; group has exactly 1 label | auto-derived | `command` | executes | nothing |
| Stale in review; remedies: nudge comment or move back | config-declared options | `options` (one option `needsInput`) | skip → `needs_choice` | `--option move_back` (or `--option nudge_review --input "…"`) |
| Stale initiative health; only remedy is an authored update | config-declared (single option, `comment = true`) | `needs_input` | skip → `needs_input` | `--input "…"` |
| WIP limit exceeded (which issue to pause is a human call) | builtin: none | `null` | not applicable | not applicable |
| Initiative complete but still active | builtin contract | `command` | executes | nothing |

The invariant: the CLI never makes a judgment call on its own. Unattended
execution covers only remedies fully determined in advance; choices and
content always arrive via explicit per-finding `apply` flags.

### 4.6 Module layout

- `src/hygiene/mod.rs` — config types (serde, snake_case TOML), load/merge,
  validation (R7).
- `src/hygiene/model.rs` — flattened entity model + conversion from existing
  GraphQL types.
- `src/hygiene/engine.rs` — predicate evaluation, builtins, finding
  construction, sorting. Pure; unit-tested.
- `src/hygiene/state.rs` — last-run artifact (R32–R34) and snooze state
  (R26); profile/auth-scoped JSON with atomic writes.
- `src/commands/hygiene.rs` — clap subcommands, fetching (reusing issue/
  project/initiative queries; adding an `updatedAt` server filter for issues),
  fix/apply pipelines reusing the `bulk` concurrency pattern, output.
- `src/main.rs` — `Commands::Hygiene` variant, dispatch, `agent` help text.

## 5. Open questions

- OQ1. Should `--mine` for projects/initiatives match lead/owner only, or
  also membership? (Current answer: lead/owner only.)
- OQ2. Should `report` support NDJSON grouping output or JSON only? (Current
  answer: JSON object keyed by group.)
- OQ3. Does the initiative API expose enough for `healthUpdatedAt` cheaply,
  or do we need the updates connection per initiative? (Resolved during
  implementation: `Initiative.health` and `Initiative.healthUpdatedAt` exist
  as scalar fields, so the check query fetches them directly with no N+1.)

## 6. Implementation notes

- `--rules PATH` (global across all hygiene subcommands) or the
  `LINEAR_CLI_HYGIENE_RULES` env var load rules from an alternate file
  (precedence: flag > env var > default user-level path). An override path
  that is missing or fails to parse is an error naming the path — unlike the
  default path, where a missing file means "no rules" (R8). The repo
  `.linear.toml` `[hygiene]` scope override still applies regardless of the
  rules source, and `rules --init --rules PATH` writes the starter to PATH.
- The org-wide scope flag reuses the global `--all` (which also means "fetch
  all pages"); hygiene always paginates fully, so the two meanings coincide.
- Server-side filtering (R13) pushes the *scope* axes (team, assignee/lead/
  owner, project, initiative) into GraphQL filters; per-rule predicate
  conditions (`updatedAt`, `state`, …) evaluate client-side because one fetch
  serves many rules. Results are identical either way.
- `hygiene rules --schema` reuses the global `--schema` flag.
- Auto-derived fix candidates cover `priority` (1–4), `estimate` (from the
  context estimation policy), and label groups. Workflow-status candidates are
  intentionally omitted: `status` is non-nullable in the field model, so
  `missing`-based fixes (the only consumer of candidates) can never reference
  it, and a multi-team scope would otherwise need per-team candidate sets.
- Applying a label fix merges with the issue's existing labels via the API
  (`linear i update -l` alone would replace the label set).
- `report --by team` groups issues by their identifier prefix (`ENG-123` →
  `ENG`); projects and initiatives group under `-` since findings do not
  carry a team reference.
