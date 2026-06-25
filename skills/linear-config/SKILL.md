---
name: linear-config
description: Configure linear - auth (API key + OAuth), workspaces, diagnostics, setup wizard.
allowed-tools: Bash
---

# Configuration

Manage auth, profiles, workspaces, diagnostics, and completions.

## Start here

```bash
linear auth login
linear auth status
linear config show
linear doctor
linear completions static zsh > ~/.zfunc/_linear
```

## Agent notes
- This fork stores credentials in the OS keyring only.
- Use `--api-key KEY` or `--profile NAME` for one invocation; do not rely on env auth overrides.
- Full syntax and less-common flags: `linear auth --help`, `linear config --help`.
