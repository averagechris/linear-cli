---
name: linear-templates
description: Manage issue templates - local templates and Linear API templates. Use when creating or using templates.
allowed-tools: Bash
---

# Local Templates

```bash
# List local templates
linear tpl list

# Show template
linear tpl show bug

# Create local template
linear tpl create bug

# Delete local template
linear tpl delete bug
```

# API Templates (Linear server-side)

```bash
# List remote templates
linear tpl remote-list
linear tpl remote-list --output json

# Get remote template
linear tpl remote-get TEMPLATE_ID

# Create remote template
linear tpl remote-create "Bug Report" -t ENG

# Update remote template
linear tpl remote-update TEMPLATE_ID --name "Updated"

# Delete remote template
linear tpl remote-delete TEMPLATE_ID --force
```

## Flags

| Flag | Purpose |
|------|---------|
| `-t TEAM` | Team for remote templates |
| `--output json` | JSON output |
