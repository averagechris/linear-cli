# Usage Examples

Use `linear common` for common tasks and `linear <command> --help` for full syntax.
The primary binary is `linear`; `linear-cli` may exist as a compatibility symlink.

## Issues

```bash
linear i list --mine
linear i list -t ENG -s "In Progress"
linear i get LIN-123 --output json --compact
linear i create "Bug fix" -t ENG -p 1 --id-only
linear i update LIN-123 -s Done --dry-run
linear i start LIN-123 --checkout
```

## Projects, teams, and cycles

```bash
linear p list
linear p get PROJECT_ID --output json --compact
linear p create "Q1 Roadmap" -t ENG
linear t members ENG
linear c current -t ENG
linear sp status -t ENG
```

## Comments and attachments

```bash
linear cm list LIN-123 --output json --compact
linear cm create LIN-123 -b "Update posted"
linear att list LIN-123
linear up fetch URL -f /tmp/screenshot.png
```

## Search and context

```bash
linear s issues "authentication bug" --output json --compact --fields identifier,title,state.name
linear context --output json --compact
linear i get LIN-123 --comments --history --output json --compact
```

## Git and PRs

```bash
linear g branch LIN-123
linear g checkout LIN-123
linear g checkout LIN-123 --vcs jj
linear g pr LIN-123 --draft
```

## Watch comments for automation

`watch comments` emits one NDJSON event per new matching comment. Pipe it to your own daemon; the CLI does not provide an `--exec` mode.

```bash
linear watch comments LIN-123 LIN-456 --output ndjson
linear watch comments --mine --source slack --output ndjson | ./handle-linear-comment
linear watch comments --team ENG --comment-filter 'comment.body~=@agent' --output ndjson
linear watch comments LIN-123 --since -1h --state-file ~/.cache/linear/watch.json --output ndjson
```

Each event includes issue, comment, author, URL, labels, assignee, and external sync metadata when present.

## Bulk operations

```bash
linear b update-state Done -i LIN-1,LIN-2
linear b assign me -i LIN-1,LIN-2
linear b label bug -i LIN-1,LIN-2
linear b unassign -i LIN-1,LIN-2
```

## Import/export

```bash
linear import csv issues.csv -t ENG --dry-run
linear import json issues.json -t ENG
linear export csv -t ENG -f issues.csv
linear export projects-csv -f projects.csv
```

## Interactive, workspaces, and config

```bash
linear interactive --team ENG
linear config workspace-list
linear config workspace-add personal
linear config workspace-switch personal
linear auth status
linear doctor
```

## JSON and agent output

```bash
linear i list --output json --compact --fields identifier,title,state.name
linear i list --output ndjson --filter state.name="In Progress"
linear p list --output json --sort name --order asc
linear cm list LIN-123 --output json --compact
linear i create "Task" -t ENG --id-only --quiet
```
