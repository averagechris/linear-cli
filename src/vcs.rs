use anyhow::Result;
use std::sync::OnceLock;

/// Extract a Linear issue identifier from a branch name string.
///
/// Returns `Some("ENG-123")` (uppercased) if found, `None` otherwise.
/// This is the pure-logic core used by [`detect_current_issue`].
pub fn extract_issue_from_branch(branch: &str) -> Option<String> {
    static ISSUE_RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = ISSUE_RE.get_or_init(|| regex::Regex::new(r"(?i)([a-z]+-\d+)").unwrap());

    re.find(branch).map(|m| m.as_str().to_uppercase())
}

/// Detect the current Linear issue identifier from the git branch name.
///
/// Returns `Ok(identifier)` (e.g. "ENG-123") if found, or an error if not
/// in a git repo or no issue ID is present in the branch name.
pub fn detect_current_issue() -> Result<String> {
    let branch = run_git_command(&["rev-parse", "--abbrev-ref", "HEAD"])?;

    extract_issue_from_branch(&branch).ok_or_else(|| {
        anyhow::anyhow!(
            "No Linear issue ID found in branch '{}'. Provide an issue ID explicitly.",
            branch
        )
    })
}

/// Resolve an optional issue ID: use the provided value, or fall back to
/// detecting the current issue from the git branch.
pub fn resolve_issue_id(id: Option<String>) -> Result<String> {
    match id {
        Some(id) if !id.is_empty() => Ok(id),
        _ => detect_current_issue(),
    }
}

pub fn run_git_command(args: &[&str]) -> Result<String> {
    let output = crate::process::scrubbed_command("git")
        .args(args)
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Git command failed: {}", stderr.trim());
    }
}

pub fn validate_branch_name(branch: &str) -> Result<()> {
    if branch.trim().is_empty() {
        anyhow::bail!("Branch name cannot be empty");
    }
    if branch.starts_with('-') {
        anyhow::bail!("Branch name cannot start with '-'");
    }
    if branch == "@" || branch.contains("@{") {
        anyhow::bail!("Branch name contains invalid ref syntax");
    }

    let output = crate::process::scrubbed_command("git")
        .args(["check-ref-format", "--branch", branch])
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Invalid branch name '{}': {}", branch, stderr.trim());
    }
}

pub fn git_branch_exists(branch: &str) -> bool {
    if validate_branch_name(branch).is_err() {
        return false;
    }

    let ref_name = format!("refs/heads/{}", branch);
    crate::process::scrubbed_command("git")
        .args(["show-ref", "--verify", "--quiet", &ref_name])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn generate_branch_name(identifier: &str, title: &str) -> String {
    const MAX_SLUG_CHARS: usize = 50;

    // Convert title to kebab-case for branch name
    let slug: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    let slug: String = slug
        .chars()
        .take(MAX_SLUG_CHARS)
        .collect::<String>()
        .trim_end_matches('-')
        .to_string();
    let slug = if slug.is_empty() { "update" } else { &slug };

    format!("{}/{}", identifier.to_lowercase(), slug)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_branch_slugs_truncate_on_character_boundaries() {
        let branch = generate_branch_name("LIN-1", &"界".repeat(60));
        assert_eq!(branch, format!("lin-1/{}", "界".repeat(50)));
    }

    #[test]
    fn punctuation_only_titles_use_a_nonempty_slug() {
        let branch = generate_branch_name("LIN-2", "!!! --- ???");
        assert_eq!(branch, "lin-2/update");
    }

    #[test]
    fn truncation_does_not_leave_a_trailing_separator() {
        let branch = generate_branch_name("LIN-3", &format!("{} b", "a".repeat(49)));
        assert_eq!(branch, format!("lin-3/{}", "a".repeat(49)));
    }

    // --- extract_issue_from_branch ---

    #[test]
    fn test_extract_simple_branch() {
        assert_eq!(
            extract_issue_from_branch("eng-123-fix-bug"),
            Some("ENG-123".to_string())
        );
    }

    #[test]
    fn test_extract_with_prefix_path() {
        assert_eq!(
            extract_issue_from_branch("feature/LIN-456-new-feature"),
            Some("LIN-456".to_string())
        );
    }

    #[test]
    fn test_extract_uppercase_branch() {
        assert_eq!(
            extract_issue_from_branch("SCW-789-some-task"),
            Some("SCW-789".to_string())
        );
    }

    #[test]
    fn test_extract_mixed_case() {
        assert_eq!(
            extract_issue_from_branch("Fix/scw-42-thing"),
            Some("SCW-42".to_string())
        );
    }

    #[test]
    fn test_extract_no_issue_id() {
        assert_eq!(extract_issue_from_branch("main"), None);
    }

    #[test]
    fn test_extract_no_issue_id_develop() {
        assert_eq!(extract_issue_from_branch("develop"), None);
    }

    #[test]
    fn test_extract_word_prefix_matches() {
        // "fix-123" matches because "fix" is a valid [a-z]+ prefix
        assert_eq!(
            extract_issue_from_branch("fix-123"),
            Some("FIX-123".to_string())
        );
    }

    #[test]
    fn test_extract_bare_numbers_no_match() {
        // Pure numbers without a letter prefix shouldn't match
        assert_eq!(extract_issue_from_branch("123-fix"), None);
    }

    #[test]
    fn test_extract_multiple_ids_takes_first() {
        assert_eq!(
            extract_issue_from_branch("eng-1-and-eng-2"),
            Some("ENG-1".to_string())
        );
    }

    #[test]
    fn test_extract_nested_path() {
        assert_eq!(
            extract_issue_from_branch("user/chris/abc-99-wip"),
            Some("ABC-99".to_string())
        );
    }

    #[test]
    fn test_extract_empty_branch() {
        assert_eq!(extract_issue_from_branch(""), None);
    }

    // --- resolve_issue_id ---

    #[test]
    fn test_resolve_with_explicit_id() {
        assert_eq!(
            resolve_issue_id(Some("ENG-123".to_string())).unwrap(),
            "ENG-123"
        );
    }

    #[test]
    fn test_resolve_empty_string_falls_back() {
        // Empty string should trigger fallback (which will fail outside a git repo,
        // but the logic of "empty means fallback" is what we're testing)
        let result = resolve_issue_id(Some(String::new()));
        assert!(result.is_err()); // no git repo in test context
    }

    #[test]
    fn test_resolve_none_falls_back() {
        let result = resolve_issue_id(None);
        assert!(result.is_err()); // no git repo in test context
    }
}
