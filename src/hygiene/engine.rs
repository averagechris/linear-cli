//! Pure rule evaluation: predicates (§4.2), builtins (§4.3), finding
//! construction (§3.3/§4.4), fix derivation (R30/R31, §4.5), and
//! deterministic sorting (R18).
//!
//! The engine performs no I/O. Wall-clock time, label-group candidates, and
//! field candidates (for auto-derived fixes) are all supplied by the caller.

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

use crate::hygiene::config::{
    BuiltinParams, BuiltinRule, Condition, FixConfig, FixOptionConfig, Predicate, Rule,
    ScopeConfig, SetValue, Severity, WhenRule,
};
use crate::hygiene::model::{settable_field_flag, EntityKind, EntityModel, FieldValue, OwnerRef};

/// Inputs the engine needs beyond rules and entities. Everything is passed in
/// so evaluation stays pure and testable (R37).
#[derive(Debug, Clone, Copy)]
pub struct EvalInputs<'a> {
    /// Wall-clock "now" used for all staleness math (R12).
    pub now: DateTime<Utc>,
    /// Label-group name → candidate label names, resolved by the commands
    /// layer from the context label-group config + option caches.
    pub label_groups: &'a BTreeMap<String, Vec<String>>,
    /// Field name → valid candidate values (statuses, priorities, …) used for
    /// auto-derived fixes on missing-field predicates (R30).
    pub field_candidates: &'a BTreeMap<String, Vec<String>>,
}

/// Entity reference embedded in a finding (§4.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindingEntity {
    #[serde(rename = "type")]
    pub entity_type: EntityKind,
    pub id: String,
    pub identifier: String,
    pub title: String,
    #[serde(default)]
    pub url: Option<String>,
}

/// One executable option of an `options`-kind fix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixOption {
    pub action: String,
    pub command: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub needs_input: bool,
}

/// A fix attached to a finding (R28). Serializes with a `kind` tag of
/// `command` / `options` / `needs_input`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Fix {
    Command { command: String },
    Options { options: Vec<FixOption> },
    NeedsInput { command: String },
}

/// A hygiene finding (§3.3/§4.4). JSON field names are camelCase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub dedupe_key: String,
    pub rule: String,
    pub severity: Severity,
    pub entity: FindingEntity,
    pub owner: Option<OwnerRef>,
    pub summary: String,
    pub evidence: Value,
    pub fix: Option<Fix>,
}

/// Run all enabled rules over the entities, honoring scope exemptions (R14),
/// and return deterministically sorted findings (R18).
pub fn run_rules(
    rules: &[Rule],
    entities: &[EntityModel],
    scope: &ScopeConfig,
    inputs: &EvalInputs,
) -> Vec<Finding> {
    let eligible: Vec<&EntityModel> = entities
        .iter()
        .filter(|entity| !is_exempt(entity, scope))
        .collect();

    let mut findings = Vec::new();
    for rule in rules.iter().filter(|r| r.enabled()) {
        match rule {
            Rule::When(when_rule) => {
                for entity in eligible.iter().filter(|e| e.kind == when_rule.entity) {
                    if let Some(finding) = evaluate_when_rule(when_rule, entity, inputs) {
                        findings.push(finding);
                    }
                }
            }
            Rule::Builtin(builtin) => {
                findings.extend(evaluate_builtin(builtin, &eligible, inputs));
            }
        }
    }
    sort_findings(&mut findings);
    findings
}

/// Entities carrying any exempt label are skipped by all rules (R14).
pub fn is_exempt(entity: &EntityModel, scope: &ScopeConfig) -> bool {
    let labels = entity.labels();
    labels.iter().any(|label| {
        scope
            .exempt_labels
            .iter()
            .any(|e| e.eq_ignore_ascii_case(label))
    })
}

/// Sort findings by severity desc, rule id, entity identifier (R18).
pub fn sort_findings(findings: &mut [Finding]) {
    findings.sort_by(|a, b| {
        a.severity
            .rank()
            .cmp(&b.severity.rank())
            .then_with(|| a.rule.cmp(&b.rule))
            .then_with(|| a.entity.identifier.cmp(&b.entity.identifier))
    });
}

/// Drop findings whose dedupe key is actively snoozed (R18/R26).
pub fn filter_snoozed(findings: Vec<Finding>, snoozed: &BTreeSet<String>) -> Vec<Finding> {
    findings
        .into_iter()
        .filter(|f| !snoozed.contains(&f.dedupe_key))
        .collect()
}

// ---------------------------------------------------------------------------
// `when`-rule evaluation
// ---------------------------------------------------------------------------

struct CondMatch {
    description: String,
    evidence: Vec<(String, Value)>,
}

/// Evaluate a `when`-rule against one entity. All conditions must match (R10).
pub fn evaluate_when_rule(
    rule: &WhenRule,
    entity: &EntityModel,
    inputs: &EvalInputs,
) -> Option<Finding> {
    let mut descriptions = Vec::new();
    let mut evidence = Map::new();
    for condition in &rule.when {
        let matched = match_condition(condition, entity, inputs)?;
        descriptions.push(matched.description);
        for (key, value) in matched.evidence {
            evidence.insert(key, value);
        }
    }
    // Always snapshot updatedAt in evidence (R16).
    evidence
        .entry("updatedAt".to_string())
        .or_insert_with(|| entity.field("updatedAt").to_json());

    Some(Finding {
        dedupe_key: format!("{}:{}", rule.id, entity.identifier),
        rule: rule.id.clone(),
        severity: rule.severity,
        entity: finding_entity(entity),
        owner: entity.owner.clone(),
        summary: format!("{} {}", entity.identifier, descriptions.join(", ")),
        evidence: Value::Object(evidence),
        fix: derive_fix(rule, entity, inputs),
    })
}

fn finding_entity(entity: &EntityModel) -> FindingEntity {
    FindingEntity {
        entity_type: entity.kind,
        id: entity.id.clone(),
        identifier: entity.identifier.clone(),
        title: entity.title.clone(),
        url: entity.url.clone(),
    }
}

fn match_condition(
    condition: &Condition,
    entity: &EntityModel,
    inputs: &EvalInputs,
) -> Option<CondMatch> {
    let field = condition.field.as_str();
    let value = entity.field(field);
    let field_evidence = (field.to_string(), value.to_json());

    match &condition.predicate {
        Predicate::In(list) => {
            let matched = match &value {
                FieldValue::String(s) => list.contains(s),
                FieldValue::StringList(items) => items.iter().any(|i| list.contains(i)),
                _ => false,
            };
            matched.then(|| CondMatch {
                description: format!("{field}='{}'", value.display()),
                evidence: vec![field_evidence],
            })
        }
        Predicate::NotIn(list) => {
            let matched = match &value {
                FieldValue::String(s) => !list.contains(s),
                FieldValue::StringList(items) => !items.iter().any(|i| list.contains(i)),
                FieldValue::Null => true,
                _ => false,
            };
            matched.then(|| CondMatch {
                description: format!("{field}='{}'", value.display()),
                evidence: vec![field_evidence],
            })
        }
        Predicate::Missing(expected) => {
            let matched = value.is_missing() == *expected;
            matched.then(|| CondMatch {
                description: if *expected {
                    format!("{field} missing")
                } else {
                    format!("{field} present ('{}')", value.display())
                },
                evidence: vec![field_evidence],
            })
        }
        Predicate::OlderThan { seconds, raw } => {
            let age = field_age_seconds(&value, inputs.now)?;
            (age > *seconds as i64).then(|| CondMatch {
                description: format!(
                    "{field} {} old (threshold {raw})",
                    format_duration_compact(age.max(0) as u64)
                ),
                evidence: vec![
                    field_evidence,
                    ("ageDays".to_string(), json!(age / 86_400)),
                    ("threshold".to_string(), json!(raw)),
                ],
            })
        }
        Predicate::NewerThan { seconds, raw } => {
            let age = field_age_seconds(&value, inputs.now)?;
            (age <= *seconds as i64).then(|| CondMatch {
                description: format!(
                    "{field} {} old (within {raw})",
                    format_duration_compact(age.max(0) as u64)
                ),
                evidence: vec![
                    field_evidence,
                    ("ageDays".to_string(), json!(age / 86_400)),
                    ("threshold".to_string(), json!(raw)),
                ],
            })
        }
        Predicate::Past(expected) => {
            let FieldValue::Date(date) = value else {
                return None;
            };
            let is_past = date < inputs.now.date_naive();
            (is_past == *expected).then(|| CondMatch {
                description: if *expected {
                    format!("{field} {date} is past")
                } else {
                    format!("{field} {date} is not past")
                },
                evidence: vec![field_evidence],
            })
        }
        Predicate::ShorterThan(bound) => {
            let FieldValue::String(s) = &value else {
                return None;
            };
            let length = s.chars().count() as u64;
            (length < *bound).then(|| CondMatch {
                description: format!("{field} is {length} chars (< {bound})"),
                evidence: vec![field_evidence, (format!("{field}Length"), json!(length))],
            })
        }
        Predicate::LongerThan(bound) => {
            let FieldValue::String(s) = &value else {
                return None;
            };
            let length = s.chars().count() as u64;
            (length > *bound).then(|| CondMatch {
                description: format!("{field} is {length} chars (> {bound})"),
                evidence: vec![field_evidence, (format!("{field}Length"), json!(length))],
            })
        }
        Predicate::Matches(regex) => {
            let FieldValue::String(s) = &value else {
                return None;
            };
            regex.is_match(s).then(|| CondMatch {
                description: format!("{field} matches /{}/", regex.as_str()),
                evidence: vec![field_evidence],
            })
        }
        Predicate::NotMatches(regex) => {
            let matched = match &value {
                FieldValue::String(s) => !regex.is_match(s),
                FieldValue::Null => true,
                _ => return None,
            };
            matched.then(|| CondMatch {
                description: format!("{field} does not match /{}/", regex.as_str()),
                evidence: vec![field_evidence],
            })
        }
        Predicate::Lt(bound)
        | Predicate::Lte(bound)
        | Predicate::Gt(bound)
        | Predicate::Gte(bound)
        | Predicate::Eq(bound) => {
            let FieldValue::Number(n) = value else {
                return None;
            };
            let op = condition.predicate.operator_name();
            let matched = match &condition.predicate {
                Predicate::Lt(_) => n < *bound,
                Predicate::Lte(_) => n <= *bound,
                Predicate::Gt(_) => n > *bound,
                Predicate::Gte(_) => n >= *bound,
                _ => n == *bound,
            };
            matched.then(|| CondMatch {
                description: format!(
                    "{field}={} ({op} {})",
                    crate::hygiene::model::format_number(n),
                    crate::hygiene::model::format_number(*bound)
                ),
                evidence: vec![(field.to_string(), json!(n))],
            })
        }
        Predicate::MissingGroup(group) => {
            // Unknown groups never match; the commands layer validates group
            // names against the context config before evaluation.
            let candidates = inputs.label_groups.get(group)?;
            let labels = entity.labels();
            let has_group_label = labels
                .iter()
                .any(|label| candidates.iter().any(|c| c.eq_ignore_ascii_case(label)));
            (!has_group_label).then(|| CondMatch {
                description: format!("labels missing group '{group}'"),
                evidence: vec![
                    ("labels".to_string(), json!(labels)),
                    ("missingGroup".to_string(), json!(group)),
                ],
            })
        }
    }
}

/// Age of a datetime/date field in seconds relative to `now`. `None` when the
/// field is null or not a temporal value.
fn field_age_seconds(value: &FieldValue, now: DateTime<Utc>) -> Option<i64> {
    match value {
        FieldValue::DateTime(dt) => Some((now - *dt).num_seconds()),
        FieldValue::Date(d) => {
            let midnight = Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0)?);
            Some((now - midnight).num_seconds())
        }
        _ => None,
    }
}

/// Compact `6d` / `4h` / `30m` / `45s` rendering for summaries.
pub fn format_duration_compact(seconds: u64) -> String {
    if seconds >= 86_400 {
        format!("{}d", seconds / 86_400)
    } else if seconds >= 3_600 {
        format!("{}h", seconds / 3_600)
    } else if seconds >= 60 {
        format!("{}m", seconds / 60)
    } else {
        format!("{}s", seconds)
    }
}

// ---------------------------------------------------------------------------
// Fix derivation (R30/R31, §4.5)
// ---------------------------------------------------------------------------

fn derive_fix(rule: &WhenRule, entity: &EntityModel, inputs: &EvalInputs) -> Option<Fix> {
    match &rule.fix {
        Some(FixConfig::Set(set)) => Some(Fix::Command {
            command: update_command(entity, set),
        }),
        Some(FixConfig::Options(options)) => Some(config_options_fix(options, entity)),
        None => auto_derived_fix(rule, entity, inputs),
    }
}

fn config_options_fix(options: &[FixOptionConfig], entity: &EntityModel) -> Fix {
    let built: Vec<FixOption> = options
        .iter()
        .map(|option| {
            let mut parts = Vec::new();
            if !option.set.is_empty() {
                parts.push(update_command(entity, &option.set));
            }
            if option.comment {
                parts.push(comment_command(entity));
            }
            FixOption {
                action: option.action.clone(),
                command: parts.join(" && "),
                needs_input: option.comment,
            }
        })
        .collect();

    // A single authored-content option degrades to a needs_input fix (§4.5).
    if built.len() == 1 && built[0].needs_input {
        Fix::NeedsInput {
            command: built[0].command.clone(),
        }
    } else {
        Fix::Options { options: built }
    }
}

/// Without a `[fix]` block, missing-field / missing-group predicates get an
/// `options`-kind fix enumerating valid candidates from supplied metadata;
/// exactly one candidate degrades to `command` (R30). The CLI enumerates; it
/// never selects.
fn auto_derived_fix(rule: &WhenRule, entity: &EntityModel, inputs: &EvalInputs) -> Option<Fix> {
    for condition in &rule.when {
        match &condition.predicate {
            Predicate::Missing(true) => {
                let field = condition.field.as_str();
                if settable_field_flag(entity.kind, field).is_none() {
                    continue;
                }
                let Some(candidates) = inputs.field_candidates.get(field).filter(|c| !c.is_empty())
                else {
                    continue;
                };
                return Some(options_from_candidates(entity, field, candidates));
            }
            Predicate::MissingGroup(group) => {
                let Some(candidates) = inputs.label_groups.get(group).filter(|c| !c.is_empty())
                else {
                    continue;
                };
                return Some(options_from_candidates(entity, "labels", candidates));
            }
            _ => {}
        }
    }
    None
}

fn options_from_candidates(entity: &EntityModel, field: &str, candidates: &[String]) -> Fix {
    let options: Vec<FixOption> = candidates
        .iter()
        .map(|candidate| FixOption {
            action: action_slug(field, candidate),
            command: update_command(
                entity,
                &[(field.to_string(), SetValue::String(candidate.clone()))],
            ),
            needs_input: false,
        })
        .collect();
    if options.len() == 1 {
        Fix::Command {
            command: options[0].command.clone(),
        }
    } else {
        Fix::Options { options }
    }
}

/// Stable option-action names: `p1`–`p4` for priorities, slugified values
/// otherwise (`In Progress` → `in-progress`).
fn action_slug(field: &str, candidate: &str) -> String {
    if field == "priority" {
        if let Ok(n) = candidate.trim().parse::<i64>() {
            return format!("p{n}");
        }
    }
    let mut slug = String::new();
    for c in candidate.trim().chars() {
        if c.is_alphanumeric() {
            slug.extend(c.to_lowercase());
        } else if (c.is_whitespace() || c == '-' || c == '_' || c == '/') && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_matches('-').to_string()
}

/// Build the `linear … update` command applying the given field assignments.
/// Commands never embed secrets or profile flags (R29).
fn update_command(entity: &EntityModel, set: &[(String, SetValue)]) -> String {
    let mut command = match entity.kind {
        EntityKind::Issue => format!("linear i update {}", shell_arg(&entity.identifier)),
        EntityKind::Project => format!("linear p update {}", shell_arg(&entity.id)),
        EntityKind::Initiative => format!("linear init update {}", shell_arg(&entity.id)),
    };
    for (field, value) in set {
        // Validated as settable at config load; skip defensively otherwise.
        let Some(flag) = settable_field_flag(entity.kind, field) else {
            continue;
        };
        command.push(' ');
        command.push_str(flag);
        command.push(' ');
        command.push_str(&shell_arg(&value.display()));
    }
    command
}

fn comment_command(entity: &EntityModel) -> String {
    format!(
        "linear cm create {} -b \"<comment>\"",
        shell_arg(&entity.identifier)
    )
}

/// Quote a command argument when it contains anything beyond safe literal
/// characters. Double-quoted with `\`, `"`, `$`, and backtick escaped.
fn shell_arg(value: &str) -> String {
    let safe = !value.is_empty()
        && value.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | ':' | '@' | '+' | ',')
        });
    if safe {
        return value.to_string();
    }
    let mut quoted = String::from("\"");
    for c in value.chars() {
        if matches!(c, '\\' | '"' | '$' | '`') {
            quoted.push('\\');
        }
        quoted.push(c);
    }
    quoted.push('"');
    quoted
}

// ---------------------------------------------------------------------------
// Builtins (§4.3)
// ---------------------------------------------------------------------------

fn evaluate_builtin(
    rule: &BuiltinRule,
    entities: &[&EntityModel],
    inputs: &EvalInputs,
) -> Vec<Finding> {
    match &rule.params {
        BuiltinParams::WipLimit {
            max_in_progress_per_assignee,
            statuses,
        } => wip_limit(rule, *max_in_progress_per_assignee, statuses, entities),
        BuiltinParams::InitiativeCompletedButActive => {
            initiative_completed_but_active(rule, entities)
        }
        BuiltinParams::ProjectSingleIssue {
            min_issues,
            min_age_seconds,
            min_age,
        } => project_single_issue(
            rule,
            *min_issues,
            *min_age_seconds,
            min_age,
            entities,
            inputs,
        ),
        BuiltinParams::ProjectNoTargetDateLongLived {
            min_age_seconds,
            min_age,
        } => project_no_target_date(rule, *min_age_seconds, min_age, entities, inputs),
    }
}

/// One finding per assignee over the WIP limit. Grouped by a non-entity
/// subject, so the dedupe key is `<rule-id>:<assignee>` (R17) and the entity
/// reference carries the subject as its identifier.
fn wip_limit(
    rule: &BuiltinRule,
    max: u64,
    statuses: &[String],
    entities: &[&EntityModel],
) -> Vec<Finding> {
    let mut by_assignee: BTreeMap<String, Vec<&EntityModel>> = BTreeMap::new();
    for entity in entities.iter().filter(|e| e.kind == EntityKind::Issue) {
        let FieldValue::String(status) = entity.field("status") else {
            continue;
        };
        if !statuses.contains(&status) {
            continue;
        }
        let FieldValue::String(assignee) = entity.field("assignee") else {
            continue; // unassigned issues don't count toward a person's WIP
        };
        by_assignee.entry(assignee).or_default().push(entity);
    }

    let mut findings = Vec::new();
    for (assignee, issues) in by_assignee {
        let count = issues.len() as u64;
        if count <= max {
            continue;
        }
        let mut identifiers: Vec<&str> = issues.iter().map(|i| i.identifier.as_str()).collect();
        identifiers.sort_unstable();
        findings.push(Finding {
            dedupe_key: format!("{}:{}", rule.id, assignee),
            rule: rule.id.clone(),
            severity: rule.severity,
            entity: FindingEntity {
                entity_type: EntityKind::Issue,
                id: String::new(),
                identifier: assignee.clone(),
                title: format!("{count} issues in progress for {assignee}"),
                url: None,
            },
            owner: issues[0].owner.clone(),
            summary: format!(
                "{assignee} has {count} issues in {} (limit {max})",
                statuses.join("/")
            ),
            evidence: json!({
                "assignee": assignee,
                "count": count,
                "limit": max,
                "statuses": statuses,
                "issues": identifiers,
            }),
            fix: None, // which issue to pause is a human call (§4.5)
        });
    }
    findings
}

fn initiative_completed_but_active(rule: &BuiltinRule, entities: &[&EntityModel]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for entity in entities.iter().filter(|e| e.kind == EntityKind::Initiative) {
        let Some(states) = entity.side.initiative_project_states.as_ref() else {
            continue;
        };
        if states.is_empty() || !states.iter().all(|s| s.eq_ignore_ascii_case("completed")) {
            continue;
        }
        let FieldValue::String(state) = entity.field("state") else {
            continue;
        };
        if state.eq_ignore_ascii_case("completed") || state.eq_ignore_ascii_case("canceled") {
            continue;
        }
        findings.push(Finding {
            dedupe_key: format!("{}:{}", rule.id, entity.identifier),
            rule: rule.id.clone(),
            severity: rule.severity,
            entity: finding_entity(entity),
            owner: entity.owner.clone(),
            summary: format!(
                "{} has all {} linked projects completed but is still '{}'",
                entity.identifier,
                states.len(),
                state
            ),
            evidence: json!({
                "state": state,
                "linkedProjects": states.len(),
                "projectStates": states,
                "updatedAt": entity.field("updatedAt").to_json(),
            }),
            // Builtin contract: deterministic remedy (§4.5).
            fix: Some(Fix::Command {
                command: format!("linear init update {} -s Completed", shell_arg(&entity.id)),
            }),
        });
    }
    findings
}

fn project_single_issue(
    rule: &BuiltinRule,
    min_issues: u64,
    min_age_seconds: u64,
    min_age: &str,
    entities: &[&EntityModel],
    inputs: &EvalInputs,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    for entity in entities.iter().filter(|e| e.kind == EntityKind::Project) {
        let Some(count) = entity.side.project_issue_count else {
            continue;
        };
        if count >= min_issues {
            continue;
        }
        let Some(age) = field_age_seconds(&entity.field("createdAt"), inputs.now) else {
            continue;
        };
        if age <= min_age_seconds as i64 {
            continue;
        }
        findings.push(Finding {
            dedupe_key: format!("{}:{}", rule.id, entity.identifier),
            rule: rule.id.clone(),
            severity: rule.severity,
            entity: finding_entity(entity),
            owner: entity.owner.clone(),
            summary: format!(
                "{} wraps {count} issue(s) after {} (expected ≥ {min_issues} issues within {min_age})",
                entity.identifier,
                format_duration_compact(age.max(0) as u64)
            ),
            evidence: json!({
                "issueCount": count,
                "minIssues": min_issues,
                "createdAt": entity.field("createdAt").to_json(),
                "ageDays": age / 86_400,
                "threshold": min_age,
                "updatedAt": entity.field("updatedAt").to_json(),
            }),
            fix: None,
        });
    }
    findings
}

fn project_no_target_date(
    rule: &BuiltinRule,
    min_age_seconds: u64,
    min_age: &str,
    entities: &[&EntityModel],
    inputs: &EvalInputs,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    for entity in entities.iter().filter(|e| e.kind == EntityKind::Project) {
        if !entity.field("targetDate").is_missing() {
            continue;
        }
        let Some(age) = field_age_seconds(&entity.field("createdAt"), inputs.now) else {
            continue;
        };
        if age <= min_age_seconds as i64 {
            continue;
        }
        findings.push(Finding {
            dedupe_key: format!("{}:{}", rule.id, entity.identifier),
            rule: rule.id.clone(),
            severity: rule.severity,
            entity: finding_entity(entity),
            owner: entity.owner.clone(),
            summary: format!(
                "{} has no target date after {} (threshold {min_age})",
                entity.identifier,
                format_duration_compact(age.max(0) as u64)
            ),
            evidence: json!({
                "targetDate": Value::Null,
                "createdAt": entity.field("createdAt").to_json(),
                "ageDays": age / 86_400,
                "threshold": min_age,
                "updatedAt": entity.field("updatedAt").to_json(),
            }),
            fix: None, // dates cannot be enumerated; visibility only
        });
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hygiene::config::{parse_hygiene_toml, HygieneConfig};
    use crate::hygiene::model::SideData;
    use chrono::NaiveDate;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 2, 12, 0, 0).unwrap()
    }

    fn days_ago(days: i64) -> FieldValue {
        FieldValue::DateTime(now() - chrono::Duration::days(days))
    }

    fn issue(identifier: &str) -> EntityModel {
        let mut fields = BTreeMap::new();
        fields.insert("status".to_string(), FieldValue::String("Todo".into()));
        fields.insert("priority".to_string(), FieldValue::Number(3.0));
        fields.insert("estimate".to_string(), FieldValue::Null);
        fields.insert("assignee".to_string(), FieldValue::String("chris".into()));
        fields.insert("title".to_string(), FieldValue::String("A title".into()));
        fields.insert("description".to_string(), FieldValue::String("Body".into()));
        fields.insert("labels".to_string(), FieldValue::StringList(vec![]));
        fields.insert("team".to_string(), FieldValue::String("ENG".into()));
        fields.insert("project".to_string(), FieldValue::Null);
        fields.insert("cycle".to_string(), FieldValue::Null);
        fields.insert("createdAt".to_string(), days_ago(30));
        fields.insert("updatedAt".to_string(), days_ago(1));
        fields.insert("dueDate".to_string(), FieldValue::Null);
        EntityModel {
            kind: EntityKind::Issue,
            id: format!("uuid-{identifier}"),
            identifier: identifier.to_string(),
            title: "A title".to_string(),
            url: Some(format!("https://linear.app/org/issue/{identifier}")),
            owner: Some(OwnerRef {
                id: Some("u1".into()),
                name: Some("chris".into()),
                display_name: Some("Chris".into()),
            }),
            fields,
            side: SideData::default(),
        }
    }

    fn project(identifier: &str) -> EntityModel {
        let mut fields = BTreeMap::new();
        fields.insert("state".to_string(), FieldValue::String("started".into()));
        fields.insert("lead".to_string(), FieldValue::Null);
        fields.insert("name".to_string(), FieldValue::String(identifier.into()));
        fields.insert("labels".to_string(), FieldValue::StringList(vec![]));
        fields.insert("createdAt".to_string(), days_ago(90));
        fields.insert("updatedAt".to_string(), days_ago(2));
        fields.insert("targetDate".to_string(), FieldValue::Null);
        EntityModel {
            kind: EntityKind::Project,
            id: format!("proj-{identifier}"),
            identifier: identifier.to_string(),
            title: identifier.to_string(),
            url: None,
            owner: None,
            fields,
            side: SideData {
                project_issue_count: Some(5),
                initiative_project_states: None,
            },
        }
    }

    fn initiative(identifier: &str, state: &str, project_states: &[&str]) -> EntityModel {
        let mut fields = BTreeMap::new();
        fields.insert("state".to_string(), FieldValue::String(state.into()));
        fields.insert("labels".to_string(), FieldValue::StringList(vec![]));
        fields.insert("createdAt".to_string(), days_ago(200));
        fields.insert("updatedAt".to_string(), days_ago(3));
        fields.insert(
            "linkedProjects".to_string(),
            FieldValue::Number(project_states.len() as f64),
        );
        EntityModel {
            kind: EntityKind::Initiative,
            id: format!("init-{identifier}"),
            identifier: identifier.to_string(),
            title: identifier.to_string(),
            url: None,
            owner: None,
            fields,
            side: SideData {
                project_issue_count: None,
                initiative_project_states: Some(
                    project_states.iter().map(|s| s.to_string()).collect(),
                ),
            },
        }
    }

    fn empty_inputs<'a>(
        groups: &'a BTreeMap<String, Vec<String>>,
        candidates: &'a BTreeMap<String, Vec<String>>,
    ) -> EvalInputs<'a> {
        EvalInputs {
            now: now(),
            label_groups: groups,
            field_candidates: candidates,
        }
    }

    fn rules_from(toml_body: &str) -> HygieneConfig {
        parse_hygiene_toml(toml_body).expect("valid config")
    }

    fn eval_single(rule_toml: &str, entity: &EntityModel) -> Option<Finding> {
        let groups = BTreeMap::new();
        let candidates = BTreeMap::new();
        eval_single_with(rule_toml, entity, &groups, &candidates)
    }

    fn eval_single_with(
        rule_toml: &str,
        entity: &EntityModel,
        groups: &BTreeMap<String, Vec<String>>,
        candidates: &BTreeMap<String, Vec<String>>,
    ) -> Option<Finding> {
        let config = rules_from(rule_toml);
        let Rule::When(rule) = &config.rules[0] else {
            panic!("expected when rule");
        };
        evaluate_when_rule(rule, entity, &empty_inputs(groups, candidates))
    }

    // --- operators ---

    #[test]
    fn operator_in_and_not_in_strings() {
        let entity = issue("ENG-1");
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\nstatus = { in = [\"Todo\", \"Backlog\"] }\n";
        assert!(eval_single(toml, &entity).is_some());
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\nstatus = { in = [\"Done\"] }\n";
        assert!(eval_single(toml, &entity).is_none());
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\nstatus = { not_in = [\"Done\"] }\n";
        assert!(eval_single(toml, &entity).is_some());
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\nstatus = { not_in = [\"Todo\"] }\n";
        assert!(eval_single(toml, &entity).is_none());
    }

    #[test]
    fn operator_in_matches_any_label_element() {
        let mut entity = issue("ENG-1");
        entity.fields.insert(
            "labels".into(),
            FieldValue::StringList(vec!["bug".into(), "payments".into()]),
        );
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\nlabels = { in = [\"payments\"] }\n";
        assert!(eval_single(toml, &entity).is_some());
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\nlabels = { not_in = [\"payments\"] }\n";
        assert!(eval_single(toml, &entity).is_none());
    }

    #[test]
    fn operator_not_in_matches_null() {
        let mut entity = issue("ENG-1");
        entity.fields.insert("assignee".into(), FieldValue::Null);
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\nassignee = { not_in = [\"sam\"] }\n";
        assert!(eval_single(toml, &entity).is_some());
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\nassignee = { in = [\"sam\"] }\n";
        assert!(eval_single(toml, &entity).is_none());
    }

    #[test]
    fn operator_missing_true_and_false() {
        let entity = issue("ENG-1"); // estimate null, assignee present
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\nestimate = { missing = true }\n";
        assert!(eval_single(toml, &entity).is_some());
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\nassignee = { missing = true }\n";
        assert!(eval_single(toml, &entity).is_none());
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\nassignee = { missing = false }\n";
        assert!(eval_single(toml, &entity).is_some());
        // Empty label list counts as missing.
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\nlabels = { missing = true }\n";
        assert!(eval_single(toml, &entity).is_some());
    }

    #[test]
    fn operator_older_and_newer_than() {
        let mut entity = issue("ENG-1");
        entity.fields.insert("updatedAt".into(), days_ago(6));
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\nupdatedAt = { older_than = \"2d\" }\n";
        let finding = eval_single(toml, &entity).expect("stale issue matches");
        assert_eq!(finding.evidence["ageDays"], json!(6));
        assert_eq!(finding.evidence["threshold"], json!("2d"));
        assert!(finding.summary.contains("6d old (threshold 2d)"));

        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\nupdatedAt = { older_than = \"7d\" }\n";
        assert!(eval_single(toml, &entity).is_none());

        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\nupdatedAt = { newer_than = \"7d\" }\n";
        assert!(eval_single(toml, &entity).is_some());
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\nupdatedAt = { newer_than = \"2d\" }\n";
        assert!(eval_single(toml, &entity).is_none());
    }

    #[test]
    fn operator_older_than_null_never_matches() {
        let mut entity = issue("ENG-1");
        entity.fields.insert("updatedAt".into(), FieldValue::Null);
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\nupdatedAt = { older_than = \"2d\" }\n";
        assert!(eval_single(toml, &entity).is_none());
    }

    #[test]
    fn operator_past() {
        let mut entity = issue("ENG-1");
        entity.fields.insert(
            "dueDate".into(),
            FieldValue::Date(NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()),
        );
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\ndueDate = { past = true }\n";
        assert!(eval_single(toml, &entity).is_some());

        entity.fields.insert(
            "dueDate".into(),
            FieldValue::Date(NaiveDate::from_ymd_opt(2026, 8, 1).unwrap()),
        );
        assert!(eval_single(toml, &entity).is_none());
        let toml_not = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\ndueDate = { past = false }\n";
        assert!(eval_single(toml_not, &entity).is_some());

        // Null date matches neither polarity.
        entity.fields.insert("dueDate".into(), FieldValue::Null);
        assert!(eval_single(toml, &entity).is_none());
        assert!(eval_single(toml_not, &entity).is_none());
    }

    #[test]
    fn operator_shorter_and_longer_than() {
        let mut entity = issue("ENG-1");
        entity
            .fields
            .insert("description".into(), FieldValue::String("short".into()));
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\ndescription = { shorter_than = 10 }\n";
        assert!(eval_single(toml, &entity).is_some());
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\ndescription = { longer_than = 3 }\n";
        assert!(eval_single(toml, &entity).is_some());
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\ndescription = { longer_than = 5 }\n";
        assert!(eval_single(toml, &entity).is_none());
        // Null string matches neither length bound.
        entity.fields.insert("description".into(), FieldValue::Null);
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\ndescription = { shorter_than = 10 }\n";
        assert!(eval_single(toml, &entity).is_none());
    }

    #[test]
    fn operator_matches_and_not_matches() {
        let entity = issue("ENG-1"); // title "A title"
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\ntitle = { matches = \"^A \" }\n";
        assert!(eval_single(toml, &entity).is_some());
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\ntitle = { matches = \"^Z\" }\n";
        assert!(eval_single(toml, &entity).is_none());
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\ntitle = { not_matches = \"^Z\" }\n";
        assert!(eval_single(toml, &entity).is_some());
        // Null text: matches fails, not_matches succeeds.
        let mut entity = entity;
        entity.fields.insert("description".into(), FieldValue::Null);
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\ndescription = { matches = \".\" }\n";
        assert!(eval_single(toml, &entity).is_none());
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\ndescription = { not_matches = \".\" }\n";
        assert!(eval_single(toml, &entity).is_some());
    }

    #[test]
    fn operator_numeric_comparisons() {
        let entity = issue("ENG-1"); // priority 3
        for (op, bound, expected) in [
            ("lt", 4, true),
            ("lt", 3, false),
            ("lte", 3, true),
            ("gt", 2, true),
            ("gt", 3, false),
            ("gte", 3, true),
            ("eq", 3, true),
            ("eq", 1, false),
        ] {
            let toml = format!(
                "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\npriority = {{ {op} = {bound} }}\n"
            );
            assert_eq!(
                eval_single(&toml, &entity).is_some(),
                expected,
                "priority {op} {bound}"
            );
        }
        // Null number never matches.
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\nestimate = { gt = 0 }\n";
        assert!(eval_single(toml, &entity).is_none());
    }

    #[test]
    fn operator_missing_group() {
        let mut groups = BTreeMap::new();
        groups.insert(
            "domain".to_string(),
            vec!["payments".to_string(), "auth".to_string()],
        );
        let candidates = BTreeMap::new();
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\nlabels = { missing_group = \"domain\" }\n";

        let mut entity = issue("ENG-1");
        entity
            .fields
            .insert("labels".into(), FieldValue::StringList(vec!["bug".into()]));
        let finding =
            eval_single_with(toml, &entity, &groups, &candidates).expect("missing group matches");
        assert_eq!(finding.evidence["missingGroup"], json!("domain"));

        entity.fields.insert(
            "labels".into(),
            FieldValue::StringList(vec!["Payments".into()]),
        );
        assert!(eval_single_with(toml, &entity, &groups, &candidates).is_none());

        // Unknown group name never matches.
        let unknown = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\nlabels = { missing_group = \"nope\" }\n";
        assert!(eval_single_with(unknown, &entity, &groups, &candidates).is_none());
    }

    #[test]
    fn conditions_and_together() {
        let mut entity = issue("ENG-1");
        entity
            .fields
            .insert("status".into(), FieldValue::String("In Review".into()));
        entity.fields.insert("updatedAt".into(), days_ago(6));
        let toml = "[[hygiene.rules]]\nid=\"stale-in-review\"\nentity=\"issue\"\nseverity=\"high\"\n[hygiene.rules.when]\nstatus = { in = [\"In Review\"] }\nupdatedAt = { older_than = \"2d\" }\n";
        let finding = eval_single(toml, &entity).expect("both conditions match");
        assert_eq!(finding.dedupe_key, "stale-in-review:ENG-1");
        assert_eq!(finding.severity, Severity::High);
        assert_eq!(finding.evidence["status"], json!("In Review"));

        // Fail one leg → no finding.
        entity.fields.insert("updatedAt".into(), days_ago(1));
        assert!(eval_single(toml, &entity).is_none());
    }

    #[test]
    fn finding_shape_and_serialization() {
        let mut entity = issue("ENG-123");
        entity.fields.insert("updatedAt".into(), days_ago(6));
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\nupdatedAt = { older_than = \"2d\" }\n";
        let finding = eval_single(toml, &entity).unwrap();
        let value = serde_json::to_value(&finding).unwrap();
        assert_eq!(value["dedupeKey"], json!("r:ENG-123"));
        assert_eq!(value["severity"], json!("medium"));
        assert_eq!(value["entity"]["type"], json!("issue"));
        assert_eq!(value["entity"]["identifier"], json!("ENG-123"));
        assert_eq!(value["owner"]["displayName"], json!("Chris"));
        assert!(value["evidence"]["updatedAt"].is_string());
        assert_eq!(value["fix"], Value::Null);
        // Round-trips for the state artifact.
        let back: Finding = serde_json::from_value(value).unwrap();
        assert_eq!(back, finding);
    }

    // --- fix derivation matrix (§4.5) ---

    #[test]
    fn fix_config_set_is_command_kind() {
        let mut entity = issue("ENG-9");
        entity
            .fields
            .insert("priority".into(), FieldValue::Number(1.0));
        entity
            .fields
            .insert("status".into(), FieldValue::String("Backlog".into()));
        let toml = r#"
[[hygiene.rules]]
id = "urgent-in-backlog"
entity = "issue"
severity = "high"
[hygiene.rules.when]
priority = { eq = 1 }
status = { in = ["Backlog"] }
[hygiene.rules.fix]
set = { status = "Todo" }
"#;
        let finding = eval_single(toml, &entity).unwrap();
        assert_eq!(
            finding.fix,
            Some(Fix::Command {
                command: "linear i update ENG-9 -s Todo".to_string()
            })
        );
    }

    #[test]
    fn fix_auto_derived_options_for_missing_field() {
        let groups = BTreeMap::new();
        let mut entity = issue("ENG-2");
        entity.fields.insert("estimate".into(), FieldValue::Null);
        let mut est_candidates: BTreeMap<String, Vec<String>> = BTreeMap::new();
        est_candidates.insert(
            "estimate".to_string(),
            vec!["1".into(), "2".into(), "3".into()],
        );
        let toml = "[[hygiene.rules]]\nid=\"missing-estimate\"\nentity=\"issue\"\n[hygiene.rules.when]\nestimate = { missing = true }\n";
        let finding = eval_single_with(toml, &entity, &groups, &est_candidates).unwrap();
        let Some(Fix::Options { options }) = finding.fix else {
            panic!("expected options fix, got {:?}", finding.fix);
        };
        assert_eq!(options.len(), 3);
        assert_eq!(options[0].action, "1");
        assert_eq!(options[0].command, "linear i update ENG-2 -e 1");
        assert!(!options[0].needs_input);
    }

    #[test]
    fn fix_auto_derived_priority_actions_use_p_prefix() {
        // priority is non-nullable so `missing` can't be used on it; exercise the
        // slug logic directly instead.
        assert_eq!(action_slug("priority", "2"), "p2");
        assert_eq!(action_slug("status", "In Progress"), "in-progress");
        assert_eq!(action_slug("labels", "Payments"), "payments");
    }

    #[test]
    fn fix_auto_derived_missing_group_options_and_single_candidate_command() {
        let mut groups = BTreeMap::new();
        groups.insert(
            "domain".to_string(),
            vec![
                "payments".to_string(),
                "auth".to_string(),
                "infra".to_string(),
            ],
        );
        let candidates = BTreeMap::new();
        let entity = issue("ENG-3"); // no labels
        let toml = "[[hygiene.rules]]\nid=\"missing-domain\"\nentity=\"issue\"\n[hygiene.rules.when]\nlabels = { missing_group = \"domain\" }\n";
        let finding = eval_single_with(toml, &entity, &groups, &candidates).unwrap();
        let Some(Fix::Options { options }) = &finding.fix else {
            panic!("expected options fix");
        };
        assert_eq!(options.len(), 3);
        assert_eq!(options[0].action, "payments");
        assert_eq!(options[0].command, "linear i update ENG-3 -l payments");

        // Exactly one candidate degrades to command-kind.
        groups.insert("domain".to_string(), vec!["payments".to_string()]);
        let finding = eval_single_with(toml, &entity, &groups, &candidates).unwrap();
        assert_eq!(
            finding.fix,
            Some(Fix::Command {
                command: "linear i update ENG-3 -l payments".to_string()
            })
        );
    }

    #[test]
    fn fix_none_when_no_candidates_or_not_missing_predicate() {
        // Missing-field predicate but no candidate metadata → fix null.
        let entity = issue("ENG-4");
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\nestimate = { missing = true }\n";
        let finding = eval_single(toml, &entity).unwrap();
        assert_eq!(finding.fix, None);

        // Non-missing predicate without a fix block → fix null (visibility only).
        let mut entity = issue("ENG-5");
        entity.fields.insert("updatedAt".into(), days_ago(10));
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\nupdatedAt = { older_than = \"2d\" }\n";
        let finding = eval_single(toml, &entity).unwrap();
        assert_eq!(finding.fix, None);
    }

    #[test]
    fn fix_config_options_with_comment_marks_needs_input() {
        let mut entity = issue("ENG-6");
        entity
            .fields
            .insert("status".into(), FieldValue::String("In Review".into()));
        entity.fields.insert("updatedAt".into(), days_ago(6));
        let toml = r#"
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
"#;
        let finding = eval_single(toml, &entity).unwrap();
        let Some(Fix::Options { options }) = &finding.fix else {
            panic!("expected options fix");
        };
        assert_eq!(options.len(), 2);
        assert_eq!(options[0].action, "nudge_review");
        assert!(options[0].needs_input);
        assert_eq!(
            options[0].command,
            "linear cm create ENG-6 -b \"<comment>\""
        );
        assert_eq!(options[1].action, "move_back");
        assert!(!options[1].needs_input);
        assert_eq!(
            options[1].command,
            "linear i update ENG-6 -s \"In Progress\""
        );

        // Serialized form keeps `needsInput` only where true.
        let value = serde_json::to_value(&finding.fix).unwrap();
        assert_eq!(value["kind"], json!("options"));
        assert_eq!(value["options"][0]["needsInput"], json!(true));
        assert!(value["options"][1].get("needsInput").is_none());
    }

    #[test]
    fn fix_single_comment_option_degrades_to_needs_input() {
        let mut entity = issue("ENG-7");
        entity.fields.insert("updatedAt".into(), days_ago(20));
        let toml = r#"
[[hygiene.rules]]
id = "stale-health"
entity = "issue"
[hygiene.rules.when]
updatedAt = { older_than = "14d" }
[[hygiene.rules.fix.options]]
action = "post_update"
comment = true
"#;
        let finding = eval_single(toml, &entity).unwrap();
        let Some(Fix::NeedsInput { command }) = &finding.fix else {
            panic!("expected needs_input fix, got {:?}", finding.fix);
        };
        assert_eq!(command, "linear cm create ENG-7 -b \"<comment>\"");
        let value = serde_json::to_value(&finding.fix).unwrap();
        assert_eq!(value["kind"], json!("needs_input"));
    }

    #[test]
    fn fix_option_with_set_and_comment_chains_commands() {
        let mut entity = issue("ENG-8");
        entity.fields.insert("updatedAt".into(), days_ago(20));
        let toml = r#"
[[hygiene.rules]]
id = "r"
entity = "issue"
[hygiene.rules.when]
updatedAt = { older_than = "14d" }
[[hygiene.rules.fix.options]]
action = "pause"
set = { status = "Backlog" }
comment = true
[[hygiene.rules.fix.options]]
action = "close"
set = { status = "Canceled" }
"#;
        let finding = eval_single(toml, &entity).unwrap();
        let Some(Fix::Options { options }) = &finding.fix else {
            panic!("expected options");
        };
        assert_eq!(
            options[0].command,
            "linear i update ENG-8 -s Backlog && linear cm create ENG-8 -b \"<comment>\""
        );
        assert!(options[0].needs_input);
    }

    #[test]
    fn fix_commands_quote_values_and_never_embed_profile_flags() {
        let mut entity = issue("ENG-10");
        entity.fields.insert("updatedAt".into(), days_ago(9));
        let toml = r#"
[[hygiene.rules]]
id = "r"
entity = "issue"
[hygiene.rules.when]
updatedAt = { older_than = "2d" }
[hygiene.rules.fix]
set = { status = "In Progress", priority = 2 }
"#;
        let finding = eval_single(toml, &entity).unwrap();
        let Some(Fix::Command { command }) = &finding.fix else {
            panic!("expected command");
        };
        // Fields are emitted in the toml crate's sorted key order.
        assert_eq!(command, "linear i update ENG-10 -p 2 -s \"In Progress\"");
        assert!(!command.contains("--api-key"));
        assert!(!command.contains("--profile"));
    }

    #[test]
    fn shell_arg_escapes_specials() {
        assert_eq!(shell_arg("Todo"), "Todo");
        assert_eq!(shell_arg("In Progress"), "\"In Progress\"");
        assert_eq!(shell_arg("a\"b"), "\"a\\\"b\"");
        assert_eq!(shell_arg("$HOME"), "\"\\$HOME\"");
        assert_eq!(shell_arg(""), "\"\"");
    }

    #[test]
    fn project_fix_commands_use_project_flags() {
        let mut entity = project("payments");
        entity.fields.insert("updatedAt".into(), days_ago(30));
        let toml = r#"
[[hygiene.rules]]
id = "r"
entity = "project"
[hygiene.rules.when]
updatedAt = { older_than = "7d" }
[hygiene.rules.fix]
set = { state = "paused" }
"#;
        let config = rules_from(toml);
        let Rule::When(rule) = &config.rules[0] else {
            panic!()
        };
        let groups = BTreeMap::new();
        let candidates = BTreeMap::new();
        let finding =
            evaluate_when_rule(rule, &entity, &empty_inputs(&groups, &candidates)).unwrap();
        assert_eq!(
            finding.fix,
            Some(Fix::Command {
                command: "linear p update proj-payments --status paused".to_string()
            })
        );
    }

    // --- builtins ---

    fn builtin_rule(toml_body: &str) -> BuiltinRule {
        let config = rules_from(toml_body);
        let Rule::Builtin(rule) = &config.rules[0] else {
            panic!("expected builtin");
        };
        rule.clone()
    }

    #[test]
    fn builtin_wip_limit_groups_by_assignee() {
        let rule = builtin_rule(
            "[[hygiene.rules]]\nbuiltin = \"wip-limit\"\n[hygiene.rules.params]\nmax_in_progress_per_assignee = 2\n",
        );
        let mut issues = Vec::new();
        for (identifier, assignee) in [
            ("ENG-1", "chris"),
            ("ENG-2", "chris"),
            ("ENG-3", "chris"),
            ("ENG-4", "sam"),
        ] {
            let mut e = issue(identifier);
            e.fields
                .insert("status".into(), FieldValue::String("In Progress".into()));
            e.fields
                .insert("assignee".into(), FieldValue::String(assignee.into()));
            issues.push(e);
        }
        // Unassigned in-progress issue never counts.
        let mut unassigned = issue("ENG-5");
        unassigned
            .fields
            .insert("status".into(), FieldValue::String("In Progress".into()));
        unassigned
            .fields
            .insert("assignee".into(), FieldValue::Null);
        issues.push(unassigned);

        let refs: Vec<&EntityModel> = issues.iter().collect();
        let groups = BTreeMap::new();
        let candidates = BTreeMap::new();
        let findings = evaluate_builtin(&rule, &refs, &empty_inputs(&groups, &candidates));
        assert_eq!(findings.len(), 1);
        let finding = &findings[0];
        assert_eq!(finding.dedupe_key, "wip-limit:chris");
        assert_eq!(finding.entity.identifier, "chris");
        assert_eq!(finding.evidence["count"], json!(3));
        assert_eq!(finding.evidence["limit"], json!(2));
        assert_eq!(
            finding.evidence["issues"],
            json!(["ENG-1", "ENG-2", "ENG-3"])
        );
        assert_eq!(finding.fix, None);
        assert!(finding.summary.contains("chris has 3 issues"));
    }

    #[test]
    fn builtin_initiative_completed_but_active() {
        let rule =
            builtin_rule("[[hygiene.rules]]\nbuiltin = \"initiative-completed-but-active\"\n");
        let flagged = initiative("platform", "started", &["completed", "Completed"]);
        let not_all_done = initiative("mobile", "started", &["completed", "started"]);
        let already_done = initiative("legacy", "Completed", &["completed"]);
        let no_projects = initiative("empty", "started", &[]);
        let entities = [&flagged, &not_all_done, &already_done, &no_projects];
        let groups = BTreeMap::new();
        let candidates = BTreeMap::new();
        let findings = evaluate_builtin(&rule, &entities, &empty_inputs(&groups, &candidates));
        assert_eq!(findings.len(), 1);
        let finding = &findings[0];
        assert_eq!(
            finding.dedupe_key,
            "initiative-completed-but-active:platform"
        );
        assert_eq!(
            finding.fix,
            Some(Fix::Command {
                command: "linear init update init-platform -s Completed".to_string()
            })
        );
    }

    #[test]
    fn builtin_project_single_issue() {
        let rule = builtin_rule(
            "[[hygiene.rules]]\nbuiltin = \"project-single-issue\"\n[hygiene.rules.params]\nmin_issues = 2\nmin_age = \"14d\"\n",
        );
        let mut lonely = project("solo");
        lonely.side.project_issue_count = Some(1);
        let mut young = project("fresh");
        young.side.project_issue_count = Some(1);
        young.fields.insert("createdAt".into(), days_ago(3));
        let healthy = project("busy"); // 5 issues
        let mut unknown = project("mystery");
        unknown.side.project_issue_count = None;

        let entities = [&lonely, &young, &healthy, &unknown];
        let groups = BTreeMap::new();
        let candidates = BTreeMap::new();
        let findings = evaluate_builtin(&rule, &entities, &empty_inputs(&groups, &candidates));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].dedupe_key, "project-single-issue:solo");
        assert_eq!(findings[0].evidence["issueCount"], json!(1));
        assert_eq!(findings[0].fix, None);
    }

    #[test]
    fn builtin_project_no_target_date() {
        let rule = builtin_rule(
            "[[hygiene.rules]]\nbuiltin = \"project-no-target-date-long-lived\"\n[hygiene.rules.params]\nmin_age = \"60d\"\n",
        );
        let old_undated = project("drift"); // 90d old, no targetDate
        let mut dated = project("planned");
        dated.fields.insert(
            "targetDate".into(),
            FieldValue::Date(NaiveDate::from_ymd_opt(2026, 9, 1).unwrap()),
        );
        let mut young = project("new");
        young.fields.insert("createdAt".into(), days_ago(10));

        let entities = [&old_undated, &dated, &young];
        let groups = BTreeMap::new();
        let candidates = BTreeMap::new();
        let findings = evaluate_builtin(&rule, &entities, &empty_inputs(&groups, &candidates));
        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].dedupe_key,
            "project-no-target-date-long-lived:drift"
        );
        assert_eq!(findings[0].fix, None);
    }

    // --- run_rules: exemption, disabled rules, sorting, snooze ---

    #[test]
    fn run_rules_skips_exempt_and_disabled() {
        let toml = r#"
[[hygiene.rules]]
id = "stale"
entity = "issue"
[hygiene.rules.when]
updatedAt = { older_than = "1d" }

[[hygiene.rules]]
id = "off"
entity = "issue"
enabled = false
[hygiene.rules.when]
updatedAt = { older_than = "1d" }
"#;
        let config = rules_from(toml);
        let mut normal = issue("ENG-1");
        normal.fields.insert("updatedAt".into(), days_ago(5));
        let mut exempt = issue("ENG-2");
        exempt.fields.insert("updatedAt".into(), days_ago(5));
        exempt.fields.insert(
            "labels".into(),
            FieldValue::StringList(vec!["ignore-audit".into()]),
        );
        let entities = vec![normal, exempt];
        let groups = BTreeMap::new();
        let candidates = BTreeMap::new();
        let findings = run_rules(
            &config.rules,
            &entities,
            &ScopeConfig::default(),
            &empty_inputs(&groups, &candidates),
        );
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].dedupe_key, "stale:ENG-1");
    }

    #[test]
    fn findings_sorted_severity_rule_identifier() {
        let toml = r#"
[[hygiene.rules]]
id = "b-medium"
entity = "issue"
[hygiene.rules.when]
updatedAt = { older_than = "1d" }

[[hygiene.rules]]
id = "a-medium"
entity = "issue"
[hygiene.rules.when]
updatedAt = { older_than = "1d" }

[[hygiene.rules]]
id = "z-high"
entity = "issue"
severity = "high"
[hygiene.rules.when]
updatedAt = { older_than = "1d" }
"#;
        let config = rules_from(toml);
        let mut e2 = issue("ENG-2");
        e2.fields.insert("updatedAt".into(), days_ago(5));
        let mut e1 = issue("ENG-1");
        e1.fields.insert("updatedAt".into(), days_ago(5));
        let entities = vec![e2, e1];
        let groups = BTreeMap::new();
        let candidates = BTreeMap::new();
        let findings = run_rules(
            &config.rules,
            &entities,
            &ScopeConfig::default(),
            &empty_inputs(&groups, &candidates),
        );
        let keys: Vec<&str> = findings.iter().map(|f| f.dedupe_key.as_str()).collect();
        assert_eq!(
            keys,
            vec![
                "z-high:ENG-1",
                "z-high:ENG-2",
                "a-medium:ENG-1",
                "a-medium:ENG-2",
                "b-medium:ENG-1",
                "b-medium:ENG-2",
            ]
        );
    }

    #[test]
    fn filter_snoozed_removes_active_keys() {
        let mut entity = issue("ENG-1");
        entity.fields.insert("updatedAt".into(), days_ago(5));
        let toml = "[[hygiene.rules]]\nid=\"r\"\nentity=\"issue\"\n[hygiene.rules.when]\nupdatedAt = { older_than = \"1d\" }\n";
        let finding = eval_single(toml, &entity).unwrap();
        let mut snoozed = BTreeSet::new();
        snoozed.insert("r:ENG-1".to_string());
        assert!(filter_snoozed(vec![finding.clone()], &snoozed).is_empty());
        assert_eq!(filter_snoozed(vec![finding], &BTreeSet::new()).len(), 1);
    }

    #[test]
    fn format_duration_compact_units() {
        assert_eq!(format_duration_compact(45), "45s");
        assert_eq!(format_duration_compact(120), "2m");
        assert_eq!(format_duration_compact(7_200), "2h");
        assert_eq!(format_duration_compact(6 * 86_400), "6d");
    }
}
