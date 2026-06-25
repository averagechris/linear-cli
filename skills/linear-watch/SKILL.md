---
name: linear-watch
description: Watch Linear issues for changes. Use for monitoring updates.
allowed-tools: Bash
---

# Watch

Stream Linear changes for automation.

## Start here

```bash
linear watch comments --mine --output ndjson
linear watch comments LIN-123 --since -1h --output ndjson
linear watch comments --team ENG --comment-filter 'comment.body~=@agent' --output ndjson
```

## Agent notes
- Use NDJSON for daemon/agent pipelines.
- Add `--state-file PATH` to avoid replaying already-seen comments.
- Full syntax and less-common flags: `linear watch comments --help`.
