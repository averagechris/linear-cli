---
name: linear-workflow
description: Start/stop work on Linear issues. Use when beginning work, creating branches, or getting current issue context.
allowed-tools: Bash
---

# Issue Workflow

Start, stop, close, and inspect work from issue IDs or branches.

## Start here

```bash
linear i start LIN-123 --checkout
linear i stop LIN-123
linear i close LIN-123
linear context --output json --compact
```

## Agent notes
- For parsing, add `--output json --compact`; use `--fields a,b.c` to trim payloads.
- Full syntax and less-common flags: `linear i start --help`, `linear context --help`.
