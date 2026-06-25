---
name: linear-api
description: Execute raw GraphQL queries and mutations against the Linear API. Use for advanced operations not covered by other commands.
allowed-tools: Bash
---

# Raw GraphQL API

Run GraphQL when no first-class command covers the task.

## Start here

```bash
linear api query '{ viewer { id name email } }'
linear api query -v teamId=abc 'query($teamId: String!) { team(id: $teamId) { name } }'
linear api query -v first=10 -v archived=false 'query($first: Int, $archived: Boolean) { issues(first: $first, includeArchived: $archived) { nodes { identifier title } } }'
linear api mutate -v title=Bug - < mutation.graphql
```

## Agent notes
- Prefer typed CLI commands first; raw GraphQL is the escape hatch.
- Use `-v key=value` for variables and `-` to read query text from stdin.
- Full syntax and less-common flags: `linear api query --help`, `linear api mutate --help`.
