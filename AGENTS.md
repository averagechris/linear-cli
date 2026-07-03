## Linear Integration

Use `linear` for all Linear.app operations. Do not use Linear MCP tools.

### Commands
- `linear i list` - List issues
- `linear i list -t TEAM` - List team's issues
- `linear i create "Title" -t TEAM` - Create issue
- `linear i get LIN-123` - View issue details
- `linear i get LIN-1 LIN-2 LIN-3` - Batch fetch multiple issues
- `linear i get LIN-123 --output json` - View as JSON
- `linear i update LIN-123 -s Done` - Update status
- `linear i start LIN-123 --checkout` - Start work (assign + branch)
- `linear g pr LIN-123` - Create GitHub PR
- `linear g pr LIN-123 --draft` - Create draft PR
- `linear s issues "query"` - Search issues
- `linear context` - Get current issue from git branch
- `linear hy check --output json` - Workflow-hygiene findings with executable fixes
- `linear hy fix --yes` - Bulk-execute deterministic (command-kind) fixes
- `linear hy apply KEY --option ACTION` - Apply one stored fix (or `--input "text"`)
- `linear hy rules --init` / `--schema` - Scaffold hygiene.toml / print field model
- `linear hy check --rules PATH` - Load alternate hygiene rules (env fallback: `LINEAR_CLI_HYGIENE_RULES`; flag > env > default)
- `linear update` - Compare the current binary to canonical SourceHut `vX.Y.Z` tags (no self-update)
- `linear cm list ISSUE_ID --output json` - Get comments as JSON
- `linear up fetch URL -f file.png` - Download attachments

### Agent-Friendly Flags
- `--output json` - Machine-readable output
- `--compact` - Compact JSON output (no pretty formatting)
- `--fields a,b,c` - Limit JSON output to selected fields (supports dot paths)
- `--sort field` - Sort JSON array output by field (default: identifier/id)
- `--order asc|desc` - Sort order for JSON array output
- `--quiet` or `-q` - Suppress decorative output
- `--id-only` - Output only created/updated ID where command help documents support
- `--api-key KEY` - Override API key for this invocation only (never persist or export it)
- `--dry-run` - Preview where command help documents support
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
- `linear i create/update` accept `--data` JSON input (use `-` for stdin)
- `linear agent` prints agent-focused capabilities and examples
- `linear update --check` compares the current binary to canonical SourceHut `vX.Y.Z` tags without installing or mutating anything
- `nix run .#linear` is the minimal package; `nix run .#linear-bundled` wraps the CLI with runtime tools like `git`, `gh`, `jj`, and `less`
- JSON samples live in `docs/json/`
- Use `--help` on any command for full options
- **This fork is keyring-only for credentials** — never add a plaintext config fallback for API keys or OAuth tokens. Migrate secrets into the OS keyring and keep `config.toml` metadata-only.
- **Never reintroduce ambient secret propagation** — `--api-key` / `--profile` should stay in process-local runtime overrides, not `std::env::set_var`, so child processes do not inherit secrets.
- **Do not reintroduce env-based auth/profile overrides** — `LINEAR_API_KEY` and `LINEAR_CLI_PROFILE` are intentionally unsupported in this fork. Use keyring-backed auth plus explicit `--api-key` / `--profile` flags for one-off invocations.
- **Tests must never touch the OS keyring or the network** — integration tests that execute commands past argument parsing belong in `tests/mock_api_tests.rs`, which uses the hidden `--api-url` flag plus an explicit `--api-key` and an isolated `HOME`. `--api-url` is for tests/mocks only and requires `--api-key` (enforced at parse time and in `graphql_endpoint`), so keyring credentials can never be sent to a custom endpoint. Mock responses live in `tests/fixtures/linear_api/mock_rules.json`; `scripts/check_api_fixtures.py --online` (wired into `jj lint`) validates them against the live schema using the installed `linear` binary.
- **Do not reintroduce self-update or GitHub-release install flows** — this fork uses manual package-manager / flake updates and SourceHut `vX.Y.Z` tags as the release source. If you need validation, prefer `nix flake check` for build+fmt and run `nix run .#ci-clippy` / `nix run .#ci-test` separately.
- **Upstream review memory** — upstream was reviewed through `v0.3.26` / `master@upstream` commit `42780fb` on 2026-06-15. Already ported from `v0.3.22`-`v0.3.26`: history `relationChanges` query removal (`d385887e`), sprint velocity ordering/pagination (`b2b6182f`), failure exit-code/GraphQL classification (`42780fb`), auth-scoped cache dirs (`f488cac`), comments/teams/templates terminal sanitization (`c771414`), priority test deflake (`b3ccdd2`), and setup API-key validation-before-save (`2e56d88`). Future upstream review should start after `42780fb`; keep excluding GitHub Actions/releases, self-update flows, env-based auth overrides, upstream `update.rs` GitHub-release logic, and only revisit absolute pager path opt-in (`c32f166`) if users need it.

## Failure Modes

### Pager leaves terminal in a bad state after a successful command

| symptom | evidence | root cause | wrong instinct | corrected default behavior | where the lesson belongs next |
| --- | --- | --- | --- | --- | --- |
| On macOS, a successful table-output command can leave the shell acting raw-ish until `reset` or `stty sane`. | `stty -a` after the command may show `pendin`; piping through an external pager can avoid the symptom. | Auto-pager cleanup depended on a guard `Drop`, but `async_main` called `std::process::exit`, which skips destructors entirely. The pager path also redirected stdout with `dup2` without restoring the original fd on teardown. | Tweaking `LESS`, `PAGER`, or termios flags first. | When touching pager code, preserve a saved stdout fd, restore it before pager shutdown, and return an exit code to `main` instead of calling `std::process::exit` while cleanup guards are still in scope. | Keep this note in `AGENTS.md` and add/keep a Unix regression test around stdout redirection in `src/main.rs`. |
| A future change tries to reintroduce self-update, GitHub release checks, env-based auth, or GitHub Actions CI. | You see updater code hitting GitHub releases, docs mentioning `LINEAR_API_KEY` / `LINEAR_CLI_PROFILE`, release workflows, or docs telling users to let the CLI update itself. | This fork hardens supply-chain behavior by removing self-modifying update paths, using canonical SourceHut tags for version checks, keeping credentials in the keyring, and replacing hosted CI with local Nix-backed validation. | Restoring convenience updater/release plumbing because upstream had it. | Keep updates explicit and warning-only, compare versions against SourceHut `vX.Y.Z` tags, use keyring-backed auth plus explicit one-off flags, and validate with `nix flake check`, `nix run .#ci-clippy`, and `nix run .#ci-test`. | Keep this note in `AGENTS.md` and revisit only if the fork intentionally adopts a reviewed release pipeline again. |

<!-- Last audited: 2026-06-15 | updated upstream review cutoff through v0.3.26 / 42780fb and recorded selectively ported fixes -->
