//! Hermetic integration tests that exercise real command paths against a
//! local mock of the Linear GraphQL API.
//!
//! Design constraints:
//! - Never touch the OS keyring: every invocation passes an explicit
//!   `--api-key`, and `HOME` points at a per-test temp dir so no real
//!   config, cache, or keychain state is read or written.
//! - Never touch the network: `--api-url` points at a local mock server.
//! - The mock responses live in `tests/fixtures/linear_api/mock_rules.json`
//!   and are kept honest by `scripts/check_api_fixtures.py` (wired into
//!   `jj lint`), which validates the fixture operations against the live
//!   Linear schema and the responses against the operations' selection sets.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;

const TEST_API_KEY: &str = "lin_api_mock_key_for_tests_only";

/// A rule loaded from the fixture file: match substrings + canned response.
/// `match` substrings target the operation text and `match_variables` the
/// request variables; the server tests both against the whole request body.
struct MockRule {
    matches: Vec<String>,
    response: String,
}

fn load_rules() -> Vec<MockRule> {
    let raw = include_str!("fixtures/linear_api/mock_rules.json");
    let parsed: serde_json::Value =
        serde_json::from_str(raw).expect("mock_rules.json is valid JSON");
    parsed["rules"]
        .as_array()
        .expect("mock_rules.json has a rules array")
        .iter()
        .map(|rule| {
            let substrings = |key: &str| -> Vec<String> {
                rule[key]
                    .as_array()
                    .map(|items| {
                        items
                            .iter()
                            .map(|m| m.as_str().expect("match entries are strings").to_string())
                            .collect()
                    })
                    .unwrap_or_default()
            };
            let mut matches = substrings("match");
            matches.extend(substrings("match_variables"));
            assert!(!matches.is_empty(), "rule has match substrings");
            MockRule {
                matches,
                response: rule["response"].to_string(),
            }
        })
        .collect()
}

/// Start a minimal HTTP server serving fixture responses on a random port.
/// The accept-loop thread is detached; it dies with the test process.
fn start_mock_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let addr = listener.local_addr().expect("mock server addr");
    std::thread::spawn(move || {
        let rules = load_rules();
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let Some(body) = read_http_request(&mut stream) else {
                continue;
            };
            let response_body = rules
                .iter()
                .find(|rule| rule.matches.iter().all(|m| body.contains(m.as_str())))
                .map(|rule| rule.response.clone())
                .unwrap_or_else(|| {
                    format!(
                        "{{\"errors\":[{{\"message\":\"mock server has no rule for request: {}\"}}]}}",
                        body.replace('"', "'").chars().take(120).collect::<String>()
                    )
                });
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    format!("http://{}", addr)
}

/// Read one HTTP request and return its body.
fn read_http_request(stream: &mut std::net::TcpStream) -> Option<String> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end;
    loop {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end = pos + 4;
            break;
        }
        if buf.len() > 1_048_576 {
            return None;
        }
    }
    let headers = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let content_length: usize = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().ok())?
        })
        .unwrap_or(0);
    while buf.len() < header_end + content_length {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    Some(String::from_utf8_lossy(&buf[header_end..]).to_string())
}

/// Run the CLI against the mock server with a fully isolated HOME.
/// Returns (exit code, stdout, stderr).
fn run_cli_mocked(
    server_url: &str,
    home: &std::path::Path,
    args: &[&str],
) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_linear"))
        .env("HOME", home)
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("XDG_CACHE_HOME")
        .args(["--api-key", TEST_API_KEY, "--api-url", server_url])
        .args(args)
        .output()
        .expect("Failed to execute command");

    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

#[test]
fn whoami_returns_mocked_viewer() {
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();
    let (code, stdout, stderr) =
        run_cli_mocked(&server, home.path(), &["whoami", "--output", "json"]);
    assert_eq!(code, 0, "whoami should succeed against mock: {}", stderr);
    assert!(
        stdout.contains("test.user@example.com"),
        "whoami should return the mocked viewer, got: {}",
        stdout
    );
}

#[test]
fn issue_create_succeeds_against_mock() {
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();
    let (code, stdout, stderr) = run_cli_mocked(
        &server,
        home.path(),
        &[
            "issues",
            "create",
            "Test issue from mock",
            "-t",
            "FAKE",
            "--output",
            "json",
        ],
    );
    assert_eq!(code, 0, "create should succeed against mock: {}", stderr);
    assert!(
        stdout.contains("FAKE-123"),
        "create should return the mocked identifier, got: {}",
        stdout
    );
}

#[test]
fn issue_create_resolves_status_name_via_mock() {
    // Exercises the statuses fetch + team-UUID-keyed cache write end to end.
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();
    let (code, stdout, stderr) = run_cli_mocked(
        &server,
        home.path(),
        &[
            "issues",
            "create",
            "Test issue from mock",
            "-t",
            "FAKE",
            "-s",
            "In Progress",
            "--output",
            "json",
        ],
    );
    assert_eq!(
        code, 0,
        "create with status name should succeed against mock: {}",
        stderr
    );
    assert!(
        stdout.contains("FAKE-123"),
        "create should return the mocked identifier, got: {}",
        stdout
    );
}

#[test]
fn issue_create_dry_run_previews_without_creating() {
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();
    let (code, stdout, stderr) = run_cli_mocked(
        &server,
        home.path(),
        &[
            "issues",
            "create",
            "Dry run issue",
            "-t",
            "FAKE",
            "--dry-run",
        ],
    );
    assert_eq!(code, 0, "dry-run create should succeed: {}", stderr);
    assert!(
        stdout.contains("DRY RUN") || stdout.contains("dry_run"),
        "dry-run should print a preview, got: {}",
        stdout
    );
    assert!(
        !stdout.contains("FAKE-123"),
        "dry-run must not reach the create mutation"
    );
}

#[test]
fn api_url_requires_explicit_api_key() {
    // Security invariant: keyring credentials are never sent to a custom
    // endpoint, so --api-url without --api-key must be rejected at parse time.
    let home = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_linear"))
        .env("HOME", home.path())
        .args(["whoami", "--api-url", "http://127.0.0.1:9"])
        .output()
        .expect("Failed to execute command");
    let code = output.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert_ne!(code, 0, "--api-url without --api-key must fail");
    assert!(
        stderr.contains("api-key") || stderr.contains("api_key"),
        "error should point at the missing --api-key, got: {}",
        stderr
    );
}

#[test]
fn initiatives_list_succeeds_against_mock() {
    // Regression: the list query selected `Initiative.progress`, which the
    // live schema removed, so every `initiatives list` failed with a 400.
    // The fixture operation is validated against the live schema by
    // scripts/check_api_fixtures.py --online, guarding future drift.
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();
    let (code, stdout, stderr) = run_cli_mocked(
        &server,
        home.path(),
        &["initiatives", "list", "--output", "json"],
    );
    assert_eq!(code, 0, "initiatives list should succeed: {}", stderr);
    assert!(
        stdout.contains("Fake Initiative"),
        "list should return the mocked initiative, got: {}",
        stdout
    );

    // Table mode renders without the removed progress field.
    let (code, stdout, stderr) = run_cli_mocked(&server, home.path(), &["initiatives", "list"]);
    assert_eq!(code, 0, "table mode should succeed: {}", stderr);
    assert!(
        stdout.contains("Fake Initiative"),
        "table should include the initiative, got: {}",
        stdout
    );
}

#[test]
fn project_update_status_resolves_project_statuses_connection() {
    // Regression: `projectStatuses` is a connection; querying it without the
    // nodes wrapper failed with GRAPHQL_VALIDATION_FAILED (HTTP 400), so
    // `p update --status <name>` was broken end to end.
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();
    let (code, stdout, stderr) = run_cli_mocked(
        &server,
        home.path(),
        &[
            "projects",
            "update",
            "cccc3333-0000-4000-8000-00000000000d",
            "--status",
            "Canceled",
            "--output",
            "json",
        ],
    );
    assert_eq!(code, 0, "status update should succeed: {}", stderr);
    assert!(
        stdout.contains("Fake Project"),
        "update should return the mocked project, got: {}",
        stdout
    );
}

#[test]
fn project_update_label_flag_resolves_project_labels() {
    // `p update -l` must resolve names through `projectLabels` (a separate
    // label system from issue labels) and send `labelIds`.
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();
    let (code, stdout, stderr) = run_cli_mocked(
        &server,
        home.path(),
        &[
            "projects",
            "update",
            "cccc3333-0000-4000-8000-00000000000d",
            "-l",
            "platform",
            "--output",
            "json",
        ],
    );
    assert_eq!(code, 0, "label update should succeed: {}", stderr);
    assert!(
        stdout.contains("Fake Project"),
        "update should return the mocked project, got: {}",
        stdout
    );
}

// ---------------------------------------------------------------------------
// `linear hygiene` end-to-end tests (docs/hygiene.md R38)
// ---------------------------------------------------------------------------

/// The user-level linear-cli config dir inside an isolated HOME, mirroring
/// `dirs::config_dir()` per platform.
fn hygiene_config_dir(home: &std::path::Path) -> std::path::PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/linear-cli")
    } else {
        home.join(".config/linear-cli")
    }
}

fn write_hygiene_toml(home: &std::path::Path, content: &str) {
    let dir = hygiene_config_dir(home);
    std::fs::create_dir_all(&dir).expect("create config dir");
    std::fs::write(dir.join("hygiene.toml"), content).expect("write hygiene.toml");
}

/// Like `run_cli_mocked`, but with the working directory pinned to the temp
/// HOME so no repo-level `.linear.toml` can leak into context resolution.
fn run_hy(server_url: &str, home: &std::path::Path, args: &[&str]) -> (i32, String, String) {
    run_hy_env(server_url, home, &[], args)
}

/// `run_hy` with extra env vars (e.g. LINEAR_CLI_HYGIENE_RULES).
fn run_hy_env(
    server_url: &str,
    home: &std::path::Path,
    envs: &[(&str, &str)],
    args: &[&str],
) -> (i32, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_linear"));
    command
        .env("HOME", home)
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("XDG_CACHE_HOME")
        .env_remove("LINEAR_CLI_HYGIENE_RULES")
        .current_dir(home)
        .args(["--api-key", TEST_API_KEY, "--api-url", server_url])
        .args(args);
    for (key, value) in envs {
        command.env(key, value);
    }
    let output = command.output().expect("Failed to execute command");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

/// Issue + project + initiative rules matching the hygiene_* fixtures.
const HYGIENE_RULES_ALL_ENTITIES: &str = r#"
[[hygiene.rules]]
id = "urgent-in-backlog"
entity = "issue"
severity = "high"
[hygiene.rules.when]
priority = { eq = 1 }
status = { in = ["Backlog"] }
[hygiene.rules.fix]
set = { status = "Todo" }

[[hygiene.rules]]
id = "stale-in-review"
entity = "issue"
severity = "high"
[hygiene.rules.when]
status = { in = ["In Review"] }
updatedAt = { older_than = "2d" }
[[hygiene.rules.fix.options]]
action = "nudge_review"
comment = true
[[hygiene.rules.fix.options]]
action = "move_back"
set = { status = "In Progress" }

[[hygiene.rules]]
id = "needs-update"
entity = "issue"
[hygiene.rules.when]
status = { in = ["In Progress"] }
updatedAt = { older_than = "2d" }
[[hygiene.rules.fix.options]]
action = "post_update"
comment = true

[[hygiene.rules]]
id = "project-no-lead"
entity = "project"
[hygiene.rules.when]
state = { in = ["started"] }
lead = { missing = true }

[[hygiene.rules]]
id = "initiative-stale-health"
entity = "initiative"
severity = "high"
[hygiene.rules.when]
state = { in = ["Active"] }
healthUpdatedAt = { older_than = "14d" }
"#;

/// Issue-only rules used by the fix/apply tests, including a label fix that
/// must merge with existing labels rather than replace them.
const HYGIENE_RULES_FIXABLE: &str = r#"
[[hygiene.rules]]
id = "urgent-in-backlog"
entity = "issue"
severity = "high"
[hygiene.rules.when]
priority = { eq = 1 }
status = { in = ["Backlog"] }
[hygiene.rules.fix]
set = { status = "Todo" }

[[hygiene.rules]]
id = "label-fix"
entity = "issue"
[hygiene.rules.when]
priority = { eq = 1 }
[hygiene.rules.fix]
set = { labels = "payments" }

[[hygiene.rules]]
id = "stale-in-review"
entity = "issue"
severity = "high"
[hygiene.rules.when]
status = { in = ["In Review"] }
updatedAt = { older_than = "2d" }
[[hygiene.rules.fix.options]]
action = "nudge_review"
comment = true
[[hygiene.rules.fix.options]]
action = "move_back"
set = { status = "In Progress" }

[[hygiene.rules]]
id = "needs-update"
entity = "issue"
[hygiene.rules.when]
status = { in = ["In Progress"] }
updatedAt = { older_than = "2d" }
[[hygiene.rules.fix.options]]
action = "post_update"
comment = true
"#;

fn parse_json(stdout: &str) -> serde_json::Value {
    serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!("expected JSON output, got error {e}: {stdout}");
    })
}

#[test]
fn hygiene_check_reports_findings_across_entities() {
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();
    write_hygiene_toml(home.path(), HYGIENE_RULES_ALL_ENTITIES);

    let (code, stdout, stderr) = run_hy(
        &server,
        home.path(),
        &["hygiene", "check", "-t", "FAKE", "--output", "json"],
    );
    assert_eq!(code, 0, "check should succeed: {stderr}");
    let findings = parse_json(&stdout);
    let findings = findings.as_array().expect("findings array");
    assert_eq!(findings.len(), 5, "expected 5 findings: {stdout}");

    // Deterministic sort: severity desc, then rule id, then identifier (R18).
    let keys: Vec<&str> = findings
        .iter()
        .map(|f| f["dedupeKey"].as_str().unwrap())
        .collect();
    assert_eq!(
        keys,
        vec![
            "initiative-stale-health:fake-initiative",
            "stale-in-review:FAKE-2",
            "urgent-in-backlog:FAKE-1",
            "needs-update:FAKE-3",
            "project-no-lead:fake-project",
        ]
    );

    // Finding shape (R16) + fix kinds across the derivation matrix.
    let urgent = &findings[2];
    assert_eq!(urgent["rule"], "urgent-in-backlog");
    assert_eq!(urgent["severity"], "high");
    assert_eq!(urgent["entity"]["type"], "issue");
    assert_eq!(urgent["entity"]["identifier"], "FAKE-1");
    assert_eq!(urgent["owner"]["displayName"], "test.user");
    assert!(urgent["summary"].as_str().unwrap().contains("FAKE-1"));
    assert!(urgent["evidence"]["updatedAt"].is_string());
    assert_eq!(urgent["fix"]["kind"], "command");
    assert_eq!(urgent["fix"]["command"], "linear i update FAKE-1 -s Todo");

    assert_eq!(findings[1]["fix"]["kind"], "options");
    assert_eq!(findings[1]["fix"]["options"][0]["action"], "nudge_review");
    assert_eq!(findings[3]["fix"]["kind"], "needs_input");
    assert_eq!(findings[4]["entity"]["type"], "project");
    assert!(findings[4]["fix"].is_null());

    // NDJSON honors one finding per line (R21).
    let (code, stdout, _) = run_hy(
        &server,
        home.path(),
        &["hygiene", "check", "-t", "FAKE", "--output", "ndjson"],
    );
    assert_eq!(code, 0);
    assert_eq!(stdout.trim().lines().count(), 5);
    for line in stdout.trim().lines() {
        assert!(parse_json(line)["dedupeKey"].is_string());
    }
}

#[test]
fn hygiene_check_without_rules_hints_and_exits_zero() {
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();

    // JSON mode: hint field (R8).
    let (code, stdout, _) = run_hy(
        &server,
        home.path(),
        &["hygiene", "check", "--output", "json"],
    );
    assert_eq!(code, 0, "zero rules must still exit 0");
    let payload = parse_json(&stdout);
    assert!(payload["hint"].as_str().unwrap().contains("rules --init"));
    assert_eq!(payload["findings"], serde_json::json!([]));

    // Table mode: hint on stderr; --quiet suppresses it.
    let (code, _, stderr) = run_hy(&server, home.path(), &["hygiene", "check"]);
    assert_eq!(code, 0);
    assert!(stderr.contains("rules --init"), "hint on stderr: {stderr}");
    let (code, _, stderr) = run_hy(&server, home.path(), &["hygiene", "check", "--quiet"]);
    assert_eq!(code, 0);
    assert!(!stderr.contains("rules --init"), "--quiet suppresses hint");
}

#[test]
fn hygiene_check_fail_if_findings_exits_one() {
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();
    write_hygiene_toml(home.path(), HYGIENE_RULES_ALL_ENTITIES);

    let (code, _, stderr) = run_hy(
        &server,
        home.path(),
        &[
            "hygiene",
            "check",
            "-t",
            "FAKE",
            "--fail-if-findings",
            "--output",
            "json",
        ],
    );
    assert_eq!(code, 1, "--fail-if-findings must exit 1: {stderr}");
}

#[test]
fn hygiene_rules_lists_effective_rules() {
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();
    write_hygiene_toml(home.path(), HYGIENE_RULES_ALL_ENTITIES);

    let (code, stdout, stderr) = run_hy(
        &server,
        home.path(),
        &["hygiene", "rules", "--output", "json"],
    );
    assert_eq!(code, 0, "valid config lists rules: {stderr}");
    let rules = parse_json(&stdout);
    let rules = rules.as_array().expect("rules array");
    assert_eq!(rules.len(), 5);
    let urgent = rules
        .iter()
        .find(|r| r["id"] == "urgent-in-backlog")
        .expect("urgent rule listed");
    assert_eq!(urgent["kind"], "when");
    assert_eq!(urgent["entity"], "issue");
    assert_eq!(urgent["severity"], "high");
    assert_eq!(urgent["enabled"], true);
    assert!(urgent["source"].as_str().unwrap().contains("hygiene.toml"));
}

#[test]
fn hygiene_rules_invalid_config_lists_all_errors() {
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();
    write_hygiene_toml(
        home.path(),
        r#"
[hygiene]
apply_ttl = "nope"

[[hygiene.rules]]
id = "bad"
entity = "issue"
[hygiene.rules.when]
status = { includes = ["Todo"] }
"#,
    );

    let (code, _, stderr) = run_hy(&server, home.path(), &["hygiene", "rules"]);
    assert_eq!(code, 1, "invalid config must exit 1");
    assert!(
        stderr.contains("apply_ttl") && stderr.contains("unknown operator 'includes'"),
        "all errors listed together (R7/R22): {stderr}"
    );
}

#[test]
fn hygiene_rules_schema_prints_field_model() {
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();
    let (code, stdout, stderr) = run_hy(
        &server,
        home.path(),
        &["hygiene", "rules", "--schema", "--output", "json"],
    );
    assert_eq!(code, 0, "schema should print: {stderr}");
    let schema = parse_json(&stdout);
    assert!(schema["entities"]["issue"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["name"] == "status"));
    assert!(schema["operators"]
        .as_array()
        .unwrap()
        .iter()
        .any(|o| o["operator"] == "missing_group"));
}

#[test]
fn hygiene_rules_init_writes_starter_and_refuses_overwrite() {
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();

    let (code, _, stderr) = run_hy(&server, home.path(), &["hygiene", "rules", "--init"]);
    assert_eq!(code, 0, "--init should write starter: {stderr}");
    let path = hygiene_config_dir(home.path()).join("hygiene.toml");
    let starter = std::fs::read_to_string(&path).expect("starter file written");
    assert!(starter.contains("wip-limit"));
    assert!(starter.contains("[hygiene.rules.fix]"));

    // The starter file must itself validate.
    let (code, _, stderr) = run_hy(&server, home.path(), &["hygiene", "rules"]);
    assert_eq!(code, 0, "starter config must validate: {stderr}");

    // Refuse to overwrite without --force (R22).
    let (code, _, stderr) = run_hy(&server, home.path(), &["hygiene", "rules", "--init"]);
    assert_eq!(code, 1);
    assert!(stderr.contains("--force"), "overwrite hint: {stderr}");
    let (code, _, _) = run_hy(
        &server,
        home.path(),
        &["hygiene", "rules", "--init", "--force"],
    );
    assert_eq!(code, 0, "--force overwrites");
}

#[test]
fn hygiene_rules_flag_loads_alternate_rules_file() {
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();
    // The default user-level config stays empty; rules come from --rules.
    let alt = home.path().join("alt-rules.toml");
    std::fs::write(&alt, HYGIENE_RULES_ALL_ENTITIES).unwrap();
    let alt_str = alt.to_str().unwrap();

    let (code, stdout, stderr) = run_hy(
        &server,
        home.path(),
        &[
            "hygiene", "check", "--rules", alt_str, "-t", "FAKE", "--output", "json",
        ],
    );
    assert_eq!(code, 0, "check with --rules should succeed: {stderr}");
    let findings = parse_json(&stdout);
    assert!(
        findings
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["rule"] == "urgent-in-backlog"),
        "findings should come from the --rules file, got: {stdout}"
    );

    // `hygiene rules` reports the override path as the source.
    let (code, stdout, stderr) = run_hy(
        &server,
        home.path(),
        &["hygiene", "rules", "--rules", alt_str, "--output", "json"],
    );
    assert_eq!(code, 0, "rules with --rules should succeed: {stderr}");
    let rules = parse_json(&stdout);
    assert!(
        rules
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["source"].as_str().unwrap().contains("alt-rules.toml")),
        "source should name the override path, got: {stdout}"
    );
}

#[test]
fn hygiene_rules_env_var_fallback_and_flag_precedence() {
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();
    let alt = home.path().join("env-rules.toml");
    std::fs::write(&alt, HYGIENE_RULES_ALL_ENTITIES).unwrap();
    let alt_str = alt.to_str().unwrap();

    // Env var alone selects the alternate file.
    let (code, stdout, stderr) = run_hy_env(
        &server,
        home.path(),
        &[("LINEAR_CLI_HYGIENE_RULES", alt_str)],
        &["hygiene", "rules", "--output", "json"],
    );
    assert_eq!(code, 0, "env var rules file should load: {stderr}");
    let rules = parse_json(&stdout);
    assert!(
        rules
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["source"].as_str().unwrap().contains("env-rules.toml")),
        "source should name the env path, got: {stdout}"
    );

    // --rules beats the env var: env points at a missing file, but the
    // flag's valid path wins so the command succeeds.
    let (code, _, stderr) = run_hy_env(
        &server,
        home.path(),
        &[("LINEAR_CLI_HYGIENE_RULES", "/nonexistent/env.toml")],
        &["hygiene", "rules", "--rules", alt_str, "--output", "json"],
    );
    assert_eq!(code, 0, "--rules should take precedence over env: {stderr}");
}

#[test]
fn hygiene_check_fetch_failure_exits_nonzero_even_with_limit() {
    // Regression guard from the live trial: output-shaping flags like --limit
    // must never swallow a fetch failure's exit code (JSON error envelope is
    // printed either way).
    let home = tempfile::tempdir().unwrap();
    write_hygiene_toml(home.path(), HYGIENE_RULES_ALL_ENTITIES);
    // Unroutable local endpoint: the fetch fails without touching the network.
    let dead = "http://127.0.0.1:1";

    let (code, _, stderr) = run_hy(
        dead,
        home.path(),
        &["hygiene", "check", "-t", "FAKE", "--output", "json"],
    );
    assert_eq!(code, 1, "fetch failure must exit 1: {stderr}");

    let (code, _, stderr) = run_hy(
        dead,
        home.path(),
        &[
            "hygiene", "check", "-t", "FAKE", "--output", "json", "--limit", "25",
        ],
    );
    assert_eq!(code, 1, "--limit must not swallow the exit code: {stderr}");
    assert!(
        stderr.contains("\"error\":true") || stderr.contains("\"error\": true"),
        "JSON error envelope expected on stderr: {stderr}"
    );
}

#[test]
fn hygiene_missing_override_rules_file_exits_one_naming_path() {
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();
    // A valid default config exists, but the override must not fall back.
    write_hygiene_toml(home.path(), HYGIENE_RULES_ALL_ENTITIES);
    let missing = home.path().join("missing-rules.toml");
    let missing_str = missing.to_str().unwrap();

    let (code, _, stderr) = run_hy(
        &server,
        home.path(),
        &["hygiene", "check", "--rules", missing_str, "-t", "FAKE"],
    );
    assert_eq!(code, 1, "missing --rules file must be exit 1: {stderr}");
    assert!(
        stderr.contains(missing_str),
        "error should name the missing path: {stderr}"
    );

    // Same for the env var.
    let (code, _, stderr) = run_hy_env(
        &server,
        home.path(),
        &[("LINEAR_CLI_HYGIENE_RULES", missing_str)],
        &["hygiene", "check", "-t", "FAKE"],
    );
    assert_eq!(code, 1, "missing env rules file must be exit 1: {stderr}");
    assert!(
        stderr.contains(missing_str) && stderr.contains("LINEAR_CLI_HYGIENE_RULES"),
        "error should name the path and the env var: {stderr}"
    );

    // An unparsable override file is also an error naming the path.
    let broken = home.path().join("broken-rules.toml");
    std::fs::write(&broken, "[hygiene]\nbogus = true\n").unwrap();
    let (code, _, stderr) = run_hy(
        &server,
        home.path(),
        &[
            "hygiene",
            "check",
            "--rules",
            broken.to_str().unwrap(),
            "-t",
            "FAKE",
        ],
    );
    assert_eq!(code, 1, "invalid --rules file must be exit 1: {stderr}");
    assert!(
        stderr.contains("broken-rules.toml"),
        "error should name the override path: {stderr}"
    );
}

#[test]
fn hygiene_rules_init_with_rules_flag_writes_to_path() {
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();
    let target = home.path().join("nested").join("starter.toml");
    let target_str = target.to_str().unwrap();

    let (code, _, stderr) = run_hy(
        &server,
        home.path(),
        &["hygiene", "rules", "--init", "--rules", target_str],
    );
    assert_eq!(code, 0, "--init --rules should write to PATH: {stderr}");
    let starter = std::fs::read_to_string(&target).expect("starter written to --rules path");
    assert!(starter.contains("wip-limit"));
    // The default location must stay untouched.
    assert!(!hygiene_config_dir(home.path())
        .join("hygiene.toml")
        .exists());

    // Refuses to overwrite the override path without --force too.
    let (code, _, stderr) = run_hy(
        &server,
        home.path(),
        &["hygiene", "rules", "--init", "--rules", target_str],
    );
    assert_eq!(code, 1);
    assert!(stderr.contains("--force"), "overwrite hint: {stderr}");
}

#[test]
fn hygiene_report_aggregates_by_group() {
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();
    write_hygiene_toml(home.path(), HYGIENE_RULES_ALL_ENTITIES);

    let (code, stdout, stderr) = run_hy(
        &server,
        home.path(),
        &["hygiene", "report", "-t", "FAKE", "--output", "json"],
    );
    assert_eq!(code, 0, "report should succeed: {stderr}");
    let by_rule = parse_json(&stdout);
    assert_eq!(by_rule["urgent-in-backlog"], 1);
    assert_eq!(by_rule["stale-in-review"], 1);

    let (code, stdout, _) = run_hy(
        &server,
        home.path(),
        &[
            "hygiene", "report", "-t", "FAKE", "--by", "entity", "--output", "json",
        ],
    );
    assert_eq!(code, 0);
    let by_entity = parse_json(&stdout);
    assert_eq!(by_entity["issue"], 3);
    assert_eq!(by_entity["project"], 1);
    assert_eq!(by_entity["initiative"], 1);

    let (code, _, stderr) = run_hy(
        &server,
        home.path(),
        &["hygiene", "report", "--by", "bogus"],
    );
    assert_eq!(code, 1);
    assert!(stderr.contains("rule, owner, team, or entity"), "{stderr}");
}

#[test]
fn hygiene_fix_dry_run_previews_without_mutating() {
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();
    write_hygiene_toml(home.path(), HYGIENE_RULES_FIXABLE);

    let (code, stdout, stderr) = run_hy(
        &server,
        home.path(),
        &[
            "hygiene",
            "fix",
            "-t",
            "FAKE",
            "--dry-run",
            "--output",
            "json",
        ],
    );
    assert_eq!(code, 0, "fix --dry-run should succeed: {stderr}");
    let payload = parse_json(&stdout);
    assert_eq!(payload["dryRun"], true);
    let results = payload["results"].as_array().unwrap();
    let status_of = |key: &str| -> String {
        results
            .iter()
            .find(|r| r["dedupeKey"] == key)
            .unwrap_or_else(|| panic!("missing result for {key}: {stdout}"))["status"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(status_of("urgent-in-backlog:FAKE-1"), "would_execute");
    assert_eq!(status_of("label-fix:FAKE-1"), "would_execute");
    assert_eq!(status_of("stale-in-review:FAKE-2"), "skipped");
    assert_eq!(status_of("needs-update:FAKE-3"), "skipped");
    assert_eq!(payload["summary"]["executed"], 0);
}

#[test]
fn hygiene_fix_yes_executes_commands_and_reports_skips() {
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();
    write_hygiene_toml(home.path(), HYGIENE_RULES_FIXABLE);

    // Without --yes in JSON/non-TTY mode the command must refuse (R24).
    let (code, _, stderr) = run_hy(
        &server,
        home.path(),
        &["hygiene", "fix", "-t", "FAKE", "--output", "json"],
    );
    assert_eq!(code, 1, "fix without --yes must refuse: {stderr}");
    assert!(stderr.contains("--yes"), "instructive error: {stderr}");

    let (code, stdout, stderr) = run_hy(
        &server,
        home.path(),
        &["hygiene", "fix", "-t", "FAKE", "--yes", "--output", "json"],
    );
    assert_eq!(code, 0, "fix --yes should succeed: {stderr}");
    let payload = parse_json(&stdout);
    let results = payload["results"].as_array().unwrap();
    let result_of = |key: &str| -> &serde_json::Value {
        results
            .iter()
            .find(|r| r["dedupeKey"] == key)
            .unwrap_or_else(|| panic!("missing result for {key}: {stdout}"))
    };
    // Command-kind fixes execute; the label fix passing proves the executor
    // merged the existing label (the mock only matches both label UUIDs).
    assert_eq!(result_of("urgent-in-backlog:FAKE-1")["status"], "executed");
    assert_eq!(result_of("label-fix:FAKE-1")["status"], "executed");
    // Ambiguous fixes are skipped with machine-readable reasons (R24).
    let stale = result_of("stale-in-review:FAKE-2");
    assert_eq!(stale["status"], "skipped");
    assert_eq!(stale["reason"], "needs_choice");
    let needs = result_of("needs-update:FAKE-3");
    assert_eq!(needs["status"], "skipped");
    assert_eq!(needs["reason"], "needs_input");
    assert_eq!(payload["summary"]["executed"], 2);
    assert_eq!(payload["summary"]["skipped"], 2);
    assert_eq!(payload["summary"]["failed"], 0);
}

#[test]
fn hygiene_apply_executes_and_is_idempotent() {
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();
    write_hygiene_toml(home.path(), HYGIENE_RULES_FIXABLE);

    let (code, _, stderr) = run_hy(
        &server,
        home.path(),
        &["hygiene", "check", "-t", "FAKE", "--output", "json", "-q"],
    );
    assert_eq!(code, 0, "check populates artifact: {stderr}");

    // Dry run resolves the mutation without executing.
    let (code, stdout, _) = run_hy(
        &server,
        home.path(),
        &[
            "hygiene",
            "apply",
            "urgent-in-backlog:FAKE-1",
            "--dry-run",
            "--output",
            "json",
        ],
    );
    assert_eq!(code, 0);
    let payload = parse_json(&stdout);
    assert_eq!(payload["results"][0]["status"], "would_apply");
    assert_eq!(
        payload["results"][0]["command"],
        "linear i update FAKE-1 -s Todo"
    );

    // Command-kind apply needs no extra flags (R25).
    let (code, stdout, stderr) = run_hy(
        &server,
        home.path(),
        &[
            "hygiene",
            "apply",
            "urgent-in-backlog:FAKE-1",
            "--output",
            "json",
        ],
    );
    assert_eq!(code, 0, "apply should succeed: {stderr}");
    assert_eq!(parse_json(&stdout)["results"][0]["status"], "applied");

    // Re-applying is an idempotent no-op (R34).
    let (code, stdout, _) = run_hy(
        &server,
        home.path(),
        &[
            "hygiene",
            "apply",
            "urgent-in-backlog:FAKE-1",
            "--output",
            "json",
        ],
    );
    assert_eq!(code, 0);
    assert_eq!(
        parse_json(&stdout)["results"][0]["status"],
        "already_resolved"
    );

    // Unknown dedupe keys are exit 2 (R27).
    let (code, _, stderr) = run_hy(
        &server,
        home.path(),
        &["hygiene", "apply", "nope:FAKE-99", "--output", "json"],
    );
    assert_eq!(code, 2, "unknown key must exit 2: {stderr}");
}

#[test]
fn hygiene_apply_option_and_input_flags() {
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();
    write_hygiene_toml(home.path(), HYGIENE_RULES_FIXABLE);
    let (code, _, stderr) = run_hy(
        &server,
        home.path(),
        &["hygiene", "check", "-t", "FAKE", "--output", "json", "-q"],
    );
    assert_eq!(code, 0, "check populates artifact: {stderr}");

    // Options-kind without --option: error listing valid actions (R25).
    let (code, _, stderr) = run_hy(
        &server,
        home.path(),
        &["hygiene", "apply", "stale-in-review:FAKE-2"],
    );
    assert_eq!(code, 1);
    assert!(
        stderr.contains("nudge_review") && stderr.contains("move_back"),
        "valid actions listed: {stderr}"
    );

    // Invalid --option: same actionable error.
    let (code, _, stderr) = run_hy(
        &server,
        home.path(),
        &[
            "hygiene",
            "apply",
            "stale-in-review:FAKE-2",
            "--option",
            "bogus",
        ],
    );
    assert_eq!(code, 1);
    assert!(stderr.contains("move_back"), "{stderr}");

    // Valid --option executes the selected action.
    let (code, stdout, stderr) = run_hy(
        &server,
        home.path(),
        &[
            "hygiene",
            "apply",
            "stale-in-review:FAKE-2",
            "--option",
            "move_back",
            "--output",
            "json",
        ],
    );
    assert_eq!(code, 0, "apply --option should succeed: {stderr}");
    assert_eq!(parse_json(&stdout)["results"][0]["status"], "applied");

    // needs_input-kind without --input: instructive error.
    let (code, _, stderr) = run_hy(
        &server,
        home.path(),
        &["hygiene", "apply", "needs-update:FAKE-3"],
    );
    assert_eq!(code, 1);
    assert!(stderr.contains("--input"), "{stderr}");

    // With --input the comment body is the authored content (the mock only
    // matches a commentCreate carrying the input text).
    let (code, stdout, stderr) = run_hy(
        &server,
        home.path(),
        &[
            "hygiene",
            "apply",
            "needs-update:FAKE-3",
            "--input",
            "Reviewed and nudged",
            "--output",
            "json",
        ],
    );
    assert_eq!(code, 0, "apply --input should succeed: {stderr}");
    assert_eq!(parse_json(&stdout)["results"][0]["status"], "applied");
}

/// Project label-group rule matching the hygiene_projects fixture; the
/// "domain" group must be declared in the repo context (.linear.toml).
const HYGIENE_RULES_PROJECT_LABEL_GROUP: &str = r#"
[[hygiene.rules]]
id = "project-missing-domain"
entity = "project"
[hygiene.rules.when]
labels = { missing_group = "domain" }
"#;

#[test]
fn hygiene_project_label_group_fix_generates_and_applies() {
    // Regression: project label-group findings advertised `fix.kind =
    // "options"` whose commands were `linear p update <uuid>` with no label
    // argument, so every `hy apply --option <label>` failed with
    // "Unsupported fix command". The full chain must work: fix generation
    // emits `-l <label>`, apply decodes it, and the executor merges with the
    // project's existing project labels.
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();
    write_hygiene_toml(home.path(), HYGIENE_RULES_PROJECT_LABEL_GROUP);
    std::fs::write(
        home.path().join(".linear.toml"),
        "[[context.label_groups]]\nkey = \"domain\"\n",
    )
    .expect("write .linear.toml");

    let (code, stdout, stderr) = run_hy(
        &server,
        home.path(),
        &["hygiene", "check", "-t", "FAKE", "--output", "json"],
    );
    assert_eq!(code, 0, "check should succeed: {stderr}");
    let findings = parse_json(&stdout);
    let finding = findings
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["dedupeKey"] == "project-missing-domain:fake-project")
        .unwrap_or_else(|| panic!("missing project finding: {stdout}"));
    assert_eq!(finding["fix"]["kind"], "options");
    let options = finding["fix"]["options"].as_array().unwrap();
    let platform = options
        .iter()
        .find(|o| o["action"] == "platform")
        .unwrap_or_else(|| panic!("missing platform option: {stdout}"));
    assert_eq!(
        platform["command"], "linear p update cccc3333-0000-4000-8000-00000000000d -l platform",
        "fix command must carry the label flag"
    );

    // Applying the option resolves the project label and merges with the
    // existing labels (the mock only matches a projectUpdate carrying both
    // project-label UUIDs).
    let (code, stdout, stderr) = run_hy(
        &server,
        home.path(),
        &[
            "hygiene",
            "apply",
            "project-missing-domain:fake-project",
            "--option",
            "platform",
            "--output",
            "json",
        ],
    );
    assert_eq!(code, 0, "apply --option should succeed: {stderr}");
    assert_eq!(parse_json(&stdout)["results"][0]["status"], "applied");
}

#[test]
fn hygiene_apply_refuses_expired_artifact() {
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();
    write_hygiene_toml(home.path(), HYGIENE_RULES_FIXABLE);
    let (code, _, stderr) = run_hy(
        &server,
        home.path(),
        &["hygiene", "check", "-t", "FAKE", "--output", "json", "-q"],
    );
    assert_eq!(code, 0, "check populates artifact: {stderr}");

    // Age the artifact past the default 30m TTL (R33).
    let artifact_path =
        find_file(home.path(), "hygiene-last-run.json").expect("check persisted the run artifact");
    let content = std::fs::read_to_string(&artifact_path).unwrap();
    let mut artifact: serde_json::Value = serde_json::from_str(&content).unwrap();
    artifact["generatedAt"] = serde_json::json!("2000-01-01T00:00:00Z");
    std::fs::write(&artifact_path, serde_json::to_string(&artifact).unwrap()).unwrap();

    let (code, _, stderr) = run_hy(
        &server,
        home.path(),
        &["hygiene", "apply", "urgent-in-backlog:FAKE-1"],
    );
    assert_eq!(code, 1, "expired artifact must be refused");
    assert!(
        stderr.contains("Re-run") && stderr.contains("hygiene check"),
        "refusal directs re-running check: {stderr}"
    );
}

#[test]
fn hygiene_snooze_suppresses_findings_until_expiry() {
    let server = start_mock_server();
    let home = tempfile::tempdir().unwrap();
    write_hygiene_toml(home.path(), HYGIENE_RULES_ALL_ENTITIES);

    let (code, _, stderr) = run_hy(
        &server,
        home.path(),
        &["hygiene", "check", "-t", "FAKE", "--output", "json", "-q"],
    );
    assert_eq!(code, 0, "{stderr}");

    // Unknown keys are exit 2 (R27).
    let (code, _, _) = run_hy(
        &server,
        home.path(),
        &["hygiene", "snooze", "nope:FAKE-99", "--for", "1h"],
    );
    assert_eq!(code, 2);

    let (code, _, stderr) = run_hy(
        &server,
        home.path(),
        &[
            "hygiene",
            "snooze",
            "urgent-in-backlog:FAKE-1",
            "--for",
            "1h",
        ],
    );
    assert_eq!(code, 0, "snooze should succeed: {stderr}");

    // Snoozed findings disappear from check output and fix/apply (R18/R26).
    let (code, stdout, _) = run_hy(
        &server,
        home.path(),
        &["hygiene", "check", "-t", "FAKE", "--output", "json"],
    );
    assert_eq!(code, 0);
    let findings = parse_json(&stdout);
    assert!(
        !findings
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["dedupeKey"] == "urgent-in-backlog:FAKE-1"),
        "snoozed finding must be suppressed: {stdout}"
    );

    let (code, stdout, _) = run_hy(
        &server,
        home.path(),
        &["hygiene", "snooze", "--list", "--output", "json"],
    );
    assert_eq!(code, 0);
    assert_eq!(
        parse_json(&stdout)[0]["dedupeKey"],
        "urgent-in-backlog:FAKE-1"
    );

    let (code, stdout, _) = run_hy(
        &server,
        home.path(),
        &["hygiene", "snooze", "--clear", "--output", "json"],
    );
    assert_eq!(code, 0);
    assert_eq!(parse_json(&stdout)["cleared"], 1);

    let (code, stdout, _) = run_hy(
        &server,
        home.path(),
        &["hygiene", "check", "-t", "FAKE", "--output", "json"],
    );
    assert_eq!(code, 0);
    assert_eq!(findings_len(&stdout), 5, "cleared snooze restores findings");
}

fn findings_len(stdout: &str) -> usize {
    parse_json(stdout).as_array().map(|a| a.len()).unwrap_or(0)
}

/// Recursively find a file by name under `dir`.
fn find_file(dir: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
    for entry in std::fs::read_dir(dir).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file(&path, name) {
                return Some(found);
            }
        } else if path.file_name().and_then(|n| n.to_str()) == Some(name) {
            return Some(path);
        }
    }
    None
}
