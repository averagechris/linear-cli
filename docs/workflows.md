# Example Workflows

## Daily work

```bash
linear i list --mine
linear i start LIN-123 --checkout
# work with your normal editor/VCS flow
linear cm create LIN-123 -b "Status: implementation is ready for review"
linear done
```

## Create, triage, and assign

```bash
linear i create "Login button not working" -t ENG -p 2 -l bug --id-only
linear i update LIN-456 -a me -s "In Progress"
linear b label urgent -i LIN-456,LIN-789
linear triage list -t ENG
```

## Branch and PR

```bash
linear context --output json --compact
linear g checkout LIN-123
linear g checkout LIN-123 --vcs jj
linear g pr LIN-123 --draft
```

## Agent daemon watching comments

```bash
linear watch comments --mine --source slack --output ndjson \
  --state-file ~/.cache/linear/watch-comments.json \
  | ./handle-linear-comment
```

## Reporting

```bash
linear i list -t ENG --output json --compact --fields identifier,title,state.name
linear sp velocity -t ENG -n 10
linear export csv -t ENG -f issues.csv
```
