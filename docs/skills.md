# Agent Skills

`linear-cli` publishes concise Agent Skills for assistants that support the [Agent Skills](https://agentskills.io) format. Each skill is intentionally small: when to use it, a few high-value commands, and agent-specific flags that reduce noisy output.

The primary command in rendered skills is `linear`.

## Install

```bash
npx skills add Finesssee/linear-cli              # all skills
npx skills add Finesssee/linear-cli -g           # global install
npx skills add Finesssee/linear-cli --skill linear-list
```

## Available skills (38)

| Area | Skills |
| --- | --- |
| Issues | `linear-list`, `linear-create`, `linear-update`, `linear-workflow`, `linear-comments`, `linear-done` |
| Git/PR | `linear-git`, `linear-pr` |
| Planning | `linear-projects`, `linear-project-updates`, `linear-milestones`, `linear-roadmaps`, `linear-initiatives`, `linear-cycles`, `linear-sprint` |
| Organization | `linear-teams`, `linear-labels`, `linear-statuses`, `linear-relations`, `linear-templates`, `linear-views` |
| Operations | `linear-bulk`, `linear-import`, `linear-export`, `linear-triage`, `linear-favorites`, `linear-attachments` |
| Tracking | `linear-metrics`, `linear-history`, `linear-time`, `linear-watch`, `linear-webhooks` |
| Advanced | `linear-api`, `linear-search`, `linear-notifications`, `linear-documents`, `linear-uploads`, `linear-config` |

## Agent conventions

Skills avoid duplicating the full CLI help. Agents should use:

```bash
linear <command> --help
linear agent
linear i list --output json --compact --fields identifier,title,state.name
linear watch comments --mine --output ndjson
```

Useful flags: `--output json|ndjson`, `--compact`, `--fields`, `--filter`, `--limit`, `--sort`, `--order`, `--quiet`, `--fail-on-empty`; use `--dry-run` and `--id-only` only where command help documents support.

Exit codes: `0` success, `1` general error, `2` not found, `3` auth, `4` rate limited.

## Manage installed skills

```bash
npx skills list
npx skills check
npx skills update
npx skills remove --skill linear-list
npx skills remove Finesssee/linear-cli
```
