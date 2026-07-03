//! `hygiene.toml` configuration: types, parsing, validation, and merge with
//! the repo-level `.linear.toml` `[hygiene]` scope override (R1–R9, R30).
//!
//! Parsing walks the raw TOML value manually so that *all* validation errors
//! are collected and reported together (R7/R22), each naming the rule id and
//! offending key.

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::dates::parse_duration_seconds;
use crate::hygiene::model::{self, EntityKind, FieldType};

/// Default TTL for trusting the last-run artifact in `hygiene apply` (R33).
pub const DEFAULT_APPLY_TTL: &str = "30m";
/// Default exempt label (R6).
pub const DEFAULT_EXEMPT_LABEL: &str = "ignore-audit";

/// Finding/rule severity (R5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    High,
    Medium,
    Low,
}

impl Severity {
    /// Sort rank: high sorts first (R18).
    pub fn rank(&self) -> u8 {
        match self {
            Severity::High => 0,
            Severity::Medium => 1,
            Severity::Low => 2,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::High => "high",
            Severity::Medium => "medium",
            Severity::Low => "low",
        }
    }

    pub fn parse(input: &str) -> Option<Self> {
        match input {
            "high" => Some(Severity::High),
            "medium" => Some(Severity::Medium),
            "low" => Some(Severity::Low),
            _ => None,
        }
    }
}

/// `[hygiene.scope]` (R6).
#[derive(Debug, Clone, PartialEq)]
pub struct ScopeConfig {
    pub exempt_labels: Vec<String>,
    pub teams: Vec<String>,
    pub include_archived: bool,
}

impl Default for ScopeConfig {
    fn default() -> Self {
        ScopeConfig {
            exempt_labels: vec![DEFAULT_EXEMPT_LABEL.to_string()],
            teams: Vec::new(),
            include_archived: false,
        }
    }
}

/// Partial scope from a repo-level `.linear.toml` `[hygiene]` table (R2).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScopeOverride {
    pub exempt_labels: Option<Vec<String>>,
    pub teams: Option<Vec<String>>,
    pub include_archived: Option<bool>,
}

impl ScopeOverride {
    /// Part of the override API surface; currently exercised by unit tests.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.exempt_labels.is_none() && self.teams.is_none() && self.include_archived.is_none()
    }
}

/// Merge a repo scope override onto the user scope (project overrides user).
pub fn merge_scope(base: &ScopeConfig, over: &ScopeOverride) -> ScopeConfig {
    ScopeConfig {
        exempt_labels: over
            .exempt_labels
            .clone()
            .unwrap_or_else(|| base.exempt_labels.clone()),
        teams: over.teams.clone().unwrap_or_else(|| base.teams.clone()),
        include_archived: over.include_archived.unwrap_or(base.include_archived),
    }
}

/// A scalar value in a `fix.set` assignment.
#[derive(Debug, Clone, PartialEq)]
pub enum SetValue {
    String(String),
    Number(f64),
}

impl SetValue {
    pub fn display(&self) -> String {
        match self {
            SetValue::String(s) => s.clone(),
            SetValue::Number(n) => model::format_number(*n),
        }
    }
}

/// A named option in a config-declared `options` fix (R30).
#[derive(Debug, Clone, PartialEq)]
pub struct FixOptionConfig {
    pub action: String,
    /// Field assignments applied by this option (may be empty when
    /// `comment = true`).
    pub set: Vec<(String, SetValue)>,
    /// `comment = true` marks the option as needing authored content.
    pub comment: bool,
}

/// A `[fix]` block on a `when`-rule (R30).
#[derive(Debug, Clone, PartialEq)]
pub enum FixConfig {
    /// `set = { field = "value" }` → deterministic `command`-kind fix.
    Set(Vec<(String, SetValue)>),
    /// Named `options` → `options`-kind fix (single option with
    /// `comment = true` degrades to `needs_input`).
    Options(Vec<FixOptionConfig>),
}

/// One predicate condition (§4.2). Conditions in a rule AND together (R10).
#[derive(Debug, Clone)]
pub struct Condition {
    pub field: String,
    pub predicate: Predicate,
}

/// Predicate operators (§4.2). Durations keep their original spelling for
/// evidence/summary output.
#[derive(Debug, Clone)]
pub enum Predicate {
    In(Vec<String>),
    NotIn(Vec<String>),
    Missing(bool),
    OlderThan { seconds: u64, raw: String },
    NewerThan { seconds: u64, raw: String },
    Past(bool),
    ShorterThan(u64),
    LongerThan(u64),
    Matches(Regex),
    NotMatches(Regex),
    Lt(f64),
    Lte(f64),
    Gt(f64),
    Gte(f64),
    Eq(f64),
    MissingGroup(String),
}

impl Predicate {
    pub fn operator_name(&self) -> &'static str {
        match self {
            Predicate::In(_) => "in",
            Predicate::NotIn(_) => "not_in",
            Predicate::Missing(_) => "missing",
            Predicate::OlderThan { .. } => "older_than",
            Predicate::NewerThan { .. } => "newer_than",
            Predicate::Past(_) => "past",
            Predicate::ShorterThan(_) => "shorter_than",
            Predicate::LongerThan(_) => "longer_than",
            Predicate::Matches(_) => "matches",
            Predicate::NotMatches(_) => "not_matches",
            Predicate::Lt(_) => "lt",
            Predicate::Lte(_) => "lte",
            Predicate::Gt(_) => "gt",
            Predicate::Gte(_) => "gte",
            Predicate::Eq(_) => "eq",
            Predicate::MissingGroup(_) => "missing_group",
        }
    }
}

/// A validated `when`-rule.
#[derive(Debug, Clone)]
pub struct WhenRule {
    pub id: String,
    pub entity: EntityKind,
    pub severity: Severity,
    pub enabled: bool,
    pub when: Vec<Condition>,
    pub fix: Option<FixConfig>,
}

/// Builtin rule kinds (§4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinKind {
    WipLimit,
    InitiativeCompletedButActive,
    ProjectSingleIssue,
    ProjectNoTargetDateLongLived,
}

impl BuiltinKind {
    pub fn name(&self) -> &'static str {
        match self {
            BuiltinKind::WipLimit => "wip-limit",
            BuiltinKind::InitiativeCompletedButActive => "initiative-completed-but-active",
            BuiltinKind::ProjectSingleIssue => "project-single-issue",
            BuiltinKind::ProjectNoTargetDateLongLived => "project-no-target-date-long-lived",
        }
    }

    pub fn parse(input: &str) -> Option<Self> {
        match input {
            "wip-limit" => Some(BuiltinKind::WipLimit),
            "initiative-completed-but-active" => Some(BuiltinKind::InitiativeCompletedButActive),
            "project-single-issue" => Some(BuiltinKind::ProjectSingleIssue),
            "project-no-target-date-long-lived" => Some(BuiltinKind::ProjectNoTargetDateLongLived),
            _ => None,
        }
    }

    pub fn entity(&self) -> EntityKind {
        match self {
            BuiltinKind::WipLimit => EntityKind::Issue,
            BuiltinKind::InitiativeCompletedButActive => EntityKind::Initiative,
            BuiltinKind::ProjectSingleIssue | BuiltinKind::ProjectNoTargetDateLongLived => {
                EntityKind::Project
            }
        }
    }
}

/// Typed builtin params with defaults applied (§4.3).
#[derive(Debug, Clone, PartialEq)]
pub enum BuiltinParams {
    WipLimit {
        max_in_progress_per_assignee: u64,
        statuses: Vec<String>,
    },
    InitiativeCompletedButActive,
    ProjectSingleIssue {
        min_issues: u64,
        min_age_seconds: u64,
        min_age: String,
    },
    ProjectNoTargetDateLongLived {
        min_age_seconds: u64,
        min_age: String,
    },
}

/// A validated builtin rule.
#[derive(Debug, Clone)]
pub struct BuiltinRule {
    pub id: String,
    pub kind: BuiltinKind,
    pub severity: Severity,
    pub enabled: bool,
    pub params: BuiltinParams,
}

/// A validated rule of either kind (R3).
#[derive(Debug, Clone)]
pub enum Rule {
    When(WhenRule),
    Builtin(BuiltinRule),
}

impl Rule {
    pub fn id(&self) -> &str {
        match self {
            Rule::When(r) => &r.id,
            Rule::Builtin(r) => &r.id,
        }
    }

    pub fn severity(&self) -> Severity {
        match self {
            Rule::When(r) => r.severity,
            Rule::Builtin(r) => r.severity,
        }
    }

    pub fn enabled(&self) -> bool {
        match self {
            Rule::When(r) => r.enabled,
            Rule::Builtin(r) => r.enabled,
        }
    }

    pub fn entity(&self) -> EntityKind {
        match self {
            Rule::When(r) => r.entity,
            Rule::Builtin(r) => r.kind.entity(),
        }
    }
}

/// The fully validated hygiene configuration.
#[derive(Debug, Clone)]
pub struct HygieneConfig {
    pub scope: ScopeConfig,
    /// Raw `apply_ttl` value (e.g. `"30m"`).
    pub apply_ttl: String,
    pub apply_ttl_seconds: u64,
    pub rules: Vec<Rule>,
}

impl Default for HygieneConfig {
    fn default() -> Self {
        HygieneConfig {
            scope: ScopeConfig::default(),
            apply_ttl: DEFAULT_APPLY_TTL.to_string(),
            apply_ttl_seconds: parse_duration_seconds(DEFAULT_APPLY_TTL).unwrap(),
            rules: Vec::new(),
        }
    }
}

impl HygieneConfig {
    pub fn enabled_rules(&self) -> impl Iterator<Item = &Rule> {
        self.rules.iter().filter(|r| r.enabled())
    }
}

// ---------------------------------------------------------------------------
// Parsing / validation
// ---------------------------------------------------------------------------

type Errors = Vec<String>;

/// Parse and validate the user-level `hygiene.toml`. On failure returns *all*
/// errors (R7/R22).
pub fn parse_hygiene_toml(content: &str) -> Result<HygieneConfig, Errors> {
    let root: toml::Value =
        toml::from_str(content).map_err(|e| vec![format!("invalid TOML: {e}")])?;
    let mut errors = Errors::new();

    let Some(root_table) = root.as_table() else {
        return Err(vec!["hygiene.toml: expected a TOML table".to_string()]);
    };
    for key in root_table.keys() {
        if key != "hygiene" {
            errors.push(format!(
                "hygiene.toml: unknown top-level key '{key}' (expected only [hygiene])"
            ));
        }
    }
    let Some(hygiene) = root_table.get("hygiene") else {
        return if errors.is_empty() {
            Ok(HygieneConfig::default())
        } else {
            Err(errors)
        };
    };
    let Some(hygiene) = hygiene.as_table() else {
        errors.push("hygiene: expected a table".to_string());
        return Err(errors);
    };

    let mut config = HygieneConfig::default();

    for key in hygiene.keys() {
        if !matches!(key.as_str(), "scope" | "apply_ttl" | "rules") {
            errors.push(format!("hygiene: unknown key '{key}'"));
        }
    }

    if let Some(scope_value) = hygiene.get("scope") {
        if let Some(over) = parse_scope_table(scope_value, "hygiene.scope", &mut errors) {
            config.scope = merge_scope(&ScopeConfig::default(), &over);
        }
    }

    if let Some(ttl) = hygiene.get("apply_ttl") {
        match ttl.as_str() {
            Some(raw) => match parse_duration_seconds(raw) {
                Some(seconds) => {
                    config.apply_ttl = raw.to_string();
                    config.apply_ttl_seconds = seconds;
                }
                None => errors.push(format!(
                    "hygiene.apply_ttl: invalid duration '{raw}' (use 90m, 6h, 7d, 2w)"
                )),
            },
            None => errors.push("hygiene.apply_ttl: expected a duration string".to_string()),
        }
    }

    if let Some(rules_value) = hygiene.get("rules") {
        match rules_value.as_array() {
            Some(rules) => {
                for (index, rule_value) in rules.iter().enumerate() {
                    if let Some(rule) = parse_rule(rule_value, index, &mut errors) {
                        config.rules.push(rule);
                    }
                }
            }
            None => errors.push("hygiene.rules: expected an array of tables".to_string()),
        }
    }

    // Duplicate ids are a config error (R4).
    let mut seen = std::collections::BTreeSet::new();
    for rule in &config.rules {
        if !seen.insert(rule.id().to_string()) {
            errors.push(format!("rule '{}': duplicate rule id", rule.id()));
        }
    }

    if errors.is_empty() {
        Ok(config)
    } else {
        Err(errors)
    }
}

fn parse_scope_table(
    value: &toml::Value,
    context: &str,
    errors: &mut Errors,
) -> Option<ScopeOverride> {
    let Some(table) = value.as_table() else {
        errors.push(format!("{context}: expected a table"));
        return None;
    };
    let mut over = ScopeOverride::default();
    for (key, entry) in table {
        match key.as_str() {
            "exempt_labels" => {
                over.exempt_labels =
                    parse_string_array(entry, &format!("{context}.exempt_labels"), errors)
            }
            "teams" => over.teams = parse_string_array(entry, &format!("{context}.teams"), errors),
            "include_archived" => match entry.as_bool() {
                Some(flag) => over.include_archived = Some(flag),
                None => errors.push(format!("{context}.include_archived: expected a boolean")),
            },
            other => errors.push(format!("{context}: unknown key '{other}'")),
        }
    }
    Some(over)
}

fn parse_string_array(
    value: &toml::Value,
    context: &str,
    errors: &mut Errors,
) -> Option<Vec<String>> {
    let Some(items) = value.as_array() else {
        errors.push(format!("{context}: expected an array of strings"));
        return None;
    };
    let mut out = Vec::new();
    for item in items {
        match item.as_str() {
            Some(s) => out.push(s.to_string()),
            None => {
                errors.push(format!("{context}: expected an array of strings"));
                return None;
            }
        }
    }
    Some(out)
}

fn parse_rule(value: &toml::Value, index: usize, errors: &mut Errors) -> Option<Rule> {
    let Some(table) = value.as_table() else {
        errors.push(format!("hygiene.rules[{index}]: expected a table"));
        return None;
    };
    if table.contains_key("builtin") {
        parse_builtin_rule(table, index, errors)
    } else {
        parse_when_rule(table, index, errors)
    }
}

fn rule_label(table: &toml::map::Map<String, toml::Value>, index: usize) -> String {
    table
        .get("id")
        .and_then(|v| v.as_str())
        .map(|id| format!("rule '{id}'"))
        .unwrap_or_else(|| format!("hygiene.rules[{index}]"))
}

fn parse_severity(
    table: &toml::map::Map<String, toml::Value>,
    label: &str,
    errors: &mut Errors,
) -> Severity {
    match table.get("severity") {
        None => Severity::Medium,
        Some(value) => match value.as_str().and_then(Severity::parse) {
            Some(sev) => sev,
            None => {
                errors.push(format!(
                    "{label}: invalid severity '{}' (expected high, medium, or low)",
                    display_toml(value)
                ));
                Severity::Medium
            }
        },
    }
}

fn parse_enabled(
    table: &toml::map::Map<String, toml::Value>,
    label: &str,
    errors: &mut Errors,
) -> bool {
    match table.get("enabled") {
        None => true,
        Some(value) => match value.as_bool() {
            Some(flag) => flag,
            None => {
                errors.push(format!("{label}: 'enabled' must be a boolean"));
                true
            }
        },
    }
}

fn parse_builtin_rule(
    table: &toml::map::Map<String, toml::Value>,
    index: usize,
    errors: &mut Errors,
) -> Option<Rule> {
    let label = rule_label(table, index);
    let before = errors.len();

    let builtin_name = match table.get("builtin").and_then(|v| v.as_str()) {
        Some(name) => name.to_string(),
        None => {
            errors.push(format!("{label}: 'builtin' must be a string"));
            return None;
        }
    };
    let Some(kind) = BuiltinKind::parse(&builtin_name) else {
        errors.push(format!("{label}: unknown builtin '{builtin_name}'"));
        return None;
    };
    let id = table
        .get("id")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| builtin_name.clone());
    let label = format!("rule '{id}'");

    for key in table.keys() {
        if !matches!(
            key.as_str(),
            "builtin" | "id" | "severity" | "enabled" | "params"
        ) {
            errors.push(format!("{label}: unknown key '{key}' for a builtin rule"));
        }
    }
    let severity = parse_severity(table, &label, errors);
    let enabled = parse_enabled(table, &label, errors);
    let params = parse_builtin_params(kind, table.get("params"), &label, errors);

    if errors.len() > before {
        return None;
    }
    Some(Rule::Builtin(BuiltinRule {
        id,
        kind,
        severity,
        enabled,
        params: params?,
    }))
}

fn parse_builtin_params(
    kind: BuiltinKind,
    params: Option<&toml::Value>,
    label: &str,
    errors: &mut Errors,
) -> Option<BuiltinParams> {
    let empty = toml::map::Map::new();
    let table = match params {
        None => &empty,
        Some(value) => match value.as_table() {
            Some(table) => table,
            None => {
                errors.push(format!("{label}: 'params' must be a table"));
                return None;
            }
        },
    };
    let before = errors.len();

    let allowed: &[&str] = match kind {
        BuiltinKind::WipLimit => &["max_in_progress_per_assignee", "statuses"],
        BuiltinKind::InitiativeCompletedButActive => &[],
        BuiltinKind::ProjectSingleIssue => &["min_issues", "min_age"],
        BuiltinKind::ProjectNoTargetDateLongLived => &["min_age"],
    };
    for key in table.keys() {
        if !allowed.contains(&key.as_str()) {
            errors.push(format!(
                "{label}: unknown param '{key}' for builtin '{}'",
                kind.name()
            ));
        }
    }

    let positive_int = |key: &str, default: u64, errors: &mut Errors| -> u64 {
        match table.get(key) {
            None => default,
            Some(value) => match value.as_integer() {
                Some(n) if n >= 1 => n as u64,
                _ => {
                    errors.push(format!("{label}: param '{key}' must be a positive integer"));
                    default
                }
            },
        }
    };
    let duration = |key: &str, default: &str, errors: &mut Errors| -> (u64, String) {
        match table.get(key) {
            None => (
                parse_duration_seconds(default).unwrap(),
                default.to_string(),
            ),
            Some(value) => match value.as_str().and_then(|raw| {
                parse_duration_seconds(raw).map(|seconds| (seconds, raw.to_string()))
            }) {
                Some(parsed) => parsed,
                None => {
                    errors.push(format!(
                        "{label}: param '{key}' must be a duration string (90m, 6h, 7d, 2w)"
                    ));
                    (
                        parse_duration_seconds(default).unwrap(),
                        default.to_string(),
                    )
                }
            },
        }
    };

    let params = match kind {
        BuiltinKind::WipLimit => {
            let statuses = match table.get("statuses") {
                None => vec!["In Progress".to_string()],
                Some(value) => {
                    match parse_string_array(value, &format!("{label}: param 'statuses'"), errors) {
                        Some(list) if !list.is_empty() => list,
                        Some(_) => {
                            errors.push(format!("{label}: param 'statuses' must not be empty"));
                            vec!["In Progress".to_string()]
                        }
                        None => vec!["In Progress".to_string()],
                    }
                }
            };
            BuiltinParams::WipLimit {
                max_in_progress_per_assignee: positive_int(
                    "max_in_progress_per_assignee",
                    3,
                    errors,
                ),
                statuses,
            }
        }
        BuiltinKind::InitiativeCompletedButActive => BuiltinParams::InitiativeCompletedButActive,
        BuiltinKind::ProjectSingleIssue => {
            let (min_age_seconds, min_age) = duration("min_age", "14d", errors);
            BuiltinParams::ProjectSingleIssue {
                min_issues: positive_int("min_issues", 2, errors),
                min_age_seconds,
                min_age,
            }
        }
        BuiltinKind::ProjectNoTargetDateLongLived => {
            let (min_age_seconds, min_age) = duration("min_age", "60d", errors);
            BuiltinParams::ProjectNoTargetDateLongLived {
                min_age_seconds,
                min_age,
            }
        }
    };

    if errors.len() > before {
        None
    } else {
        Some(params)
    }
}

fn parse_when_rule(
    table: &toml::map::Map<String, toml::Value>,
    index: usize,
    errors: &mut Errors,
) -> Option<Rule> {
    let label = rule_label(table, index);
    let before = errors.len();

    let id = match table.get("id").and_then(|v| v.as_str()) {
        Some(id) if !id.trim().is_empty() => id.to_string(),
        _ => {
            errors.push(format!("{label}: missing required 'id'"));
            String::new()
        }
    };

    for key in table.keys() {
        if !matches!(
            key.as_str(),
            "id" | "entity" | "severity" | "enabled" | "when" | "fix"
        ) {
            errors.push(format!("{label}: unknown key '{key}'"));
        }
    }

    let entity = match table.get("entity").and_then(|v| v.as_str()) {
        Some(raw) => match EntityKind::parse(raw) {
            Some(kind) => Some(kind),
            None => {
                errors.push(format!(
                    "{label}: invalid entity '{raw}' (expected issue, project, or initiative)"
                ));
                None
            }
        },
        None => {
            errors.push(format!(
                "{label}: missing required 'entity' (issue, project, or initiative)"
            ));
            None
        }
    };

    let severity = parse_severity(table, &label, errors);
    let enabled = parse_enabled(table, &label, errors);

    let mut conditions = Vec::new();
    match table.get("when") {
        Some(when_value) => match when_value.as_table() {
            Some(when_table) if !when_table.is_empty() => {
                if let Some(kind) = entity {
                    for (field, ops) in when_table {
                        conditions.extend(parse_conditions(kind, field, ops, &label, errors));
                    }
                }
            }
            Some(_) => errors.push(format!("{label}: 'when' must not be empty")),
            None => errors.push(format!("{label}: 'when' must be a table")),
        },
        None => errors.push(format!("{label}: missing required 'when' table")),
    }

    let fix = match (table.get("fix"), entity) {
        (Some(fix_value), Some(kind)) => parse_fix(fix_value, kind, &label, errors),
        (Some(_), None) => None,
        (None, _) => None,
    };

    if errors.len() > before {
        return None;
    }
    Some(Rule::When(WhenRule {
        id,
        entity: entity?,
        severity,
        enabled,
        when: conditions,
        fix,
    }))
}

fn parse_conditions(
    kind: EntityKind,
    field: &str,
    ops: &toml::Value,
    label: &str,
    errors: &mut Errors,
) -> Vec<Condition> {
    let Some(spec) = model::field_spec(kind, field) else {
        errors.push(format!(
            "{label}: unknown field '{field}' for entity '{kind}' (see `hygiene rules --schema`)"
        ));
        return Vec::new();
    };
    let Some(op_table) = ops.as_table() else {
        errors.push(format!(
            "{label}: condition on '{field}' must be a table like {{ older_than = \"2d\" }}"
        ));
        return Vec::new();
    };
    if op_table.is_empty() {
        errors.push(format!("{label}: condition on '{field}' has no operator"));
        return Vec::new();
    }

    let mut conditions = Vec::new();
    for (op, value) in op_table {
        if let Some(predicate) = parse_predicate(op, value, field, spec, label, errors) {
            // `priority` exposes Linear's 0 ("no priority") as missing, so a
            // numeric comparison against 0 can never match — reject the trap
            // instead of silently producing zero findings.
            if field == "priority" && matches!(predicate, Predicate::Eq(n) if n == 0.0) {
                errors.push(format!(
                    "{label}: 'priority = {{ eq = 0 }}' never matches (Linear's 0 means unset \
                     and is exposed as missing); use 'priority = {{ missing = true }}'"
                ));
                continue;
            }
            conditions.push(Condition {
                field: field.to_string(),
                predicate,
            });
        }
    }
    conditions
}

fn parse_predicate(
    op: &str,
    value: &toml::Value,
    field: &str,
    spec: &model::FieldSpec,
    label: &str,
    errors: &mut Errors,
) -> Option<Predicate> {
    let type_error = |errors: &mut Errors, expected: &str| {
        errors.push(format!(
            "{label}: operator '{op}' on '{field}' expects {expected}"
        ));
    };
    let applicability_error = |errors: &mut Errors, applies: &str| {
        errors.push(format!(
            "{label}: operator '{op}' does not apply to '{field}' ({} field; applies to {applies})",
            spec.ty.as_str()
        ));
    };

    match op {
        "in" | "not_in" => {
            if !matches!(spec.ty, FieldType::String | FieldType::StringList) {
                applicability_error(errors, "string fields");
                return None;
            }
            let list = parse_string_array(value, &format!("{label}: '{op}' on '{field}'"), errors)?;
            Some(if op == "in" {
                Predicate::In(list)
            } else {
                Predicate::NotIn(list)
            })
        }
        "missing" => {
            if !spec.nullable && spec.ty != FieldType::StringList {
                applicability_error(errors, "nullable fields");
                return None;
            }
            match value.as_bool() {
                Some(flag) => Some(Predicate::Missing(flag)),
                None => {
                    type_error(errors, "a boolean");
                    None
                }
            }
        }
        "older_than" | "newer_than" => {
            if !matches!(spec.ty, FieldType::DateTime | FieldType::Date) {
                applicability_error(errors, "datetime/date fields");
                return None;
            }
            let raw = match value.as_str() {
                Some(raw) => raw,
                None => {
                    type_error(errors, "a duration string (90m, 6h, 7d, 2w)");
                    return None;
                }
            };
            match parse_duration_seconds(raw) {
                Some(seconds) => Some(if op == "older_than" {
                    Predicate::OlderThan {
                        seconds,
                        raw: raw.to_string(),
                    }
                } else {
                    Predicate::NewerThan {
                        seconds,
                        raw: raw.to_string(),
                    }
                }),
                None => {
                    errors.push(format!(
                        "{label}: invalid duration '{raw}' for '{op}' on '{field}' (use 90m, 6h, 7d, 2w)"
                    ));
                    None
                }
            }
        }
        "past" => {
            if spec.ty != FieldType::Date {
                applicability_error(errors, "date fields");
                return None;
            }
            match value.as_bool() {
                Some(flag) => Some(Predicate::Past(flag)),
                None => {
                    type_error(errors, "a boolean");
                    None
                }
            }
        }
        "shorter_than" | "longer_than" => {
            if spec.ty != FieldType::String {
                applicability_error(errors, "string fields");
                return None;
            }
            match value.as_integer() {
                Some(n) if n >= 0 => Some(if op == "shorter_than" {
                    Predicate::ShorterThan(n as u64)
                } else {
                    Predicate::LongerThan(n as u64)
                }),
                _ => {
                    type_error(errors, "a non-negative integer");
                    None
                }
            }
        }
        "matches" | "not_matches" => {
            if spec.ty != FieldType::String {
                applicability_error(errors, "string fields");
                return None;
            }
            let pattern = match value.as_str() {
                Some(p) => p,
                None => {
                    type_error(errors, "a regex string");
                    return None;
                }
            };
            match Regex::new(pattern) {
                Ok(regex) => Some(if op == "matches" {
                    Predicate::Matches(regex)
                } else {
                    Predicate::NotMatches(regex)
                }),
                Err(e) => {
                    errors.push(format!(
                        "{label}: invalid regex for '{op}' on '{field}': {e}"
                    ));
                    None
                }
            }
        }
        "lt" | "lte" | "gt" | "gte" | "eq" => {
            if spec.ty != FieldType::Number {
                applicability_error(errors, "number fields");
                return None;
            }
            let number = value
                .as_float()
                .or_else(|| value.as_integer().map(|n| n as f64));
            match number {
                Some(n) => Some(match op {
                    "lt" => Predicate::Lt(n),
                    "lte" => Predicate::Lte(n),
                    "gt" => Predicate::Gt(n),
                    "gte" => Predicate::Gte(n),
                    _ => Predicate::Eq(n),
                }),
                None => {
                    type_error(errors, "a number");
                    None
                }
            }
        }
        "missing_group" => {
            if field != "labels" {
                errors.push(format!(
                    "{label}: operator 'missing_group' only applies to 'labels' (got '{field}')"
                ));
                return None;
            }
            match value.as_str() {
                Some(group) if !group.trim().is_empty() => {
                    Some(Predicate::MissingGroup(group.to_string()))
                }
                _ => {
                    type_error(errors, "a label group name");
                    None
                }
            }
        }
        unknown => {
            errors.push(format!(
                "{label}: unknown operator '{unknown}' on '{field}' (see `hygiene rules --schema`)"
            ));
            None
        }
    }
}

fn parse_fix(
    value: &toml::Value,
    entity: EntityKind,
    label: &str,
    errors: &mut Errors,
) -> Option<FixConfig> {
    let Some(table) = value.as_table() else {
        errors.push(format!("{label}: 'fix' must be a table"));
        return None;
    };
    for key in table.keys() {
        if !matches!(key.as_str(), "set" | "options") {
            errors.push(format!("{label}: unknown key 'fix.{key}'"));
        }
    }
    match (table.get("set"), table.get("options")) {
        (Some(_), Some(_)) => {
            errors.push(format!(
                "{label}: 'fix' must declare either 'set' or 'options', not both"
            ));
            None
        }
        (Some(set_value), None) => {
            let set = parse_set_table(set_value, entity, label, "fix.set", errors)?;
            if set.is_empty() {
                errors.push(format!("{label}: 'fix.set' must not be empty"));
                return None;
            }
            Some(FixConfig::Set(set))
        }
        (None, Some(options_value)) => {
            let options = parse_fix_options(options_value, entity, label, errors)?;
            Some(FixConfig::Options(options))
        }
        (None, None) => {
            errors.push(format!("{label}: 'fix' must declare 'set' or 'options'"));
            None
        }
    }
}

fn parse_set_table(
    value: &toml::Value,
    entity: EntityKind,
    label: &str,
    context: &str,
    errors: &mut Errors,
) -> Option<Vec<(String, SetValue)>> {
    let Some(table) = value.as_table() else {
        errors.push(format!(
            "{label}: '{context}' must be a table of field = value"
        ));
        return None;
    };
    let before = errors.len();
    let mut set = Vec::new();
    for (field, raw) in table {
        if model::settable_field_flag(entity, field).is_none() {
            errors.push(format!(
                "{label}: '{context}' field '{field}' is not settable for entity '{entity}' (settable: {})",
                model::settable_fields(entity).join(", ")
            ));
            continue;
        }
        let value = match raw {
            toml::Value::String(s) => SetValue::String(s.clone()),
            toml::Value::Integer(n) => SetValue::Number(*n as f64),
            toml::Value::Float(n) => SetValue::Number(*n),
            other => {
                errors.push(format!(
                    "{label}: '{context}.{field}' must be a string or number (got {})",
                    display_toml(other)
                ));
                continue;
            }
        };
        set.push((field.clone(), value));
    }
    if errors.len() > before {
        None
    } else {
        Some(set)
    }
}

fn parse_fix_options(
    value: &toml::Value,
    entity: EntityKind,
    label: &str,
    errors: &mut Errors,
) -> Option<Vec<FixOptionConfig>> {
    let Some(items) = value.as_array() else {
        errors.push(format!("{label}: 'fix.options' must be an array of tables"));
        return None;
    };
    if items.is_empty() {
        errors.push(format!("{label}: 'fix.options' must not be empty"));
        return None;
    }
    let before = errors.len();
    let mut options = Vec::new();
    let mut seen_actions = std::collections::BTreeSet::new();
    for (index, item) in items.iter().enumerate() {
        let Some(table) = item.as_table() else {
            errors.push(format!("{label}: 'fix.options[{index}]' must be a table"));
            continue;
        };
        for key in table.keys() {
            if !matches!(key.as_str(), "action" | "set" | "comment") {
                errors.push(format!("{label}: unknown key 'fix.options[{index}].{key}'"));
            }
        }
        let action = match table.get("action").and_then(|v| v.as_str()) {
            Some(action) if !action.trim().is_empty() => action.to_string(),
            _ => {
                errors.push(format!(
                    "{label}: 'fix.options[{index}]' missing required 'action' name"
                ));
                continue;
            }
        };
        if !seen_actions.insert(action.clone()) {
            errors.push(format!("{label}: duplicate fix option action '{action}'"));
        }
        let set = match table.get("set") {
            Some(set_value) => parse_set_table(
                set_value,
                entity,
                label,
                &format!("fix.options[{index}].set"),
                errors,
            )
            .unwrap_or_default(),
            None => Vec::new(),
        };
        let comment = match table.get("comment") {
            None => false,
            Some(value) => match value.as_bool() {
                Some(flag) => flag,
                None => {
                    errors.push(format!(
                        "{label}: 'fix.options[{index}].comment' must be a boolean"
                    ));
                    false
                }
            },
        };
        if comment && entity != EntityKind::Issue {
            errors.push(format!(
                "{label}: 'fix.options[{index}].comment' is only supported for issue rules"
            ));
        }
        if set.is_empty() && !comment {
            errors.push(format!(
                "{label}: 'fix.options[{index}]' must declare 'set' and/or 'comment = true'"
            ));
        }
        options.push(FixOptionConfig {
            action,
            set,
            comment,
        });
    }
    if errors.len() > before {
        None
    } else {
        Some(options)
    }
}

fn display_toml(value: &toml::Value) -> String {
    match value {
        toml::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Repo-level `.linear.toml` override (scope only, R2)
// ---------------------------------------------------------------------------

/// Parse the `[hygiene]` table of a repo `.linear.toml`, allowing scope
/// overrides only. Rule definitions there are a config error (R2).
pub fn parse_repo_hygiene_override(content: &str) -> Result<Option<ScopeOverride>, Errors> {
    let root: toml::Value =
        toml::from_str(content).map_err(|e| vec![format!(".linear.toml: invalid TOML: {e}")])?;
    let Some(hygiene) = root.get("hygiene") else {
        return Ok(None);
    };
    let mut errors = Errors::new();
    let Some(table) = hygiene.as_table() else {
        return Err(vec![".linear.toml [hygiene]: expected a table".to_string()]);
    };
    let mut over = ScopeOverride::default();
    for (key, value) in table {
        match key.as_str() {
            "scope" => {
                if let Some(parsed) =
                    parse_scope_table(value, ".linear.toml [hygiene.scope]", &mut errors)
                {
                    over = parsed;
                }
            }
            "rules" => errors.push(
                ".linear.toml [hygiene]: rule definitions are not allowed in the repo file; \
                 define rules in the user-level hygiene.toml"
                    .to_string(),
            ),
            other => errors.push(format!(
                ".linear.toml [hygiene]: unknown key '{other}' (only 'scope' may be overridden)"
            )),
        }
    }
    if errors.is_empty() {
        Ok(Some(over))
    } else {
        Err(errors)
    }
}

// ---------------------------------------------------------------------------
// Loading (I/O)
// ---------------------------------------------------------------------------

/// Env var naming an alternate hygiene rules file. This is a config *path*
/// (not a secret), so a read-only env fallback is fine here.
pub const HYGIENE_RULES_ENV: &str = "LINEAR_CLI_HYGIENE_RULES";

/// Path of the user-level `hygiene.toml`, next to `config.toml` (R1).
pub fn hygiene_config_path() -> Result<PathBuf> {
    Ok(crate::config::linear_config_dir()?.join("hygiene.toml"))
}

/// Where the effective hygiene rules file comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RulesSource {
    /// Explicit `--rules PATH` flag.
    Flag(PathBuf),
    /// `LINEAR_CLI_HYGIENE_RULES` env var.
    Env(PathBuf),
    /// Default user-level `hygiene.toml`.
    Default(PathBuf),
}

impl RulesSource {
    pub fn path(&self) -> &Path {
        match self {
            RulesSource::Flag(p) | RulesSource::Env(p) | RulesSource::Default(p) => p,
        }
    }

    /// Override paths must exist; only the default path may be absent (R8).
    pub fn is_override(&self) -> bool {
        !matches!(self, RulesSource::Default(_))
    }

    fn origin(&self) -> &'static str {
        match self {
            RulesSource::Flag(_) => "--rules",
            RulesSource::Env(_) => HYGIENE_RULES_ENV,
            RulesSource::Default(_) => "default",
        }
    }
}

/// Pure precedence: `--rules` flag > `LINEAR_CLI_HYGIENE_RULES` > default
/// user-level path. Blank env values are ignored.
pub fn resolve_rules_source(
    flag: Option<PathBuf>,
    env: Option<String>,
    default: PathBuf,
) -> RulesSource {
    if let Some(path) = flag {
        return RulesSource::Flag(path);
    }
    if let Some(raw) = env {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return RulesSource::Env(PathBuf::from(trimmed));
        }
    }
    RulesSource::Default(default)
}

/// Resolve the effective rules file for this invocation from the `--rules`
/// flag, the env var, and the default user-level path.
pub fn hygiene_rules_source(flag: Option<&Path>) -> Result<RulesSource> {
    Ok(resolve_rules_source(
        flag.map(Path::to_path_buf),
        std::env::var(HYGIENE_RULES_ENV).ok(),
        hygiene_config_path()?,
    ))
}

fn errors_to_anyhow(source: &str, errors: Errors) -> anyhow::Error {
    anyhow::anyhow!(
        "invalid hygiene config in {source}:\n  - {}",
        errors.join("\n  - ")
    )
}

/// Load the rules file named by `source`. For the default path a missing file
/// means "no rules" (`Ok(None)`, R8); for an override path (`--rules` or the
/// env var) a missing file is an error naming the path. Validation failures
/// list all errors (R22).
pub fn load_user_hygiene_config(source: &RulesSource) -> Result<Option<HygieneConfig>> {
    let path = source.path();
    if !path.exists() {
        if source.is_override() {
            // Worded to avoid the "not found" exit-2 classifier: a bad
            // override path is a general config error (exit 1).
            anyhow::bail!(
                "hygiene rules file is missing: {} (from {})",
                path.display(),
                source.origin()
            );
        }
        return Ok(None);
    }
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    parse_hygiene_toml(&content)
        .map(Some)
        .map_err(|errors| errors_to_anyhow(&path.display().to_string(), errors))
}

/// Load the effective config: the resolved rules file with the repo
/// `.linear.toml` `[hygiene]` scope override applied (R2) — the repo scope
/// override applies regardless of where the rules file came from. Returns
/// `Ok(None)` when no config exists at the default path.
pub fn load_effective_hygiene_config(rules_flag: Option<&Path>) -> Result<Option<HygieneConfig>> {
    let source = hygiene_rules_source(rules_flag)?;
    let Some(mut config) = load_user_hygiene_config(&source)? else {
        return Ok(None);
    };
    if let Some(path) = crate::config::project_context_file_path()? {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        match parse_repo_hygiene_override(&content) {
            Ok(Some(over)) => config.scope = merge_scope(&config.scope, &over),
            Ok(None) => {}
            Err(errors) => return Err(errors_to_anyhow(&path.display().to_string(), errors)),
        }
    }
    Ok(Some(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(content: &str) -> HygieneConfig {
        parse_hygiene_toml(content).expect("expected valid config")
    }

    fn parse_errors(content: &str) -> Vec<String> {
        parse_hygiene_toml(content).expect_err("expected config errors")
    }

    #[test]
    fn empty_config_uses_defaults() {
        let config = parse_ok("");
        assert_eq!(config.scope, ScopeConfig::default());
        assert_eq!(config.scope.exempt_labels, vec![DEFAULT_EXEMPT_LABEL]);
        assert!(!config.scope.include_archived);
        assert_eq!(config.apply_ttl, "30m");
        assert_eq!(config.apply_ttl_seconds, 30 * 60);
        assert!(config.rules.is_empty());
    }

    #[test]
    fn parses_design_doc_examples() {
        let config = parse_ok(
            r#"
[hygiene.scope]
teams = ["ENG"]

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

[[hygiene.rules]]
id = "issue-missing-domain"
entity = "issue"
[hygiene.rules.when]
status = { not_in = ["Triage", "Backlog", "Done"] }
labels = { missing_group = "domain" }

[[hygiene.rules]]
id = "initiative-stale-health"
entity = "initiative"
severity = "high"
[hygiene.rules.when]
state = { in = ["started"] }
healthUpdatedAt = { older_than = "14d" }

[[hygiene.rules]]
builtin = "wip-limit"
severity = "medium"
[hygiene.rules.params]
max_in_progress_per_assignee = 3
"#,
        );
        assert_eq!(config.scope.teams, vec!["ENG"]);
        assert_eq!(config.rules.len(), 5);
        let Rule::When(first) = &config.rules[0] else {
            panic!("expected when rule");
        };
        assert_eq!(first.id, "urgent-in-backlog");
        assert_eq!(first.severity, Severity::High);
        assert!(first.enabled);
        assert_eq!(first.when.len(), 2);
        assert!(matches!(
            first.fix,
            Some(FixConfig::Set(ref set)) if set == &vec![("status".to_string(), SetValue::String("Todo".into()))]
        ));
        let Rule::Builtin(builtin) = &config.rules[4] else {
            panic!("expected builtin rule");
        };
        assert_eq!(builtin.id, "wip-limit");
        assert_eq!(builtin.kind, BuiltinKind::WipLimit);
        assert_eq!(
            builtin.params,
            BuiltinParams::WipLimit {
                max_in_progress_per_assignee: 3,
                statuses: vec!["In Progress".to_string()],
            }
        );
    }

    #[test]
    fn severity_defaults_medium_and_enabled_true() {
        let config = parse_ok(
            r#"
[[hygiene.rules]]
id = "r"
entity = "issue"
[hygiene.rules.when]
status = { in = ["Todo"] }
"#,
        );
        assert_eq!(config.rules[0].severity(), Severity::Medium);
        assert!(config.rules[0].enabled());
    }

    #[test]
    fn rejects_unknown_top_level_and_hygiene_keys() {
        let errors = parse_errors("[not_hygiene]\nx = 1\n");
        assert!(errors[0].contains("unknown top-level key 'not_hygiene'"));

        let errors = parse_errors("[hygiene]\nbogus = true\n");
        assert!(errors[0].contains("unknown key 'bogus'"));
    }

    #[test]
    fn rejects_unknown_scope_key_and_bad_types() {
        let errors = parse_errors("[hygiene.scope]\nteam = \"ENG\"\n");
        assert!(errors[0].contains("hygiene.scope: unknown key 'team'"));

        let errors = parse_errors("[hygiene.scope]\nteams = \"ENG\"\n");
        assert!(errors[0].contains("expected an array of strings"));

        let errors = parse_errors("[hygiene.scope]\ninclude_archived = \"yes\"\n");
        assert!(errors[0].contains("include_archived: expected a boolean"));
    }

    #[test]
    fn rejects_invalid_apply_ttl() {
        let errors = parse_errors("[hygiene]\napply_ttl = \"soon\"\n");
        assert!(errors[0].contains("apply_ttl: invalid duration 'soon'"));
    }

    #[test]
    fn rejects_unknown_rule_key_naming_rule_id() {
        let errors = parse_errors(
            r#"
[[hygiene.rules]]
id = "my-rule"
entity = "issue"
frequency = "daily"
[hygiene.rules.when]
status = { in = ["Todo"] }
"#,
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("rule 'my-rule'") && e.contains("unknown key 'frequency'")),
            "{errors:?}"
        );
    }

    #[test]
    fn rejects_unknown_field_operator_entity_and_missing_bits() {
        let errors = parse_errors(
            r#"
[[hygiene.rules]]
id = "bad"
entity = "spaceship"
[hygiene.rules.when]
status = { in = ["Todo"] }

[[hygiene.rules]]
id = "bad2"
entity = "issue"
[hygiene.rules.when]
lead = { missing = true }

[[hygiene.rules]]
id = "bad3"
entity = "issue"
[hygiene.rules.when]
status = { includes = ["Todo"] }

[[hygiene.rules]]
id = "bad4"
entity = "issue"

[[hygiene.rules]]
entity = "issue"
[hygiene.rules.when]
status = { in = ["Todo"] }
"#,
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("rule 'bad'") && e.contains("invalid entity 'spaceship'")),
            "{errors:?}"
        );
        assert!(
            errors.iter().any(|e| e.contains("rule 'bad2'")
                && e.contains("unknown field 'lead' for entity 'issue'")),
            "{errors:?}"
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("rule 'bad3'") && e.contains("unknown operator 'includes'")),
            "{errors:?}"
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("rule 'bad4'") && e.contains("missing required 'when'")),
            "{errors:?}"
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("hygiene.rules[4]") && e.contains("missing required 'id'")),
            "{errors:?}"
        );
    }

    #[test]
    fn collects_all_errors_together() {
        let errors = parse_errors(
            r#"
[hygiene]
apply_ttl = "nope"

[[hygiene.rules]]
id = "a"
entity = "issue"
[hygiene.rules.when]
updatedAt = { older_than = "sometime" }

[[hygiene.rules]]
id = "b"
entity = "issue"
[hygiene.rules.when]
priority = { eq = "urgent" }
"#,
        );
        assert!(errors.len() >= 3, "{errors:?}");
        assert!(errors.iter().any(|e| e.contains("apply_ttl")));
        assert!(errors
            .iter()
            .any(|e| e.contains("rule 'a'") && e.contains("invalid duration 'sometime'")));
        assert!(errors
            .iter()
            .any(|e| e.contains("rule 'b'") && e.contains("expects a number")));
    }

    #[test]
    fn rejects_operator_type_mismatches() {
        // missing on non-nullable string
        let errors = parse_errors(
            "[[hygiene.rules]]\nid = \"r\"\nentity = \"issue\"\n[hygiene.rules.when]\nstatus = { missing = true }\n",
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("'missing' does not apply to 'status'")),
            "{errors:?}"
        );

        // older_than on a number
        let errors = parse_errors(
            "[[hygiene.rules]]\nid = \"r\"\nentity = \"issue\"\n[hygiene.rules.when]\npriority = { older_than = \"2d\" }\n",
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("'older_than' does not apply to 'priority'")),
            "{errors:?}"
        );

        // past on datetime
        let errors = parse_errors(
            "[[hygiene.rules]]\nid = \"r\"\nentity = \"issue\"\n[hygiene.rules.when]\nupdatedAt = { past = true }\n",
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("'past' does not apply to 'updatedAt'")),
            "{errors:?}"
        );

        // missing_group on non-labels field
        let errors = parse_errors(
            "[[hygiene.rules]]\nid = \"r\"\nentity = \"issue\"\n[hygiene.rules.when]\nstatus = { missing_group = \"domain\" }\n",
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("'missing_group' only applies to 'labels'")),
            "{errors:?}"
        );

        // matches on number
        let errors = parse_errors(
            "[[hygiene.rules]]\nid = \"r\"\nentity = \"issue\"\n[hygiene.rules.when]\nestimate = { matches = \".*\" }\n",
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("'matches' does not apply to 'estimate'")),
            "{errors:?}"
        );
    }

    #[test]
    fn rejects_invalid_regex() {
        let errors = parse_errors(
            "[[hygiene.rules]]\nid = \"r\"\nentity = \"issue\"\n[hygiene.rules.when]\ntitle = { matches = \"[unclosed\" }\n",
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("rule 'r'") && e.contains("invalid regex")),
            "{errors:?}"
        );
    }

    #[test]
    fn rejects_duplicate_rule_ids() {
        let errors = parse_errors(
            r#"
[[hygiene.rules]]
id = "dup"
entity = "issue"
[hygiene.rules.when]
status = { in = ["Todo"] }

[[hygiene.rules]]
id = "dup"
entity = "issue"
[hygiene.rules.when]
status = { in = ["Done"] }
"#,
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("rule 'dup'") && e.contains("duplicate rule id")),
            "{errors:?}"
        );
    }

    #[test]
    fn builtin_id_defaults_and_duplicate_builtins_conflict() {
        let config = parse_ok("[[hygiene.rules]]\nbuiltin = \"wip-limit\"\n");
        assert_eq!(config.rules[0].id(), "wip-limit");
        assert_eq!(config.rules[0].entity(), EntityKind::Issue);

        let errors = parse_errors(
            "[[hygiene.rules]]\nbuiltin = \"wip-limit\"\n\n[[hygiene.rules]]\nbuiltin = \"wip-limit\"\n",
        );
        assert!(
            errors.iter().any(|e| e.contains("duplicate rule id")),
            "{errors:?}"
        );
    }

    #[test]
    fn rejects_unknown_builtin_and_bad_params() {
        let errors = parse_errors("[[hygiene.rules]]\nbuiltin = \"nonsense\"\n");
        assert!(
            errors
                .iter()
                .any(|e| e.contains("unknown builtin 'nonsense'")),
            "{errors:?}"
        );

        let errors = parse_errors(
            "[[hygiene.rules]]\nbuiltin = \"wip-limit\"\n[hygiene.rules.params]\nmax_wip = 3\n",
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("rule 'wip-limit'") && e.contains("unknown param 'max_wip'")),
            "{errors:?}"
        );

        let errors = parse_errors(
            "[[hygiene.rules]]\nbuiltin = \"project-single-issue\"\n[hygiene.rules.params]\nmin_age = 14\n",
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("'min_age' must be a duration string")),
            "{errors:?}"
        );

        let errors = parse_errors(
            "[[hygiene.rules]]\nbuiltin = \"wip-limit\"\n[hygiene.rules.params]\nmax_in_progress_per_assignee = 0\n",
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("must be a positive integer")),
            "{errors:?}"
        );
    }

    #[test]
    fn builtin_param_defaults() {
        let config = parse_ok(
            "[[hygiene.rules]]\nbuiltin = \"project-single-issue\"\n\n[[hygiene.rules]]\nbuiltin = \"project-no-target-date-long-lived\"\n",
        );
        let Rule::Builtin(single) = &config.rules[0] else {
            panic!()
        };
        assert_eq!(
            single.params,
            BuiltinParams::ProjectSingleIssue {
                min_issues: 2,
                min_age_seconds: 14 * 24 * 60 * 60,
                min_age: "14d".to_string(),
            }
        );
        let Rule::Builtin(target) = &config.rules[1] else {
            panic!()
        };
        assert_eq!(
            target.params,
            BuiltinParams::ProjectNoTargetDateLongLived {
                min_age_seconds: 60 * 24 * 60 * 60,
                min_age: "60d".to_string(),
            }
        );
    }

    #[test]
    fn fix_set_validates_settable_fields() {
        let errors = parse_errors(
            r#"
[[hygiene.rules]]
id = "r"
entity = "issue"
[hygiene.rules.when]
status = { in = ["Todo"] }
[hygiene.rules.fix]
set = { team = "ENG" }
"#,
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("rule 'r'") && e.contains("'team' is not settable")),
            "{errors:?}"
        );
    }

    #[test]
    fn fix_options_parse_and_validate() {
        let config = parse_ok(
            r#"
[[hygiene.rules]]
id = "stale-in-review"
entity = "issue"
[hygiene.rules.when]
status = { in = ["In Review"] }
updatedAt = { older_than = "2d" }
[[hygiene.rules.fix.options]]
action = "nudge_review"
comment = true
[[hygiene.rules.fix.options]]
action = "move_back"
set = { status = "In Progress" }
"#,
        );
        let Rule::When(rule) = &config.rules[0] else {
            panic!()
        };
        let Some(FixConfig::Options(options)) = &rule.fix else {
            panic!("expected options fix")
        };
        assert_eq!(options.len(), 2);
        assert!(options[0].comment);
        assert!(options[0].set.is_empty());
        assert!(!options[1].comment);
    }

    #[test]
    fn fix_options_reject_bad_shapes() {
        // duplicate action names
        let errors = parse_errors(
            r#"
[[hygiene.rules]]
id = "r"
entity = "issue"
[hygiene.rules.when]
status = { in = ["Todo"] }
[[hygiene.rules.fix.options]]
action = "a"
set = { status = "Done" }
[[hygiene.rules.fix.options]]
action = "a"
set = { status = "Todo" }
"#,
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("duplicate fix option action 'a'")),
            "{errors:?}"
        );

        // option with neither set nor comment
        let errors = parse_errors(
            r#"
[[hygiene.rules]]
id = "r"
entity = "issue"
[hygiene.rules.when]
status = { in = ["Todo"] }
[[hygiene.rules.fix.options]]
action = "a"
"#,
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("must declare 'set' and/or 'comment = true'")),
            "{errors:?}"
        );

        // comment on a non-issue rule
        let errors = parse_errors(
            r#"
[[hygiene.rules]]
id = "r"
entity = "project"
[hygiene.rules.when]
state = { in = ["started"] }
[[hygiene.rules.fix.options]]
action = "a"
comment = true
"#,
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("only supported for issue rules")),
            "{errors:?}"
        );

        // both set and options
        let errors = parse_errors(
            r#"
[[hygiene.rules]]
id = "r"
entity = "issue"
[hygiene.rules.when]
status = { in = ["Todo"] }
[hygiene.rules.fix]
set = { status = "Todo" }
options = []
"#,
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("either 'set' or 'options', not both")),
            "{errors:?}"
        );

        // unknown fix key
        let errors = parse_errors(
            r#"
[[hygiene.rules]]
id = "r"
entity = "issue"
[hygiene.rules.when]
status = { in = ["Todo"] }
[hygiene.rules.fix]
assign = { status = "Todo" }
"#,
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("unknown key 'fix.assign'")),
            "{errors:?}"
        );
    }

    #[test]
    fn multiple_operators_per_field_become_conditions() {
        let config = parse_ok(
            r#"
[[hygiene.rules]]
id = "r"
entity = "issue"
[hygiene.rules.when]
updatedAt = { older_than = "2d", newer_than = "30d" }
"#,
        );
        let Rule::When(rule) = &config.rules[0] else {
            panic!()
        };
        assert_eq!(rule.when.len(), 2);
    }

    #[test]
    fn repo_override_scope_only() {
        let over = parse_repo_hygiene_override(
            "[hygiene.scope]\nteams = [\"ENG\", \"OPS\"]\ninclude_archived = true\n",
        )
        .unwrap()
        .unwrap();
        assert_eq!(over.teams, Some(vec!["ENG".to_string(), "OPS".to_string()]));
        assert_eq!(over.include_archived, Some(true));
        assert!(over.exempt_labels.is_none());

        // No hygiene table at all
        assert!(
            parse_repo_hygiene_override("[context.defaults]\nteam = \"ENG\"\n")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn repo_override_rejects_rules_and_unknown_keys() {
        let errors =
            parse_repo_hygiene_override("[[hygiene.rules]]\nid = \"x\"\nentity = \"issue\"\n")
                .unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("rule definitions are not allowed")),
            "{errors:?}"
        );

        let errors = parse_repo_hygiene_override("[hygiene]\napply_ttl = \"5m\"\n").unwrap_err();
        assert!(
            errors.iter().any(|e| e.contains("unknown key 'apply_ttl'")),
            "{errors:?}"
        );
    }

    #[test]
    fn merge_scope_project_overrides_user() {
        let base = ScopeConfig {
            exempt_labels: vec!["ignore-audit".into()],
            teams: vec!["ENG".into()],
            include_archived: false,
        };
        let over = ScopeOverride {
            teams: Some(vec!["OPS".into()]),
            ..Default::default()
        };
        let merged = merge_scope(&base, &over);
        assert_eq!(merged.teams, vec!["OPS"]);
        assert_eq!(merged.exempt_labels, vec!["ignore-audit"]);
        assert!(!merged.include_archived);
    }

    #[test]
    fn severity_rank_orders_high_first() {
        assert!(Severity::High.rank() < Severity::Medium.rank());
        assert!(Severity::Medium.rank() < Severity::Low.rank());
    }

    #[test]
    fn apply_ttl_parses_custom_value() {
        let config = parse_ok("[hygiene]\napply_ttl = \"2h\"\n");
        assert_eq!(config.apply_ttl, "2h");
        assert_eq!(config.apply_ttl_seconds, 2 * 60 * 60);
    }

    #[test]
    fn rules_source_precedence_flag_over_env_over_default() {
        let default = PathBuf::from("/home/u/.config/linear-cli/hygiene.toml");

        // Flag wins over env and default.
        let source = resolve_rules_source(
            Some(PathBuf::from("/tmp/flag.toml")),
            Some("/tmp/env.toml".to_string()),
            default.clone(),
        );
        assert_eq!(source, RulesSource::Flag(PathBuf::from("/tmp/flag.toml")));
        assert!(source.is_override());

        // Env wins over default.
        let source = resolve_rules_source(None, Some("/tmp/env.toml".to_string()), default.clone());
        assert_eq!(source, RulesSource::Env(PathBuf::from("/tmp/env.toml")));
        assert!(source.is_override());

        // Blank env is ignored.
        let source = resolve_rules_source(None, Some("   ".to_string()), default.clone());
        assert_eq!(source, RulesSource::Default(default.clone()));
        assert!(!source.is_override());

        // Nothing set: default.
        let source = resolve_rules_source(None, None, default.clone());
        assert_eq!(source.path(), default.as_path());
        assert!(!source.is_override());
    }

    #[test]
    fn missing_override_rules_file_errors_naming_path() {
        let source = RulesSource::Flag(PathBuf::from("/nonexistent/h.toml"));
        let err = load_user_hygiene_config(&source).unwrap_err().to_string();
        assert!(err.contains("/nonexistent/h.toml"), "{err}");
        assert!(err.contains("--rules"), "{err}");

        let source = RulesSource::Env(PathBuf::from("/nonexistent/h.toml"));
        let err = load_user_hygiene_config(&source).unwrap_err().to_string();
        assert!(err.contains(HYGIENE_RULES_ENV), "{err}");
    }
}
