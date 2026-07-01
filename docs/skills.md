# Agent Skills

`linear-cli` publishes Agent Skills for assistants that support the [Agent Skills](https://agentskills.io) format. The CLI itself is the source of truth for command syntax; skills should stay high-level and delegate user/repo-specific conventions to `linear context`.

The primary command in rendered skills is `linear`.

## Install

```bash
npx skills add Finesssee/linear-cli              # all skills
npx skills add Finesssee/linear-cli -g           # global install
npx skills add Finesssee/linear-cli --skill linear-issues
```

## Available skills (7)

Skills are intentionally high-level: each one leads with `linear context` for user/repo conventions and delegates exact syntax to `--help`.

| Skill | Covers |
| --- | --- |
| `linear-issues` | List/get/search, create/update, comments, relations, triage, start/stop/done workflow |
| `linear-git` | Branch/bookmark creation, issue checkout, branch context, linked GitHub PRs |
| `linear-planning` | Projects, project updates, milestones, initiatives, roadmaps, cycles, sprint analytics |
| `linear-organization` | Teams, users, labels, workflow statuses, templates, saved views, favorites |
| `linear-data` | Bulk updates, CSV/JSON import/export, documents, attachments, upload downloads |
| `linear-tracking` | Metrics, issue history, time tracking, watch streams, webhooks, notifications |
| `linear-admin` | Auth, config, context setup (`.linear.toml`), diagnostics, completions, raw GraphQL |

## Agent conventions

Skills avoid duplicating the full CLI help. Agents should use:

```bash
linear context --output json --compact
linear <command> --help
linear agent
linear i list --output json --compact --fields identifier,title,state.name
linear watch comments --mine --output ndjson
```

Before creating or updating issues, agents should inspect `linear context --output json --compact`.
Apply safe defaults such as team/status unless the user overrides them. For required label groups,
projects, initiatives, and estimates, follow the returned field policy: infer when high-confidence,
or run `linear context options ...` and ask the user when ambiguous. If context is missing, the JSON
`hints` array provides non-obtrusive setup suggestions such as `linear config set default-team TEAM`
or `linear context suggest --team TEAM > .linear.toml`.

Use `linear context refresh labels projects initiatives` or `linear context options labels --group domain --refresh`
when cached metadata is missing/stale. Check cache state with `linear context cache-status --output json --compact`.

Useful flags: `--output json|ndjson`, `--compact`, `--fields`, `--filter`, `--limit`, `--sort`, `--order`, `--quiet`, `--fail-on-empty`; use `--dry-run` and `--id-only` only where command help documents support.

Exit codes: `0` success, `1` general error, `2` not found, `3` auth, `4` rate limited.

## Manage installed skills

```bash
npx skills list
npx skills check
npx skills update
npx skills remove --skill linear-issues
npx skills remove Finesssee/linear-cli
```
