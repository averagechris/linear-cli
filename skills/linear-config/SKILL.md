---
name: linear-config
description: Configure linear - auth (API key + OAuth), workspaces, diagnostics, setup wizard.
allowed-tools: Bash
---

# Configuration

```bash
# First-time setup wizard
linear setup

# Set API key
linear config set-key

# Show config
linear config show

# Auth commands
linear auth login                # Store API key in OS keyring
linear auth oauth                # OAuth 2.0 browser flow (PKCE)
linear auth oauth --admin        # Explicitly add admin scope
linear auth oauth --client-id ID # Custom OAuth app
linear auth status               # Check auth status (shows type, expiry)
linear auth revoke               # Revoke OAuth tokens
linear auth logout               # Remove stored credentials
linear auth logout --remove-profile # Also delete the profile entry

# Workspaces
linear config workspace-add work
linear config workspace-list
linear config workspace-switch work
linear config workspace-current

# Profiles
linear --profile work i list     # Use profile

# Diagnostics
linear doctor                    # Check config and connectivity
linear doctor --fix              # Auto-fix common issues

# Shell completions (static)
linear config completions bash > ~/.bash_completion.d/linear

# Shell completions (dynamic, context-aware)
linear completions dynamic bash >> ~/.bashrc
linear completions dynamic zsh >> ~/.zshrc
linear completions dynamic fish >> ~/.config/fish/completions/linear.fish
```

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `LINEAR_CLI_OUTPUT` | Default output format |
| `LINEAR_CLI_YES` | Auto-confirm prompts |
| `LINEAR_CLI_NO_PAGER` | Disable pager |
