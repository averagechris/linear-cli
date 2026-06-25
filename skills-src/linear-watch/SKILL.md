---
name: linear-watch
description: Watch Linear issues for changes. Use for monitoring updates.
allowed-tools: Bash
---

# Watch

```bash
# Watch single issue (polls for changes)
{{CLI_PROGRAM}} watch issue LIN-123

# Custom interval
{{CLI_PROGRAM}} watch issue LIN-123 --interval 30   # Poll every 30 seconds
{{CLI_PROGRAM}} watch issue LIN-123 -i 60           # Poll every 60 seconds

# JSON output for scripting
{{CLI_PROGRAM}} watch issue LIN-123 --output json

# Watch comments for automation (one JSON event per line)
{{CLI_PROGRAM}} watch comments LIN-123 --output ndjson
{{CLI_PROGRAM}} watch comments --mine --source slack --output ndjson
{{CLI_PROGRAM}} watch comments --mine --comment-filter 'comment.body~=@agent' --output ndjson
{{CLI_PROGRAM}} watch comments --subscribed --search oauth --output ndjson
```

## Comment Watch Pipe Contract

Use `--output ndjson` and pipe events into another process. The CLI intentionally
does not provide `--exec`; each output line includes issue, comment, author,
parent, URL, labels, assignee, and external sync metadata for downstream routing.

```bash
{{CLI_PROGRAM}} watch comments --mine --source slack --output ndjson \
  | ./handle-linear-comment
```

## Flags

| Flag | Purpose |
|------|---------|
| `-i, --interval` | Seconds between polls (default: 10) |
| `--output json` | JSON output |
| `--output ndjson` | Streaming JSON output, one event per line |
| `--source SERVICE` | Filter comments synced from a service (repeat or comma-separate) |
| `--comment-filter FIELD_OP_VALUE` | Dot-path event filter (`=`, `!=`, `~=`) |
| `--mine`, `--subscribed`, `--search QUERY`, `--view VIEW` | Select watched issue sets |
| `--state-file PATH` | Persist seen comment IDs across daemon restarts |
| `--since now|-1h|RFC3339` | Startup backfill window (default: future comments only) |
