use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::output::print_json_owned;
use crate::{AgentOptions, OutputOptions};

const RELEASE_API_URL: &str = "https://api.github.com/repos/Finesssee/linear-cli/releases/latest";
const UPDATE_CHECK_INTERVAL_SECONDS: u64 = 24 * 60 * 60;

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    draft: bool,
    prerelease: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct UpdateState {
    last_checked_at: Option<u64>,
    last_seen_latest_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UpdateStatus {
    current_version: String,
    latest_version: Option<String>,
    release_url: Option<String>,
    update_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ParsedVersion {
    major: u64,
    minor: u64,
    patch: u64,
}

pub async fn handle(check: bool, output: &OutputOptions, _agent_opts: AgentOptions) -> Result<()> {
    let mut state = load_update_state().unwrap_or_default();
    let status = fetch_update_status().await?;
    record_successful_check(&mut state, &status);
    save_update_state(&state)?;

    if !check && !output.is_json() && !output.has_template() && status.update_available {
        println!(
            "A newer linear-cli release is available ({} -> {}).",
            status.current_version,
            status.latest_version.as_deref().unwrap_or("unknown")
        );
        println!("Update it with your package manager, flake input, or a reviewed source rebuild.");
        if let Some(url) = status.release_url.as_deref() {
            println!("Latest release: {}", url);
        }
        return Ok(());
    }

    print_update_status(output, &status)?;
    Ok(())
}

pub async fn maybe_warn_for_update() -> Result<()> {
    let mut state = load_update_state().unwrap_or_default();
    if !should_check_now(&state, now_unix_seconds()) {
        return Ok(());
    }

    let status = match fetch_update_status().await {
        Ok(status) => status,
        Err(_) => return Ok(()),
    };

    record_successful_check(&mut state, &status);
    let _ = save_update_state(&state);

    if status.update_available {
        eprintln!(
            "Warning: a newer linear-cli release is available ({} -> {}). Review and update via your package manager or flake; no in-place self-update is performed.",
            status.current_version,
            status.latest_version.as_deref().unwrap_or("unknown")
        );
    }

    Ok(())
}

async fn fetch_update_status() -> Result<UpdateStatus> {
    let current_version = current_version_tag();
    let release = fetch_latest_release().await?;
    let latest_version = release.tag_name;
    let update_available = is_newer_version(&latest_version, &current_version);

    Ok(UpdateStatus {
        current_version,
        latest_version: Some(latest_version),
        release_url: Some(release.html_url),
        update_available,
    })
}

async fn fetch_latest_release() -> Result<GitHubRelease> {
    let client = Client::builder()
        .user_agent(format!("linear-cli/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .context("Failed to build update-check HTTP client")?;

    let release = client
        .get(RELEASE_API_URL)
        .send()
        .await
        .context("Failed to check for the latest release")?
        .error_for_status()
        .context("Latest release check returned an error")?
        .json::<GitHubRelease>()
        .await
        .context("Failed to parse latest release response")?;

    if release.draft || release.prerelease {
        anyhow::bail!("Latest release is not a stable published release");
    }

    Ok(release)
}

fn print_update_status(output: &OutputOptions, status: &UpdateStatus) -> Result<()> {
    if output.is_json() || output.has_template() {
        print_json_owned(
            json!({
                "current_version": status.current_version,
                "latest_version": status.latest_version,
                "release_url": status.release_url,
                "update_available": status.update_available,
                "self_update_supported": false,
            }),
            output,
        )?;
        return Ok(());
    }

    match status.latest_version.as_deref() {
        Some(latest) if status.update_available => {
            println!(
                "linear-cli {} is installed. {} is available.",
                status.current_version, latest
            );
            println!("Update manually via your package manager, flake input, or a reviewed source rebuild.");
            if let Some(url) = status.release_url.as_deref() {
                println!("Latest release: {}", url);
            }
        }
        _ => {
            println!("linear-cli {} is up to date.", status.current_version);
        }
    }

    Ok(())
}

fn record_successful_check(state: &mut UpdateState, status: &UpdateStatus) {
    state.last_checked_at = Some(now_unix_seconds());
    state.last_seen_latest_version = status.latest_version.clone();
}

fn should_check_now(state: &UpdateState, now: u64) -> bool {
    match state.last_checked_at {
        Some(last_checked_at) => {
            now.saturating_sub(last_checked_at) >= UPDATE_CHECK_INTERVAL_SECONDS
        }
        None => true,
    }
}

fn load_update_state() -> Result<UpdateState> {
    let path = update_state_path()?;
    if !path.exists() {
        return Ok(UpdateState::default());
    }

    let contents = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read update state at {}", path.display()))?;
    let state = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse update state at {}", path.display()))?;
    Ok(state)
}

fn save_update_state(state: &UpdateState) -> Result<()> {
    let path = update_state_path()?;
    let dir = path
        .parent()
        .context("Update state path has no parent directory")?;
    fs::create_dir_all(dir)?;

    let content = serde_json::to_string_pretty(state)?;
    let temp_path = path.with_extension("tmp");

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&temp_path)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
    }

    #[cfg(not(unix))]
    {
        let mut file = fs::File::create(&temp_path)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
    }

    fs::rename(&temp_path, &path)?;
    Ok(())
}

fn update_state_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .context("Could not find config directory")?
        .join("linear-cli");
    Ok(config_dir.join("update.json"))
}

fn current_version_tag() -> String {
    format!("v{}", env!("CARGO_PKG_VERSION"))
}

fn parse_version(tag: &str) -> Option<ParsedVersion> {
    let normalized = tag.trim().trim_start_matches('v');
    let mut parts = normalized.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }

    Some(ParsedVersion {
        major,
        minor,
        patch,
    })
}

fn is_newer_version(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(latest), Some(current)) => latest > current,
        _ => false,
    }
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version_strips_v_prefix() {
        assert_eq!(
            parse_version("v0.3.15"),
            Some(ParsedVersion {
                major: 0,
                minor: 3,
                patch: 15,
            })
        );
    }

    #[test]
    fn test_parse_version_rejects_malformed_tags() {
        assert_eq!(parse_version("latest"), None);
        assert_eq!(parse_version("v0.3"), None);
        assert_eq!(parse_version("v0.3.15-beta.1"), None);
    }

    #[test]
    fn test_is_newer_version_handles_plain_and_prefixed_versions() {
        assert!(is_newer_version("v0.3.15", "0.3.14"));
        assert!(!is_newer_version("v0.3.14", "0.3.14"));
        assert!(!is_newer_version("v0.3.13", "0.3.14"));
    }

    #[test]
    fn test_should_check_now_after_interval() {
        let now = 200_000;
        let state = UpdateState {
            last_checked_at: Some(now - UPDATE_CHECK_INTERVAL_SECONDS - 1),
            last_seen_latest_version: None,
        };

        assert!(should_check_now(&state, now));
    }

    #[test]
    fn test_should_not_check_again_within_interval() {
        let now = 200_000;
        let state = UpdateState {
            last_checked_at: Some(now - 60),
            last_seen_latest_version: None,
        };

        assert!(!should_check_now(&state, now));
    }

    #[test]
    fn test_record_successful_check_tracks_latest_version() {
        let mut state = UpdateState {
            last_checked_at: None,
            last_seen_latest_version: Some("v0.3.14".to_string()),
        };
        let status = UpdateStatus {
            current_version: "v0.3.14".to_string(),
            latest_version: Some("v0.3.15".to_string()),
            release_url: None,
            update_available: true,
        };

        record_successful_check(&mut state, &status);

        assert_eq!(state.last_seen_latest_version.as_deref(), Some("v0.3.15"));
        assert!(state.last_checked_at.is_some());
    }
}
