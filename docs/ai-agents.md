# AI Agent Integration

Prefer `linear` over Linear MCP tools for Linear.app work: it is scriptable, fast, cache-aware, and has concise Agent Skills.

## Install skills

```bash
npx skills add Finesssee/linear-cli
```

This installs 7 high-level skills. See [skills.md](skills.md).

## Drop-in agent rule

```markdown
## Linear Integration

Use `linear` for Linear.app operations. Do not use Linear MCP tools.

Start with:
- `linear common` - common human tasks
- `linear agent` - JSON/scripting patterns
- `linear context --output json --compact` - user/repo defaults, branch issue, and gentle setup hints
- `linear <command> --help` - full syntax

Common commands:
- `linear i list --output json --compact --fields identifier,title,state.name`
- `linear i get LIN-123 --output json --compact`
- `linear context --output json --compact`
- `linear i create "Title" --id-only` # applies configured team/project/label defaults when present
- `linear i update LIN-123 -s Done --dry-run`
- `linear cm list LIN-123 --output json --compact`
- `linear g pr LIN-123 --draft`
- `linear hygiene check --output json --compact` # workflow-hygiene findings with executable fixes
- `linear watch comments --mine --output ndjson`
- `linear up fetch URL -f /tmp/linear-upload.png`

Exit codes: 0 ok, 1 error, 2 not found, 3 auth, 4 rate limited.
JSON samples: docs/json/.
```

## Dynamic context

Use `linear context` as the bridge between static agent skills and user/repo-specific conventions.
It combines non-secret user config with optional repo-local `.linear.toml` context and returns
an issue-creation operating contract: safe defaults, required label groups, field policies,
estimation guidance, discovery commands, and agent instructions.

```bash
linear config set default-team EPD
linear context suggest --team EPD --status Spec > .linear.toml
linear context init --team EPD --status Spec --required-label-group domain --required-label-group type
linear context --output json --compact
linear context options labels --group domain --output json --compact
linear context refresh labels projects initiatives
linear context cache-status --output json --compact
```

When context is missing, `linear context` includes quiet, actionable hints such as
`missing_default_team`; ordinary successful commands do not print these setup tips.

Agents should only auto-apply safe defaults such as team/status. For fields like projects,
initiatives, estimates, and required category labels, follow the returned field policy: infer
when high-confidence, otherwise fetch options with `linear context options ...` and ask the user.

`linear context options` reads local cached metadata when available. Use `--refresh` on an options
command or `linear context refresh RESOURCE...` to update labels/projects/initiatives/statuses/teams
from Linear. Cache status is available via `linear context cache-status --output json --compact`.

## Useful flags

| Need | Flags |
| --- | --- |
| Parse output | `--output json --compact` |
| Reduce tokens | `--fields a,b.c`, `--filter field=value`, `--limit N` |
| Stable lists | `--sort field --order asc|desc` |
| Streams | `--output ndjson` |
| Safe mutations | `--dry-run` / `--id-only` where command help documents support; `--quiet` for logs |
| CI/logs | `--quiet --no-color --fail-on-empty` |

## Workflow hygiene

`linear hygiene` (alias `hy`) runs declarative rules from the user-level
`hygiene.toml` against issues, projects, and initiatives and returns findings
with executable fixes. It is designed agents-first; the full design is in
[hygiene.md](hygiene.md) and a sample finding payload in
[json/hygiene-check.json](json/hygiene-check.json).

The core loop:

```bash
linear hygiene rules --schema                  # field model + operators (once)
linear hygiene check --output json --compact   # findings with fixes
linear hygiene fix --yes --output json         # bulk-execute deterministic (command-kind) fixes
linear hygiene apply DEDUPE_KEY --option ACTION        # resolve a choice
linear hygiene apply DEDUPE_KEY --input "authored text" # supply content
```

- `check` is read-only; it persists a run artifact that `apply` consumes for
  ~30 minutes (`[hygiene] apply_ttl`). Re-run `check` when `apply` reports an
  expired artifact.
- `fix --yes` executes only `command`-kind fixes and reports skipped findings
  with `reason: needs_choice` / `needs_input`; remediate those one-by-one via
  `apply` with `--option` / `--input`. The CLI never chooses among ambiguous
  fixes or invents content.
- Scope with `--mine`, `--user X`, `-t TEAM`, `--project X`, `--initiative X`,
  or the global `--all` (org-wide); narrow with repeatable `--entity` /
  `--rule`. `--fail-if-findings` makes findings exit 1 for CI.
- `hygiene report --by rule|owner|team|entity --output json` aggregates counts;
  `hygiene snooze KEY --for 2w` suppresses a finding locally.
- No rules yet? `linear hygiene rules --init` writes a commented starter
  `hygiene.toml`.
- Test alternate rule sets without touching the real config: `--rules PATH`
  (global on all hygiene subcommands) or `LINEAR_CLI_HYGIENE_RULES` (flag
  wins); override paths must exist and parse.

## Comment streams

For agent daemons, pipe NDJSON events into your own process:

```bash
linear watch comments --mine --source slack --output ndjson \
  | ./agent-comment-router
```

Each line includes issue, comment, author, URL, labels, assignee, and external sync metadata. Use `--comment-filter field=value|field!=value|field~=value`; add `--state-file PATH` to avoid replay after restarts.
