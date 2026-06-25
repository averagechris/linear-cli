---
name: linear-documents
description: Manage Linear documents. Use for creating and viewing documentation.
allowed-tools: Bash
---

# Documents

List, create, update, and delete Linear documents.

## Start here

```bash
linear d list
linear d get DOC_ID --output json --compact
linear d create "ADR-001" -p PROJECT_ID -c "Content"
linear d update DOC_ID -c "Updated" --dry-run
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- For mutations, use `--dry-run` and `--id-only` only where command help documents support.
- Full syntax and less-common flags: `linear d list --help`, `linear d create --help`.
