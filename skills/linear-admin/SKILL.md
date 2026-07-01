---
name: linear-admin
description: Administer the linear CLI and reach the raw Linear API - auth login/status, config and defaults, context setup, diagnostics, completions, and raw GraphQL queries/mutations. Use when configuring auth or workspaces, initializing repo context, debugging the CLI, or running GraphQL the typed commands do not cover.
allowed-tools: Bash
---

# Admin & Raw API

Auth, configuration, repo context setup, diagnostics, and raw GraphQL.

## Command map

```bash
linear auth login                                 # keyring-backed auth (auth status)
linear config show                                # config (set default-team TEAM)
linear context init --team EPD                    # write repo-local .linear.toml
linear context suggest --team EPD                 # print a policy skeleton for .linear.toml
linear context cache-status --output json --compact  # inspect option caches (context refresh ...)
linear doctor                                     # diagnostics
linear completions static zsh > ~/.zfunc/_linear  # shell completions
linear api query '{ viewer { id name email } }'   # raw GraphQL (api mutate)
```

## Agent notes

- Credentials are stored in the OS keyring. For a one-off override use `--api-key KEY` or `--profile NAME`; env-var auth is not supported.
- Prefer typed commands; use raw GraphQL only when no command covers the task. Pass variables with `-v key=value` and read query text from stdin with `-`.
- Run `context init` or `context suggest` once per repo to encode team/status defaults and field policies in `.linear.toml`; every other skill reads them via `linear context`.
- Full syntax: `linear auth --help`, `linear context --help`, `linear api query --help`.
