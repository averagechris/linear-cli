---
name: linear-teams
description: Manage Linear teams and users - list, create, update, delete teams. Use when managing teams or viewing user profiles.
allowed-tools: Bash
---

# Teams

```bash
# List teams
linear t list
linear t list --output json

# Get team details
linear t get ENG
linear t members ENG             # List team members

# Create team
linear t create "Platform" -k PLT
linear t create "Mobile" -k MOB --description "Mobile team" --private

# Update team
linear t update ENG --name "Engineering" --timezone "America/New_York"

# Delete team
linear t delete TEAM_ID --force
```

# Users

```bash
# List users
linear u list                    # All workspace users
linear u list --team ENG         # Team members only

# Current user
linear u me
linear me                        # Alias (whoami)
```

## Flags

| Flag | Purpose |
|------|---------|
| `-k KEY` | Team key |
| `--private` | Private team |
| `--output json` | JSON output |
| `--compact` | No formatting |
