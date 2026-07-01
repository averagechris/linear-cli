---
name: linear-admin
description: Administer the {{CLI_PROGRAM}} CLI and reach the raw Linear API - auth login/status, config and defaults, context setup, diagnostics, completions, and raw GraphQL queries/mutations. Use when configuring auth or workspaces, initializing repo context, debugging the CLI, or running GraphQL the typed commands do not cover.
allowed-tools: Bash
---

# Admin & Raw API

Auth, configuration, repo context setup, diagnostics, and raw GraphQL.

## Command map

```bash
{{CLI_PROGRAM}} auth login                                 # keyring-backed auth (auth status)
{{CLI_PROGRAM}} config show                                # config (set default-team TEAM)
{{CLI_PROGRAM}} context init --team EPD                    # write repo-local .linear.toml
{{CLI_PROGRAM}} context suggest --team EPD                 # print a policy skeleton for .linear.toml
{{CLI_PROGRAM}} context cache-status --output json --compact  # inspect option caches (context refresh ...)
{{CLI_PROGRAM}} doctor                                     # diagnostics
{{CLI_PROGRAM}} completions static zsh > ~/.zfunc/_{{CLI_PROGRAM}}  # shell completions
{{CLI_PROGRAM}} api query '{ viewer { id name email } }'   # raw GraphQL (api mutate)
```

## Agent notes

- Credentials are stored in the OS keyring. For a one-off override use `--api-key KEY` or `--profile NAME`; env-var auth is not supported.
- Prefer typed commands; use raw GraphQL only when no command covers the task. Pass variables with `-v key=value` and read query text from stdin with `-`.
- Run `context init` or `context suggest` once per repo to encode team/status defaults and field policies in `.linear.toml`; every other skill reads them via `{{CLI_PROGRAM}} context`.
- Full syntax: `{{CLI_PROGRAM}} auth --help`, `{{CLI_PROGRAM}} context --help`, `{{CLI_PROGRAM}} api query --help`.
