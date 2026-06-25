---
name: linear-watch
description: Watch Linear issues for changes. Use for monitoring updates.
allowed-tools: Bash
---

# Watch

Stream Linear changes for automation.

## Start here

```bash
{{CLI_PROGRAM}} watch comments --mine --output ndjson
{{CLI_PROGRAM}} watch comments LIN-123 --since -1h --output ndjson
{{CLI_PROGRAM}} watch comments --team ENG --comment-filter 'comment.body~=@agent' --output ndjson
```

## Agent notes
- Use NDJSON for daemon/agent pipelines.
- Add `--state-file PATH` to avoid replaying already-seen comments.
- Full syntax and less-common flags: `{{CLI_PROGRAM}} watch comments --help`.
