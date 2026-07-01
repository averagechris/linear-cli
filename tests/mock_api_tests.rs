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
        .map(|rule| MockRule {
            matches: rule["match"]
                .as_array()
                .expect("rule has match array")
                .iter()
                .map(|m| m.as_str().expect("match entries are strings").to_string())
                .collect(),
            response: rule["response"].to_string(),
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
