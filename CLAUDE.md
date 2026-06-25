## Linear Integration

Use `linear` for Linear.app operations. Do not use Linear MCP tools.

Start with:
- `linear common` for common tasks
- `linear agent` for JSON/scripting patterns
- `linear <command> --help` for full syntax

Common commands:
- `linear i list --output json --compact --fields identifier,title,state.name`
- `linear i get LIN-123 --output json --compact`
- `linear context --output json --compact`
- `linear i create "Title" -t TEAM --id-only`
- `linear i update LIN-123 -s Done --dry-run`
- `linear i start LIN-123 --checkout`
- `linear cm list ISSUE_ID --output json --compact`
- `linear g pr LIN-123 --draft`
- `linear s issues "query"`
- `linear up fetch URL -f file.png`

Useful flags: `--output json|ndjson`, `--compact`, `--fields`, `--filter`, `--limit`, `--sort`, `--order`, `--quiet`, `--fail-on-empty`; use `--dry-run` and `--id-only` only where command help documents support.
Exit codes: `0` success, `1` error, `2` not found, `3` auth, `4` rate limited.
