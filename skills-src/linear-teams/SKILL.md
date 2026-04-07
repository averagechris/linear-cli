---
name: linear-teams
description: Manage Linear teams and users - list, create, update, delete teams. Use when managing teams or viewing user profiles.
allowed-tools: Bash
---

# Teams

```bash
# List teams
{{CLI_PROGRAM}} t list
{{CLI_PROGRAM}} t list --output json

# Get team details
{{CLI_PROGRAM}} t get ENG
{{CLI_PROGRAM}} t members ENG             # List team members

# Create team
{{CLI_PROGRAM}} t create "Platform" -k PLT
{{CLI_PROGRAM}} t create "Mobile" -k MOB --description "Mobile team" --private

# Update team
{{CLI_PROGRAM}} t update ENG --name "Engineering" --timezone "America/New_York"

# Delete team
{{CLI_PROGRAM}} t delete TEAM_ID --force
```

# Users

```bash
# List users
{{CLI_PROGRAM}} u list                    # All workspace users
{{CLI_PROGRAM}} u list --team ENG         # Team members only

# Current user
{{CLI_PROGRAM}} u me
{{CLI_PROGRAM}} me                        # Alias (whoami)
```

## Flags

| Flag | Purpose |
|------|---------|
| `-k KEY` | Team key |
| `--private` | Private team |
| `--output json` | JSON output |
| `--compact` | No formatting |
