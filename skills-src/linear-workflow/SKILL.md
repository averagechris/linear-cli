---
name: linear-workflow
description: Start/stop work on Linear issues. Use when beginning work, creating branches, or getting current issue context.
allowed-tools: Bash
---

# Workflow Commands

## Start Work

```bash
# Start working (assigns to you, sets In Progress)
{{CLI_PROGRAM}} i start LIN-123

# Start + create git branch
{{CLI_PROGRAM}} i start LIN-123 --checkout
```

## Stop Work

```bash
# Stop working (unassigns, resets status)
{{CLI_PROGRAM}} i stop LIN-123
```

## Get Current Issue

```bash
# Get issue from current git branch
{{CLI_PROGRAM}} context
{{CLI_PROGRAM}} context --output json
```

## Full Workflow

```bash
# 1. Start
{{CLI_PROGRAM}} i start LIN-123 --checkout

# 2. Code...

# 3. Create PR
{{CLI_PROGRAM}} g pr LIN-123

# 4. Done
{{CLI_PROGRAM}} i update LIN-123 -s Done
```
