use anyhow::{Context, Result};
use gix::remote::Direction;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;

use crate::output::print_json_owned;
use crate::{AgentOptions, OutputOptions};

const RELEASE_REMOTE_URL: &str = "https://github.com/averagechris/linear-cli.git";
const RELEASE_REFS_URL: &str = "https://github.com/averagechris/linear-cli/tags";
const RELEASE_SOURCE: &str = "github-tags";
const TAG_REFSPEC: &str = "refs/tags/*:refs/tags/*";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct UpdateStatus {
    current_version: String,
    latest_version: Option<String>,
    release_url: &'static str,
    update_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ParsedVersion {
    major: u64,
    minor: u64,
    patch: u64,
}

pub async fn handle(check: bool, output: &OutputOptions, _agent_opts: AgentOptions) -> Result<()> {
    let status = fetch_update_status()?;

    if !check && !output.is_json() && !output.has_template() && status.update_available {
        println!(
            "A newer linear release tag is available on GitHub ({} -> {}).",
            status.current_version,
            status.latest_version.as_deref().unwrap_or("unknown")
        );
        println!("Review it and update via your package manager, flake input, or a reviewed source rebuild.");
        println!("Remote tags: {}", status.release_url);
        return Ok(());
    }

    print_update_status(output, &status)?;
    Ok(())
}

fn fetch_update_status() -> Result<UpdateStatus> {
    let current_version = current_version_tag();
    let remote_tags = remote_tags(RELEASE_REMOTE_URL)?;
    Ok(update_status_from_tags(current_version, &remote_tags))
}

fn update_status_from_tags(current_version: String, remote_tags: &[String]) -> UpdateStatus {
    let latest_version = latest_release_tag(remote_tags);
    let update_available = latest_version
        .as_deref()
        .is_some_and(|latest| is_newer_version(latest, &current_version));

    UpdateStatus {
        current_version,
        latest_version,
        release_url: RELEASE_REFS_URL,
        update_available,
    }
}

fn remote_tags(remote_url: &str) -> Result<Vec<String>> {
    let tempdir = tempfile::tempdir().context("Failed to create temporary repository")?;
    let repo =
        gix::init_bare(tempdir.path()).context("Failed to initialize temporary git repository")?;
    let remote = repo
        .remote_at_without_url_rewrite(remote_url)
        .context("Failed to prepare GitHub release remote")?
        .with_refspecs([TAG_REFSPEC], Direction::Fetch)
        .context("Failed to configure tag refspec for release remote")?;
    let connection = remote
        .connect(Direction::Fetch)
        .context("Failed to connect to GitHub release remote")?;
    let (ref_map, _) = connection
        .ref_map(gix::progress::Discard, Default::default())
        .context("Failed to list remote tags from GitHub")?;

    let mut tags = BTreeSet::new();
    for remote_ref in ref_map.remote_refs {
        let (full_name, _, _) = remote_ref.unpack();
        let name = String::from_utf8_lossy(full_name.as_ref());
        if let Some(tag_name) = name.strip_prefix("refs/tags/") {
            tags.insert(tag_name.to_string());
        }
    }

    Ok(tags.into_iter().collect())
}

fn latest_release_tag(tags: &[String]) -> Option<String> {
    tags.iter()
        .filter_map(|tag| parse_version(tag).map(|version| (version, tag)))
        .max_by(|(left, _), (right, _)| left.cmp(right))
        .map(|(_, tag)| tag.clone())
}

fn print_update_status(output: &OutputOptions, status: &UpdateStatus) -> Result<()> {
    if output.is_json() || output.has_template() {
        print_json_owned(update_status_json(status), output)?;
        return Ok(());
    }

    match status.latest_version.as_deref() {
        Some(latest) if status.update_available => {
            println!(
                "linear {} is installed. {} is available on GitHub.",
                status.current_version, latest
            );
            println!(
                "Update manually via your package manager, flake input, or a reviewed source rebuild."
            );
            println!("Remote tags: {}", status.release_url);
        }
        Some(latest) => {
            println!(
                "linear {} is up to date with GitHub tag {}.",
                status.current_version, latest
            );
        }
        None => {
            println!(
                "linear {} is installed. No semver release tags were found on {}.",
                status.current_version, status.release_url
            );
        }
    }

    Ok(())
}

fn update_status_json(status: &UpdateStatus) -> Value {
    json!({
        "current_version": status.current_version,
        "latest_version": status.latest_version,
        "release_url": status.release_url,
        "update_available": status.update_available,
        "self_update_supported": false,
        "release_source": RELEASE_SOURCE,
    })
}

fn current_version_tag() -> String {
    format!("v{}", env!("CARGO_PKG_VERSION"))
}

fn parse_version(tag: &str) -> Option<ParsedVersion> {
    let normalized = tag.trim().strip_prefix('v')?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version_requires_v_prefix() {
        assert_eq!(
            parse_version("v0.3.15"),
            Some(ParsedVersion {
                major: 0,
                minor: 3,
                patch: 15,
            })
        );
        assert_eq!(parse_version("0.3.15"), None);
    }

    #[test]
    fn test_parse_version_rejects_malformed_tags() {
        assert_eq!(parse_version("latest"), None);
        assert_eq!(parse_version("v0.3"), None);
        assert_eq!(parse_version("v0.3.15-beta.1"), None);
    }

    #[test]
    fn test_is_newer_version_handles_semver_tags() {
        assert!(is_newer_version("v0.3.15", "v0.3.14"));
        assert!(!is_newer_version("v0.3.14", "v0.3.14"));
        assert!(!is_newer_version("v0.3.13", "v0.3.14"));
    }

    #[test]
    fn test_latest_release_tag_ignores_non_semver_tags() {
        let tags = vec![
            "main".to_string(),
            "v0.3.14".to_string(),
            "release-candidate".to_string(),
            "v0.3.16".to_string(),
        ];

        assert_eq!(latest_release_tag(&tags).as_deref(), Some("v0.3.16"));
    }

    #[test]
    fn test_serialized_update_status_shape() {
        let status = UpdateStatus {
            current_version: "v1.2.8".to_string(),
            latest_version: Some("v1.2.9".to_string()),
            release_url: RELEASE_REFS_URL,
            update_available: true,
        };

        assert_eq!(
            update_status_json(&status),
            json!({
                "current_version": "v1.2.8",
                "latest_version": "v1.2.9",
                "release_url": RELEASE_REFS_URL,
                "update_available": true,
                "self_update_supported": false,
                "release_source": "github-tags",
            })
        );
    }

    #[test]
    fn test_update_status_from_remote_tags_without_network() {
        let tags = vec!["v1.2.7".to_string(), "v1.2.9".to_string()];
        let status = update_status_from_tags("v1.2.8".to_string(), &tags);

        assert_eq!(status.current_version, "v1.2.8");
        assert_eq!(status.latest_version.as_deref(), Some("v1.2.9"));
        assert_eq!(status.release_url, RELEASE_REFS_URL);
        assert!(status.update_available);
    }
}
