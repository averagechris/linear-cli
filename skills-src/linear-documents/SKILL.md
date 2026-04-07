---
name: linear-documents
description: Manage Linear documents. Use for creating and viewing documentation.
allowed-tools: Bash
---

# Documents

```bash
# List documents
{{CLI_PROGRAM}} d list
{{CLI_PROGRAM}} d list --output json

# Get document
{{CLI_PROGRAM}} d get DOC_ID
{{CLI_PROGRAM}} d get DOC_ID --output json

# Create document
{{CLI_PROGRAM}} d create "Design Doc" -p PROJECT_ID
{{CLI_PROGRAM}} d create "RFC" -p PROJECT_ID --id-only

# Update document
{{CLI_PROGRAM}} d update DOC_ID --title "New Title"
{{CLI_PROGRAM}} d update DOC_ID --content "New content"
```

## Flags

| Flag | Purpose |
|------|---------|
| `-p PROJECT` | Project ID |
| `--id-only` | Return ID only |
| `--output json` | JSON output |
