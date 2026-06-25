---
name: linear-done
description: Mark the current branch's issue as Done. Use as a quick shortcut to close the issue you're working on.
allowed-tools: Bash
---

# Done Shortcut

Mark the issue in the current branch as done.

## Start here

```bash
{{CLI_PROGRAM}} done
{{CLI_PROGRAM}} done --status "Ready for Review"
{{CLI_PROGRAM}} context --output json --compact
```

## Agent notes
- Full syntax and less-common flags: `{{CLI_PROGRAM}} done --help`.
