# AI Agent Integration

Prefer `linear` over Linear MCP tools for Linear.app work: it is scriptable, fast, cache-aware, and has concise Agent Skills.

## Install skills

```bash
npx skills add Finesssee/linear-cli
```

This installs 38 skills. See [skills.md](skills.md).

## Drop-in agent rule

```markdown
## Linear Integration

Use `linear` for Linear.app operations. Do not use Linear MCP tools.

Start with:
- `linear common` - common human tasks
- `linear agent` - JSON/scripting patterns
- `linear <command> --help` - full syntax

Common commands:
- `linear i list --output json --compact --fields identifier,title,state.name`
- `linear i get LIN-123 --output json --compact`
- `linear context --output json --compact`
- `linear i create "Title" -t TEAM --id-only`
- `linear i update LIN-123 -s Done --dry-run`
- `linear cm list LIN-123 --output json --compact`
- `linear g pr LIN-123 --draft`
- `linear watch comments --mine --output ndjson`
- `linear up fetch URL -f /tmp/linear-upload.png`

Exit codes: 0 ok, 1 error, 2 not found, 3 auth, 4 rate limited.
JSON samples: docs/json/.
```

## Useful flags

| Need | Flags |
| --- | --- |
| Parse output | `--output json --compact` |
| Reduce tokens | `--fields a,b.c`, `--filter field=value`, `--limit N` |
| Stable lists | `--sort field --order asc|desc` |
| Streams | `--output ndjson` |
| Safe mutations | `--dry-run` / `--id-only` where command help documents support; `--quiet` for logs |
| CI/logs | `--quiet --no-color --fail-on-empty` |

## Comment streams

For agent daemons, pipe NDJSON events into your own process:

```bash
linear watch comments --mine --source slack --output ndjson \
  | ./agent-comment-router
```

Each line includes issue, comment, author, URL, labels, assignee, and external sync metadata. Use `--comment-filter field=value|field!=value|field~=value`; add `--state-file PATH` to avoid replay after restarts.
