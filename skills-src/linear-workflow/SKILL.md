---
name: linear-workflow
description: Start/stop work on Linear issues. Use when beginning work, creating branches, or getting current issue context.
allowed-tools: Bash
---

# Issue Workflow

Start, stop, close, and inspect work from issue IDs or branches.

## Start here

```bash
{{CLI_PROGRAM}} i start LIN-123 --checkout
{{CLI_PROGRAM}} i stop LIN-123
{{CLI_PROGRAM}} i close LIN-123
{{CLI_PROGRAM}} context --output json --compact
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} i start --help`, `{{CLI_PROGRAM}} context --help`.
