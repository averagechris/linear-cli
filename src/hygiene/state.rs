//! Persistent hygiene state: the last-run artifact (R32–R34) and snoozes
//! (R26). Files live in the profile- and auth-scoped state directory and are
//! written atomically (temp + rename), following the cache/templates
//! precedent. Pure logic (TTL checks, resolution marking, snooze expiry) is
//! separated from I/O so it can be unit-tested without touching the real
//! state directory.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::hygiene::engine::Finding;

/// Last-run artifact filename inside the state dir.
pub const ARTIFACT_FILE: &str = "hygiene-last-run.json";
/// Snooze state filename inside the state dir.
pub const SNOOZE_FILE: &str = "hygiene-snoozes.json";

const ARTIFACT_VERSION: u32 = 1;

/// A finding as stored in the last-run artifact, with its resolution flag
/// (set by successful `fix`/`apply`, R34).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredFinding {
    #[serde(flatten)]
    pub finding: Finding,
    #[serde(default)]
    pub resolved: bool,
}

/// Outcome of marking a finding resolved (R34).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkOutcome {
    Resolved,
    AlreadyResolved,
    NotFound,
}

/// The last-run artifact persisted by `hygiene check` (R32). Each check
/// replaces it wholesale.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunArtifact {
    pub version: u32,
    pub generated_at: DateTime<Utc>,
    /// Description of the scope the check ran over (teams, flags, …) for
    /// display; the engine does not interpret it.
    pub scope: serde_json::Value,
    pub findings: Vec<StoredFinding>,
}

impl RunArtifact {
    pub fn new(scope: serde_json::Value, findings: Vec<Finding>, now: DateTime<Utc>) -> Self {
        RunArtifact {
            version: ARTIFACT_VERSION,
            generated_at: now,
            scope,
            findings: findings
                .into_iter()
                .map(|finding| StoredFinding {
                    finding,
                    resolved: false,
                })
                .collect(),
        }
    }

    pub fn age_seconds(&self, now: DateTime<Utc>) -> u64 {
        (now - self.generated_at).num_seconds().max(0) as u64
    }

    /// `apply` refuses artifacts older than the TTL (R33).
    pub fn is_expired(&self, ttl_seconds: u64, now: DateTime<Utc>) -> bool {
        self.age_seconds(now) > ttl_seconds
    }

    pub fn find(&self, dedupe_key: &str) -> Option<&StoredFinding> {
        self.findings
            .iter()
            .find(|f| f.finding.dedupe_key == dedupe_key)
    }

    /// Mark a finding resolved; repeated calls are idempotent (R34).
    pub fn mark_resolved(&mut self, dedupe_key: &str) -> MarkOutcome {
        match self
            .findings
            .iter_mut()
            .find(|f| f.finding.dedupe_key == dedupe_key)
        {
            None => MarkOutcome::NotFound,
            Some(stored) if stored.resolved => MarkOutcome::AlreadyResolved,
            Some(stored) => {
                stored.resolved = true;
                MarkOutcome::Resolved
            }
        }
    }

    /// Findings not yet resolved by `fix`/`apply`. Part of the artifact API;
    /// currently exercised by unit tests.
    #[allow(dead_code)]
    pub fn unresolved(&self) -> impl Iterator<Item = &StoredFinding> {
        self.findings.iter().filter(|f| !f.resolved)
    }
}

/// Snooze state: dedupe key → expiry (R26). Per-machine, profile-scoped.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SnoozeState {
    #[serde(default)]
    pub entries: BTreeMap<String, DateTime<Utc>>,
}

impl SnoozeState {
    /// Snooze a key until `until` (replaces any existing entry).
    pub fn snooze(&mut self, dedupe_key: &str, until: DateTime<Utc>) {
        self.entries.insert(dedupe_key.to_string(), until);
    }

    /// Remove one entry; returns whether it existed. Part of the snooze API;
    /// currently exercised by unit tests.
    #[allow(dead_code)]
    pub fn remove(&mut self, dedupe_key: &str) -> bool {
        self.entries.remove(dedupe_key).is_some()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Whether a key has an unexpired snooze. Part of the snooze API;
    /// currently exercised by unit tests.
    #[allow(dead_code)]
    pub fn is_snoozed(&self, dedupe_key: &str, now: DateTime<Utc>) -> bool {
        self.entries
            .get(dedupe_key)
            .map(|until| *until > now)
            .unwrap_or(false)
    }

    /// Keys with unexpired snoozes, for filtering findings (R18).
    pub fn active_keys(&self, now: DateTime<Utc>) -> BTreeSet<String> {
        self.entries
            .iter()
            .filter(|(_, until)| **until > now)
            .map(|(key, _)| key.clone())
            .collect()
    }

    /// Active `(key, expiry)` pairs sorted by key, for `snooze --list`.
    pub fn list_active(&self, now: DateTime<Utc>) -> Vec<(String, DateTime<Utc>)> {
        self.entries
            .iter()
            .filter(|(_, until)| **until > now)
            .map(|(key, until)| (key.clone(), *until))
            .collect()
    }

    /// Drop expired entries; returns how many were removed.
    pub fn prune_expired(&mut self, now: DateTime<Utc>) -> usize {
        let before = self.entries.len();
        self.entries.retain(|_, until| *until > now);
        before - self.entries.len()
    }
}

// ---------------------------------------------------------------------------
// I/O
// ---------------------------------------------------------------------------

/// The default state directory (profile- and auth-scoped).
pub fn default_state_dir() -> Result<PathBuf> {
    crate::cache::hygiene_state_dir()
}

fn write_atomic(path: &Path, content: &str) -> Result<()> {
    let dir = path
        .parent()
        .context("state path has no parent directory")?;
    fs::create_dir_all(dir)?;
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

    #[cfg(windows)]
    {
        let _ = fs::remove_file(path);
    }
    fs::rename(&temp_path, path)
        .with_context(|| format!("failed to atomically update {}", path.display()))?;
    Ok(())
}

/// Load the last-run artifact from an explicit state dir (tests) or use
/// [`load_artifact`] for the default location.
pub fn load_artifact_from(state_dir: &Path) -> Result<Option<RunArtifact>> {
    let path = state_dir.join(ARTIFACT_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let content =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let artifact: RunArtifact = serde_json::from_str(&content)
        .with_context(|| format!("invalid hygiene artifact {}", path.display()))?;
    Ok(Some(artifact))
}

/// Persist the last-run artifact atomically, replacing any previous one (R32).
pub fn save_artifact_to(state_dir: &Path, artifact: &RunArtifact) -> Result<()> {
    let content = serde_json::to_string_pretty(artifact)?;
    write_atomic(&state_dir.join(ARTIFACT_FILE), &content)
}

pub fn load_artifact() -> Result<Option<RunArtifact>> {
    load_artifact_from(&default_state_dir()?)
}

pub fn save_artifact(artifact: &RunArtifact) -> Result<()> {
    save_artifact_to(&default_state_dir()?, artifact)
}

/// Load snooze state (empty when the file does not exist).
pub fn load_snoozes_from(state_dir: &Path) -> Result<SnoozeState> {
    let path = state_dir.join(SNOOZE_FILE);
    if !path.exists() {
        return Ok(SnoozeState::default());
    }
    let content =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let state: SnoozeState = serde_json::from_str(&content)
        .with_context(|| format!("invalid hygiene snooze state {}", path.display()))?;
    Ok(state)
}

pub fn save_snoozes_to(state_dir: &Path, state: &SnoozeState) -> Result<()> {
    let content = serde_json::to_string_pretty(state)?;
    write_atomic(&state_dir.join(SNOOZE_FILE), &content)
}

pub fn load_snoozes() -> Result<SnoozeState> {
    load_snoozes_from(&default_state_dir()?)
}

pub fn save_snoozes(state: &SnoozeState) -> Result<()> {
    save_snoozes_to(&default_state_dir()?, state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hygiene::config::Severity;
    use crate::hygiene::engine::{FindingEntity, Fix};
    use crate::hygiene::model::EntityKind;
    use chrono::TimeZone;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 2, 12, 0, 0).unwrap()
    }

    fn finding(dedupe_key: &str) -> Finding {
        Finding {
            dedupe_key: dedupe_key.to_string(),
            rule: dedupe_key.split(':').next().unwrap().to_string(),
            severity: Severity::High,
            entity: FindingEntity {
                entity_type: EntityKind::Issue,
                id: "uuid".into(),
                identifier: "ENG-1".into(),
                title: "T".into(),
                url: None,
            },
            owner: None,
            summary: "summary".into(),
            evidence: serde_json::json!({ "status": "In Review" }),
            fix: Some(Fix::Command {
                command: "linear i update ENG-1 -s Todo".into(),
            }),
        }
    }

    #[test]
    fn artifact_new_and_ttl() {
        let artifact = RunArtifact::new(
            serde_json::json!({ "teams": ["ENG"] }),
            vec![finding("r:ENG-1")],
            now(),
        );
        assert_eq!(artifact.version, 1);
        assert_eq!(artifact.findings.len(), 1);
        assert!(!artifact.findings[0].resolved);

        // Within TTL.
        let later = now() + chrono::Duration::minutes(29);
        assert!(!artifact.is_expired(30 * 60, later));
        // Beyond TTL (R33).
        let much_later = now() + chrono::Duration::minutes(31);
        assert!(artifact.is_expired(30 * 60, much_later));
        assert_eq!(artifact.age_seconds(much_later), 31 * 60);
        // Clock skew: negative age clamps to zero.
        assert_eq!(artifact.age_seconds(now() - chrono::Duration::hours(1)), 0);
    }

    #[test]
    fn artifact_mark_resolved_is_idempotent() {
        let mut artifact = RunArtifact::new(
            serde_json::Value::Null,
            vec![finding("r:ENG-1"), finding("r:ENG-2")],
            now(),
        );
        assert_eq!(artifact.mark_resolved("r:ENG-1"), MarkOutcome::Resolved);
        assert_eq!(
            artifact.mark_resolved("r:ENG-1"),
            MarkOutcome::AlreadyResolved
        );
        assert_eq!(artifact.mark_resolved("nope:X-1"), MarkOutcome::NotFound);
        let unresolved: Vec<&str> = artifact
            .unresolved()
            .map(|f| f.finding.dedupe_key.as_str())
            .collect();
        assert_eq!(unresolved, vec!["r:ENG-2"]);
    }

    #[test]
    fn artifact_roundtrip_with_camel_case_fields() {
        let temp = tempfile::tempdir().unwrap();
        let mut artifact = RunArtifact::new(
            serde_json::json!({ "teams": ["ENG"] }),
            vec![finding("stale:ENG-1")],
            now(),
        );
        artifact.mark_resolved("stale:ENG-1");
        save_artifact_to(temp.path(), &artifact).unwrap();

        let raw = std::fs::read_to_string(temp.path().join(ARTIFACT_FILE)).unwrap();
        assert!(raw.contains("\"dedupeKey\""));
        assert!(raw.contains("\"generatedAt\""));
        assert!(raw.contains("\"resolved\": true"));

        let loaded = load_artifact_from(temp.path()).unwrap().unwrap();
        assert_eq!(loaded, artifact);
        assert_eq!(
            loaded.find("stale:ENG-1").unwrap().finding.summary,
            "summary"
        );
    }

    #[test]
    fn artifact_save_replaces_previous_run() {
        let temp = tempfile::tempdir().unwrap();
        let first = RunArtifact::new(serde_json::Value::Null, vec![finding("a:X-1")], now());
        save_artifact_to(temp.path(), &first).unwrap();
        let second = RunArtifact::new(
            serde_json::Value::Null,
            vec![finding("b:Y-1")],
            now() + chrono::Duration::minutes(5),
        );
        save_artifact_to(temp.path(), &second).unwrap();

        let loaded = load_artifact_from(temp.path()).unwrap().unwrap();
        assert_eq!(loaded.findings.len(), 1);
        assert!(loaded.find("a:X-1").is_none());
        assert!(loaded.find("b:Y-1").is_some());
    }

    #[test]
    fn artifact_missing_file_is_none() {
        let temp = tempfile::tempdir().unwrap();
        assert!(load_artifact_from(temp.path()).unwrap().is_none());
    }

    #[test]
    fn snooze_expiry_and_active_keys() {
        let mut state = SnoozeState::default();
        state.snooze("r:ENG-1", now() + chrono::Duration::hours(1));
        state.snooze("r:ENG-2", now() - chrono::Duration::hours(1));

        assert!(state.is_snoozed("r:ENG-1", now()));
        assert!(!state.is_snoozed("r:ENG-2", now())); // expired
        assert!(!state.is_snoozed("r:ENG-3", now())); // never snoozed

        let active = state.active_keys(now());
        assert!(active.contains("r:ENG-1"));
        assert!(!active.contains("r:ENG-2"));

        let listed = state.list_active(now());
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].0, "r:ENG-1");

        // Expiry boundary: exactly-now is expired.
        state.snooze("r:ENG-4", now());
        assert!(!state.is_snoozed("r:ENG-4", now()));

        assert_eq!(state.prune_expired(now()), 2);
        assert_eq!(state.entries.len(), 1);
    }

    #[test]
    fn snooze_remove_and_clear() {
        let mut state = SnoozeState::default();
        state.snooze("r:ENG-1", now() + chrono::Duration::hours(1));
        assert!(state.remove("r:ENG-1"));
        assert!(!state.remove("r:ENG-1"));
        state.snooze("r:ENG-2", now() + chrono::Duration::hours(1));
        state.clear();
        assert!(state.entries.is_empty());
    }

    #[test]
    fn snooze_roundtrip_and_missing_file_default() {
        let temp = tempfile::tempdir().unwrap();
        assert_eq!(
            load_snoozes_from(temp.path()).unwrap(),
            SnoozeState::default()
        );

        let mut state = SnoozeState::default();
        state.snooze("r:ENG-1", now() + chrono::Duration::days(1));
        save_snoozes_to(temp.path(), &state).unwrap();
        let loaded = load_snoozes_from(temp.path()).unwrap();
        assert_eq!(loaded, state);
    }

    #[cfg(unix)]
    #[test]
    fn state_files_written_with_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let artifact = RunArtifact::new(serde_json::Value::Null, vec![], now());
        save_artifact_to(temp.path(), &artifact).unwrap();
        let mode = std::fs::metadata(temp.path().join(ARTIFACT_FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
