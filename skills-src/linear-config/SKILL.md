---
name: linear-config
description: Configure {{CLI_PROGRAM}} - auth (API key + OAuth), workspaces, diagnostics, setup wizard.
allowed-tools: Bash
---

# Configuration

Manage auth, profiles, workspaces, diagnostics, and completions.

## Start here

```bash
{{CLI_PROGRAM}} auth login
{{CLI_PROGRAM}} auth status
{{CLI_PROGRAM}} config show
{{CLI_PROGRAM}} doctor
{{CLI_PROGRAM}} completions static zsh > ~/.zfunc/_linear
```

## Agent notes
- This fork stores credentials in the OS keyring only.
- Use `--api-key KEY` or `--profile NAME` for one invocation; do not rely on env auth overrides.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} auth --help`, `{{CLI_PROGRAM}} config --help`.
