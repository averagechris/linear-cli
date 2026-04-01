## Linear Integration

Use `linear-cli` for all Linear.app operations. Do not use Linear MCP tools.

### Commands
- `linear-cli i list` - List issues
- `linear-cli i list -t TEAM` - List team's issues
- `linear-cli i create "Title" -t TEAM` - Create issue
- `linear-cli i get LIN-123` - View issue details
- `linear-cli i get LIN-1 LIN-2 LIN-3` - Batch fetch multiple issues
- `linear-cli i get LIN-123 --output json` - View as JSON
- `linear-cli i update LIN-123 -s Done` - Update status
- `linear-cli i start LIN-123 --checkout` - Start work (assign + branch)
- `linear-cli g pr LIN-123` - Create GitHub PR
- `linear-cli g pr LIN-123 --draft` - Create draft PR
- `linear-cli s issues "query"` - Search issues
- `linear-cli context` - Get current issue from git branch
- `linear-cli update` - Compare the current binary to canonical SourceHut `vX.Y.Z` tags (no self-update)
- `linear-cli cm list ISSUE_ID --output json` - Get comments as JSON
- `linear-cli up fetch URL -f file.png` - Download attachments

### Agent-Friendly Flags
- `--output json` - Machine-readable output
- `--compact` - Compact JSON output (no pretty formatting)
- `--fields a,b,c` - Limit JSON output to selected fields (supports dot paths)
- `--sort field` - Sort JSON array output by field (default: identifier/id)
- `--order asc|desc` - Sort order for JSON array output
- `--quiet` or `-q` - Suppress decorative output
- `--id-only` - Output only created/updated ID
- `--api-key KEY` - Override API key for this invocation only (never persist or export it)
- `--dry-run` - Preview without executing (create)
- `-d -` - Read description from stdin

### Exit Codes
- 0 = Success
- 1 = General error
- 2 = Not found
- 3 = Auth error
- 4 = Rate limited

### Notes
- Set `LINEAR_CLI_OUTPUT=json` to default all output to JSON
- Errors with `--output json` return `{"error": true, "message": "...", "code": N, "details": {...}, "retry_after": N}`
- `linear-cli i create/update` accept `--data` JSON input (use `-` for stdin)
- `linear-cli agent` prints agent-focused capabilities and examples
- `linear-cli update --check` compares the current binary to canonical SourceHut `vX.Y.Z` tags without installing or mutating anything
- `nix run .#linear-cli` is the minimal package; `nix run .#linear-cli-bundled` wraps the CLI with runtime tools like `git`, `gh`, `jj`, and `less`
- JSON samples live in `docs/json/`
- Use `--help` on any command for full options
- **This fork is keyring-only for credentials** — never add a plaintext config fallback for API keys or OAuth tokens. Migrate secrets into the OS keyring and keep `config.toml` metadata-only.
- **Never reintroduce ambient secret propagation** — `--api-key` / `--profile` should stay in process-local runtime overrides, not `std::env::set_var`, so child processes do not inherit secrets.
- **Do not reintroduce env-based auth/profile overrides** — `LINEAR_API_KEY` and `LINEAR_CLI_PROFILE` are intentionally unsupported in this fork. Use keyring-backed auth plus explicit `--api-key` / `--profile` flags for one-off invocations.
- **Do not reintroduce self-update or GitHub-release install flows** — this fork uses manual package-manager / flake updates and SourceHut `vX.Y.Z` tags as the release source. If you need validation, prefer `nix flake check` for build+fmt and run `nix run .#ci-clippy` / `nix run .#ci-test` separately.

## Failure Modes

### Pager leaves terminal in a bad state after a successful command

| symptom | evidence | root cause | wrong instinct | corrected default behavior | where the lesson belongs next |
| --- | --- | --- | --- | --- | --- |
| On macOS, a successful table-output command can leave the shell acting raw-ish until `reset` or `stty sane`. | `stty -a` after the command may show `pendin`; piping through an external pager can avoid the symptom. | Auto-pager cleanup depended on a guard `Drop`, but `async_main` called `std::process::exit`, which skips destructors entirely. The pager path also redirected stdout with `dup2` without restoring the original fd on teardown. | Tweaking `LESS`, `PAGER`, or termios flags first. | When touching pager code, preserve a saved stdout fd, restore it before pager shutdown, and return an exit code to `main` instead of calling `std::process::exit` while cleanup guards are still in scope. | Keep this note in `AGENTS.md` and add/keep a Unix regression test around stdout redirection in `src/main.rs`. |
| A future change tries to reintroduce self-update, GitHub release checks, env-based auth, or GitHub Actions CI. | You see updater code hitting GitHub releases, docs mentioning `LINEAR_API_KEY` / `LINEAR_CLI_PROFILE`, release workflows, or docs telling users to let the CLI update itself. | This fork hardens supply-chain behavior by removing self-modifying update paths, using canonical SourceHut tags for version checks, keeping credentials in the keyring, and replacing hosted CI with local Nix-backed validation. | Restoring convenience updater/release plumbing because upstream had it. | Keep updates explicit and warning-only, compare versions against SourceHut `vX.Y.Z` tags, use keyring-backed auth plus explicit one-off flags, and validate with `nix flake check`, `nix run .#ci-clippy`, and `nix run .#ci-test`. | Keep this note in `AGENTS.md` and revisit only if the fork intentionally adopts a reviewed release pipeline again. |

<!-- Last audited: 2026-04-01 | aligned AGENTS.md with hardened fork policy: keyring-only secrets, SourceHut tag checks, minimal vs bundled flake packages, explicit one-off overrides, and Nix-backed validation -->
