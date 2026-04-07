---
name: linear-templates
description: Manage issue templates - local templates and Linear API templates. Use when creating or using templates.
allowed-tools: Bash
---

# Local Templates

```bash
# List local templates
{{CLI_PROGRAM}} tpl list

# Show template
{{CLI_PROGRAM}} tpl show bug

# Create local template
{{CLI_PROGRAM}} tpl create bug

# Delete local template
{{CLI_PROGRAM}} tpl delete bug
```

# API Templates (Linear server-side)

```bash
# List remote templates
{{CLI_PROGRAM}} tpl remote-list
{{CLI_PROGRAM}} tpl remote-list --output json

# Get remote template
{{CLI_PROGRAM}} tpl remote-get TEMPLATE_ID

# Create remote template
{{CLI_PROGRAM}} tpl remote-create "Bug Report" -t ENG

# Update remote template
{{CLI_PROGRAM}} tpl remote-update TEMPLATE_ID --name "Updated"

# Delete remote template
{{CLI_PROGRAM}} tpl remote-delete TEMPLATE_ID --force
```

## Flags

| Flag | Purpose |
|------|---------|
| `-t TEAM` | Team for remote templates |
| `--output json` | JSON output |
