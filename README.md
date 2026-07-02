# linear-cli

[![Crates.io](https://img.shields.io/crates/v/linear-cli)](https://crates.io/crates/linear-cli)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)

A fast Linear.app CLI for issues, projects, cycles, teams, documents, and automation.

The primary command is `linear`; package-manager installs may also provide `linear-cli` as a compatibility symlink.
Credentials are stored in the OS keyring only.

## Install

Hosted release downloads, checksums, and release notes: <https://averagechris.srht.site/linear-cli/>.

```bash
# Homebrew tap package (replace with your tap)
brew tap your-org/tap
brew install your-org/tap/linear-cli

# Nix
nix run .#linear -- --help
nix run .#linear-bundled -- --help       # includes git/jj/gh/less wrappers

# Cargo
cargo install linear-cli
```

From source:

```bash
git clone https://github.com/Finesssee/linear-cli.git
cd linear-cli
cargo build --release
```

## Update

`linear update` checks canonical SourceHut `vX.Y.Z` release tags and reports whether a newer release exists. It does not self-update.

```bash
linear update --check
brew upgrade your-org/tap/linear-cli
cargo install --locked linear-cli
nix flake update
```

Release/build details live in [docs/downloads.md](docs/downloads.md) and [docs/homebrew.md](docs/homebrew.md).

## Quick start

```bash
linear auth login                         # Store API key in OS keyring
linear i list --mine                      # My issues
linear i get LIN-123                      # Issue details
linear i start LIN-123 --checkout         # Assign + In Progress + branch
linear done                               # Mark current branch issue Done
linear g pr LIN-123 --draft               # Create linked GitHub PR
```

Discover more:

```bash
linear common                             # Common tasks
linear agent                              # Agent/scripting patterns
linear <command> --help                   # Full syntax
linear completions static zsh > ~/.zfunc/_linear
```

## Command map

| Task | Command | Example |
| --- | --- | --- |
| Issues | `i`, `issues` | `linear i list --mine`, `linear i create "Bug" -t ENG` |
| Projects | `p`, `projects` | `linear p list`, `linear p get PROJECT_ID` |
| Teams/users | `t`, `teams`, `u`, `users` | `linear t members ENG`, `linear u get me` |
| Cycles/sprints | `c`, `cycles`, `sp`, `sprint` | `linear c current -t ENG`, `linear sp status -t ENG` |
| Comments | `cm`, `comments` | `linear cm list LIN-123 --output json` |
| Search | `s`, `search` | `linear s issues "auth bug"` |
| Git/PR | `g`, `git` | `linear g checkout LIN-123`, `linear g pr LIN-123` |
| Bulk updates | `b`, `bulk` | `linear b update-state Done -i LIN-1,LIN-2` |
| Attachments/uploads | `att`, `up` | `linear att list LIN-123`, `linear up fetch URL -f image.png` |
| Config/auth | `auth`, `config`, `doctor` | `linear auth status`, `linear doctor` |
| Automation | `watch`, `api` | `linear watch comments --mine --output ndjson` |

## Agent and script ergonomics

Useful global flags:

```bash
--output table|json|ndjson       # machine-readable with json/ndjson
--compact --fields a,b.c         # smaller JSON payloads
--filter field=value             # dot-path filters; =, !=, ~= contains
--limit N --all                  # result limits and pagination
--quiet --no-color               # cleaner CI/log output
--dry-run                        # preview only when command help documents support
--id-only                        # chaining where command help documents support
--yes                            # confirmation for mutations
```

Examples:

```bash
linear i list --output json --compact --fields identifier,title,state.name
linear cm list LIN-123 --output json --compact
linear watch comments --mine --output ndjson | ./handle-linear-comment
linear i update LIN-123 --data - --dry-run
linear up fetch URL -f /tmp/screenshot.png
```

Exit codes: `0` success, `1` general error, `2` not found, `3` auth, `4` rate limited.
JSON examples live in [docs/json/](docs/json/). Agent Skills are documented in [docs/skills.md](docs/skills.md) and agent setup in [docs/ai-agents.md](docs/ai-agents.md).

## Common workflows

### Create and update an issue

```bash
linear i create "Fix login" -t ENG -p 1 --id-only
linear i update LIN-123 -s "In Progress" -a me
linear cm create LIN-123 -b "Investigating now"
```

### Start work and open a PR

```bash
linear i start LIN-123 --checkout
# make changes, commit with your normal VCS flow
linear g pr LIN-123 --draft
```

### Search, inspect, and fetch context

```bash
linear s issues "oauth callback" --output json --compact --fields identifier,title,state.name
linear i get LIN-123 --comments --history --output json --compact
linear up fetch URL -f /tmp/attachment.png
```

### Bulk operations

```bash
linear b update-state Done -i LIN-1,LIN-2
linear b assign me -i LIN-1,LIN-2
linear b label bug -i LIN-1,LIN-2
```

## Configuration notes

- `linear auth login` stores credentials in the OS keyring.
- Use `--api-key KEY` or `--profile NAME` for one invocation.
- Plaintext config fallback and env-based auth/profile overrides are intentionally unsupported in this fork.
- Use `linear doctor` when auth, config, cache, or connectivity looks wrong.

## Docs

- [Usage examples](docs/examples.md)
- [Workflows](docs/workflows.md)
- [AI agents](docs/ai-agents.md)
- [Agent Skills](docs/skills.md)
- [Shell completions](docs/shell-completions.md)
- [Hosted downloads](docs/downloads.md)
- [Homebrew artifact contract](docs/homebrew.md)
- [Changelog](CHANGELOG.md)

## Development

```bash
cargo build
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt -- --check
```

Nix-backed checks:

```bash
nix flake check
nix run .#ci-clippy
nix run .#ci-test
```

## License

Dual licensed under either of:

- [MIT](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.
