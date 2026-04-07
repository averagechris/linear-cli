---
name: linear-config
description: Configure {{CLI_PROGRAM}} - auth (API key + OAuth), workspaces, diagnostics, setup wizard.
allowed-tools: Bash
---

# Configuration

```bash
# First-time setup wizard
{{CLI_PROGRAM}} setup

# Set API key
{{CLI_PROGRAM}} config set-key

# Show config
{{CLI_PROGRAM}} config show

# Auth commands
{{CLI_PROGRAM}} auth login                # Store API key in OS keyring
{{CLI_PROGRAM}} auth oauth                # OAuth 2.0 browser flow (PKCE)
{{CLI_PROGRAM}} auth oauth --admin        # Explicitly add admin scope
{{CLI_PROGRAM}} auth oauth --client-id ID # Custom OAuth app
{{CLI_PROGRAM}} auth status               # Check auth status (shows type, expiry)
{{CLI_PROGRAM}} auth revoke               # Revoke OAuth tokens
{{CLI_PROGRAM}} auth logout               # Remove stored credentials
{{CLI_PROGRAM}} auth logout --remove-profile # Also delete the profile entry

# Workspaces
{{CLI_PROGRAM}} config workspace-add work
{{CLI_PROGRAM}} config workspace-list
{{CLI_PROGRAM}} config workspace-switch work
{{CLI_PROGRAM}} config workspace-current

# Profiles
{{CLI_PROGRAM}} --profile work i list     # Use profile

# Diagnostics
{{CLI_PROGRAM}} doctor                    # Check config and connectivity
{{CLI_PROGRAM}} doctor --fix              # Auto-fix common issues

# Shell completions (static)
{{CLI_PROGRAM}} config completions bash > ~/.bash_completion.d/linear

# Shell completions (dynamic, context-aware)
{{CLI_PROGRAM}} completions dynamic bash >> ~/.bashrc
{{CLI_PROGRAM}} completions dynamic zsh >> ~/.zshrc
{{CLI_PROGRAM}} completions dynamic fish >> ~/.config/fish/completions/linear.fish
```

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `LINEAR_CLI_OUTPUT` | Default output format |
| `LINEAR_CLI_YES` | Auto-confirm prompts |
| `LINEAR_CLI_NO_PAGER` | Disable pager |
