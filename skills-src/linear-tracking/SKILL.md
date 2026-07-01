---
name: linear-tracking
description: Track and observe Linear activity - velocity and burndown metrics, issue history and audit trails, time tracking, live watch streams, webhooks, and notifications. Use when monitoring changes, streaming events, logging time, or reviewing activity.
allowed-tools: Bash
---

# Tracking & Monitoring

Metrics, issue history, time tracking, live event streams, webhooks, and notifications.

## Command map

```bash
{{CLI_PROGRAM}} metrics velocity ENG                       # metrics (cycle CYCLE_ID, project PROJECT_ID)
{{CLI_PROGRAM}} history issue LIN-123                      # audit trail (or: i get LIN-123 --history)
{{CLI_PROGRAM}} time log LIN-123 1h -d "Review"            # time tracking (time list -i LIN-123)
{{CLI_PROGRAM}} watch comments --mine --output ndjson      # stream events (--team, --since -1h)
{{CLI_PROGRAM}} wh create https://ex.com/hook --events Issue  # webhooks (list/rotate-secret/listen)
{{CLI_PROGRAM}} n list                                     # notifications (read/archive)
```

## Agent notes

- Use NDJSON (`--output ndjson`) for daemon/agent pipelines; add `--state-file PATH` to `watch` to avoid replaying seen events.
- Filter watch streams server-side, e.g. `--comment-filter 'comment.body~=@agent'`.
- For parsing, add `--output json --compact`; trim payloads with `--fields a,b.c`.
- Full syntax: `{{CLI_PROGRAM}} watch comments --help`, `{{CLI_PROGRAM}} metrics --help`, `{{CLI_PROGRAM}} wh --help`, etc.
