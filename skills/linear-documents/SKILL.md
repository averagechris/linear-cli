---
name: linear-documents
description: Manage Linear documents. Use for creating and viewing documentation.
allowed-tools: Bash
---

# Documents

```bash
# List documents
linear d list
linear d list --output json

# Get document
linear d get DOC_ID
linear d get DOC_ID --output json

# Create document
linear d create "Design Doc" -p PROJECT_ID
linear d create "RFC" -p PROJECT_ID --id-only

# Update document
linear d update DOC_ID --title "New Title"
linear d update DOC_ID --content "New content"
```

## Flags

| Flag | Purpose |
|------|---------|
| `-p PROJECT` | Project ID |
| `--id-only` | Return ID only |
| `--output json` | JSON output |
