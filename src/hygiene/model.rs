//! Flattened entity field model (design doc §4.1) and conversions from raw
//! GraphQL JSON nodes.
//!
//! The engine evaluates `when`-rule predicates against [`EntityModel`]
//! instances. The commands layer fetches raw JSON and converts it here;
//! conversions are tolerant of missing fields (they become [`FieldValue::Null`]).

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Entity types the hygiene engine understands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntityKind {
    Issue,
    Project,
    Initiative,
}

impl EntityKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntityKind::Issue => "issue",
            EntityKind::Project => "project",
            EntityKind::Initiative => "initiative",
        }
    }

    pub fn parse(input: &str) -> Option<Self> {
        match input {
            "issue" => Some(EntityKind::Issue),
            "project" => Some(EntityKind::Project),
            "initiative" => Some(EntityKind::Initiative),
            _ => None,
        }
    }

    pub fn all() -> &'static [EntityKind] {
        &[
            EntityKind::Issue,
            EntityKind::Project,
            EntityKind::Initiative,
        ]
    }
}

impl std::fmt::Display for EntityKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Field value types in the flattened model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    String,
    Number,
    DateTime,
    Date,
    StringList,
}

impl FieldType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FieldType::String => "string",
            FieldType::Number => "number",
            FieldType::DateTime => "datetime",
            FieldType::Date => "date",
            FieldType::StringList => "string[]",
        }
    }
}

/// Static description of one field in the entity model.
#[derive(Debug, Clone, Copy)]
pub struct FieldSpec {
    pub name: &'static str,
    pub ty: FieldType,
    pub nullable: bool,
}

const fn spec(name: &'static str, ty: FieldType, nullable: bool) -> FieldSpec {
    FieldSpec { name, ty, nullable }
}

const ISSUE_FIELDS: &[FieldSpec] = &[
    spec("status", FieldType::String, false),
    // Linear stores "no priority" as 0 on a non-nullable field; the model
    // exposes 0 as missing (see `issue_from_json`) so `missing = true` rules
    // and auto-derived fixes work. Numeric comparisons never match an unset
    // priority (docs/hygiene.md §4.1).
    spec("priority", FieldType::Number, true),
    spec("estimate", FieldType::Number, true),
    spec("assignee", FieldType::String, true),
    spec("title", FieldType::String, false),
    spec("description", FieldType::String, true),
    spec("labels", FieldType::StringList, false),
    spec("team", FieldType::String, false),
    spec("project", FieldType::String, true),
    spec("cycle", FieldType::String, true),
    spec("createdAt", FieldType::DateTime, false),
    spec("updatedAt", FieldType::DateTime, false),
    spec("dueDate", FieldType::Date, true),
];

const PROJECT_FIELDS: &[FieldSpec] = &[
    spec("state", FieldType::String, false),
    spec("lead", FieldType::String, true),
    spec("name", FieldType::String, false),
    spec("description", FieldType::String, true),
    spec("labels", FieldType::StringList, false),
    spec("initiative", FieldType::String, true),
    spec("createdAt", FieldType::DateTime, false),
    spec("updatedAt", FieldType::DateTime, false),
    spec("startDate", FieldType::Date, true),
    spec("targetDate", FieldType::Date, true),
    spec("health", FieldType::String, true),
    spec("healthUpdatedAt", FieldType::DateTime, true),
];

const INITIATIVE_FIELDS: &[FieldSpec] = &[
    spec("state", FieldType::String, false),
    spec("owner", FieldType::String, true),
    spec("name", FieldType::String, false),
    spec("description", FieldType::String, true),
    spec("labels", FieldType::StringList, false),
    spec("createdAt", FieldType::DateTime, false),
    spec("updatedAt", FieldType::DateTime, false),
    spec("startDate", FieldType::Date, true),
    spec("targetDate", FieldType::Date, true),
    spec("health", FieldType::String, true),
    spec("healthUpdatedAt", FieldType::DateTime, true),
    spec("linkedProjects", FieldType::Number, false),
];

/// The field model for an entity kind.
pub fn entity_fields(kind: EntityKind) -> &'static [FieldSpec] {
    match kind {
        EntityKind::Issue => ISSUE_FIELDS,
        EntityKind::Project => PROJECT_FIELDS,
        EntityKind::Initiative => INITIATIVE_FIELDS,
    }
}

/// Look up a field spec by name for an entity kind.
pub fn field_spec(kind: EntityKind, name: &str) -> Option<&'static FieldSpec> {
    entity_fields(kind).iter().find(|f| f.name == name)
}

/// Fields a config-declared or auto-derived fix may `set` per entity kind,
/// with the `linear … update` flag used to build the command string (R29/R30).
pub fn settable_field_flag(kind: EntityKind, field: &str) -> Option<&'static str> {
    match kind {
        EntityKind::Issue => match field {
            "status" => Some("-s"),
            "priority" => Some("-p"),
            "assignee" => Some("-a"),
            "estimate" => Some("-e"),
            "dueDate" => Some("--due"),
            "project" => Some("--project"),
            "title" => Some("-T"),
            "description" => Some("-d"),
            "labels" => Some("-l"),
            _ => None,
        },
        EntityKind::Project => match field {
            "state" => Some("--status"),
            "lead" => Some("--lead"),
            "startDate" => Some("--start-date"),
            "targetDate" => Some("--target-date"),
            "name" => Some("-n"),
            "description" => Some("-d"),
            "labels" => Some("-l"),
            _ => None,
        },
        EntityKind::Initiative => match field {
            "state" => Some("-s"),
            "name" => Some("-n"),
            "description" => Some("-d"),
            _ => None,
        },
    }
}

/// Settable field names for an entity kind (subset of the field model).
pub fn settable_fields(kind: EntityKind) -> Vec<&'static str> {
    entity_fields(kind)
        .iter()
        .map(|f| f.name)
        .filter(|name| settable_field_flag(kind, name).is_some())
        .collect()
}

/// Operator vocabulary (design doc §4.2), used by the schema output and
/// referenced by config validation error messages.
pub const OPERATORS: &[(&str, &str, &str)] = &[
    (
        "in",
        "string | string[]",
        "value ∈ list (any element for labels)",
    ),
    (
        "not_in",
        "string | string[]",
        "value ∉ list (no element for labels)",
    ),
    (
        "missing",
        "any nullable",
        "true → null/empty; false → present",
    ),
    (
        "older_than",
        "datetime | date",
        "age vs now exceeds duration",
    ),
    (
        "newer_than",
        "datetime | date",
        "age vs now is within duration",
    ),
    ("past", "date", "true → date is before today"),
    ("shorter_than", "string", "character length below bound"),
    ("longer_than", "string", "character length above bound"),
    ("matches", "string", "regex match (Rust regex syntax)"),
    ("not_matches", "string", "regex does not match"),
    ("lt", "number", "numeric less-than"),
    ("lte", "number", "numeric less-than-or-equal"),
    ("gt", "number", "numeric greater-than"),
    ("gte", "number", "numeric greater-than-or-equal"),
    ("eq", "number", "numeric equality"),
    (
        "missing_group",
        "labels only",
        "no label from the named context label group",
    ),
];

/// Emit the field model + operator vocabulary as JSON for
/// `hygiene rules --schema` (R22).
pub fn schema_json() -> Value {
    let mut entities = serde_json::Map::new();
    for kind in EntityKind::all() {
        let fields: Vec<Value> = entity_fields(*kind)
            .iter()
            .map(|f| {
                json!({
                    "name": f.name,
                    "type": f.ty.as_str(),
                    "nullable": f.nullable,
                    "settable": settable_field_flag(*kind, f.name).is_some(),
                })
            })
            .collect();
        entities.insert(kind.as_str().to_string(), Value::Array(fields));
    }
    let operators: Vec<Value> = OPERATORS
        .iter()
        .map(|(name, applies_to, meaning)| {
            json!({ "operator": name, "appliesTo": applies_to, "meaning": meaning })
        })
        .collect();
    json!({ "entities": entities, "operators": operators })
}

/// A typed field value on a concrete entity.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    Null,
    String(String),
    Number(f64),
    StringList(Vec<String>),
    DateTime(DateTime<Utc>),
    Date(NaiveDate),
}

impl FieldValue {
    /// `missing` semantics: null, empty string, or empty list.
    pub fn is_missing(&self) -> bool {
        match self {
            FieldValue::Null => true,
            FieldValue::String(s) => s.trim().is_empty(),
            FieldValue::StringList(items) => items.is_empty(),
            _ => false,
        }
    }

    /// JSON representation for evidence payloads.
    pub fn to_json(&self) -> Value {
        match self {
            FieldValue::Null => Value::Null,
            FieldValue::String(s) => json!(s),
            FieldValue::Number(n) => json!(n),
            FieldValue::StringList(items) => json!(items),
            FieldValue::DateTime(dt) => {
                json!(dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
            }
            FieldValue::Date(d) => json!(d.format("%Y-%m-%d").to_string()),
        }
    }

    /// Compact display for summary lines.
    pub fn display(&self) -> String {
        match self {
            FieldValue::Null => "null".to_string(),
            FieldValue::String(s) => s.clone(),
            FieldValue::Number(n) => format_number(*n),
            FieldValue::StringList(items) => items.join(", "),
            FieldValue::DateTime(dt) => dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            FieldValue::Date(d) => d.format("%Y-%m-%d").to_string(),
        }
    }
}

pub(crate) fn format_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}

/// Owner reference (assignee / lead / initiative owner) attached to findings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnerRef {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
}

/// Cross-entity side data supplied for builtins (kept out of the field model).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SideData {
    /// Number of issues in a project (`project-single-issue`).
    pub project_issue_count: Option<u64>,
    /// Lifecycle states of an initiative's linked projects
    /// (`initiative-completed-but-active`).
    pub initiative_project_states: Option<Vec<String>>,
}

/// A flattened entity the engine evaluates rules against.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityModel {
    pub kind: EntityKind,
    pub id: String,
    /// Issue identifier (`ENG-123`) or project/initiative slug (name fallback).
    pub identifier: String,
    pub title: String,
    pub url: Option<String>,
    pub owner: Option<OwnerRef>,
    pub fields: BTreeMap<String, FieldValue>,
    pub side: SideData,
}

impl EntityModel {
    pub fn field(&self, name: &str) -> FieldValue {
        self.fields.get(name).cloned().unwrap_or(FieldValue::Null)
    }

    pub fn labels(&self) -> Vec<String> {
        match self.field("labels") {
            FieldValue::StringList(items) => items,
            _ => Vec::new(),
        }
    }

    /// Convert a raw GraphQL issue node (shape used across `commands/issues.rs`
    /// queries: `state { name }`, `assignee { … }`, `labels { nodes { name } }`).
    pub fn issue_from_json(node: &Value) -> Self {
        let mut fields = BTreeMap::new();
        fields.insert("status".into(), string_at(node, &["state", "name"]));
        // Priority 0 means "no priority" in Linear's API (the field itself is
        // non-nullable); expose it as missing so `priority = { missing = true }`
        // rules match and auto-derived p1–p4 fixes fire.
        let priority = match number_of(&node["priority"]) {
            FieldValue::Number(0.0) => FieldValue::Null,
            value => value,
        };
        fields.insert("priority".into(), priority);
        fields.insert("estimate".into(), number_of(&node["estimate"]));
        fields.insert("assignee".into(), person_name(&node["assignee"]));
        fields.insert("title".into(), string_of(&node["title"]));
        fields.insert("description".into(), string_of(&node["description"]));
        fields.insert("labels".into(), label_names(&node["labels"]));
        fields.insert("team".into(), string_at(node, &["team", "key"]));
        fields.insert("project".into(), string_at(node, &["project", "name"]));
        fields.insert("cycle".into(), cycle_name(&node["cycle"]));
        fields.insert("createdAt".into(), datetime_of(&node["createdAt"]));
        fields.insert("updatedAt".into(), datetime_of(&node["updatedAt"]));
        fields.insert("dueDate".into(), date_of(&node["dueDate"]));

        EntityModel {
            kind: EntityKind::Issue,
            id: str_or_empty(&node["id"]),
            identifier: str_or_empty(&node["identifier"]),
            title: str_or_empty(&node["title"]),
            url: node["url"].as_str().map(str::to_string),
            owner: owner_ref(&node["assignee"]),
            fields,
            side: SideData::default(),
        }
    }

    /// Convert a raw GraphQL project node.
    pub fn project_from_json(node: &Value) -> Self {
        let mut fields = BTreeMap::new();
        let state = match string_of(&node["state"]) {
            FieldValue::Null => string_at(node, &["status", "name"]),
            value => value,
        };
        fields.insert("state".into(), state);
        fields.insert("lead".into(), person_name(&node["lead"]));
        fields.insert("name".into(), string_of(&node["name"]));
        fields.insert("description".into(), string_of(&node["description"]));
        fields.insert("labels".into(), label_names(&node["labels"]));
        let initiative = match string_at(node, &["initiative", "name"]) {
            FieldValue::Null => string_at(node, &["initiatives", "nodes", "0", "name"]),
            value => value,
        };
        fields.insert("initiative".into(), initiative);
        fields.insert("createdAt".into(), datetime_of(&node["createdAt"]));
        fields.insert("updatedAt".into(), datetime_of(&node["updatedAt"]));
        fields.insert("startDate".into(), date_of(&node["startDate"]));
        fields.insert("targetDate".into(), date_of(&node["targetDate"]));
        fields.insert("health".into(), string_of(&node["health"]));
        fields.insert(
            "healthUpdatedAt".into(),
            datetime_of(&node["healthUpdatedAt"]),
        );

        let issue_count = node["issueCount"]
            .as_u64()
            .or_else(|| node["issues"]["nodes"].as_array().map(|a| a.len() as u64));

        let name = str_or_empty(&node["name"]);
        EntityModel {
            kind: EntityKind::Project,
            id: str_or_empty(&node["id"]),
            identifier: node["slugId"]
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| name.clone()),
            title: name,
            url: node["url"].as_str().map(str::to_string),
            owner: owner_ref(&node["lead"]),
            fields,
            side: SideData {
                project_issue_count: issue_count,
                initiative_project_states: None,
            },
        }
    }

    /// Convert a raw GraphQL initiative node.
    pub fn initiative_from_json(node: &Value) -> Self {
        let mut fields = BTreeMap::new();
        let state = match string_of(&node["state"]) {
            FieldValue::Null => string_of(&node["status"]),
            value => value,
        };
        fields.insert("state".into(), state);
        fields.insert("owner".into(), person_name(&node["owner"]));
        fields.insert("name".into(), string_of(&node["name"]));
        fields.insert("description".into(), string_of(&node["description"]));
        fields.insert("labels".into(), label_names(&node["labels"]));
        fields.insert("createdAt".into(), datetime_of(&node["createdAt"]));
        fields.insert("updatedAt".into(), datetime_of(&node["updatedAt"]));
        fields.insert("startDate".into(), date_of(&node["startDate"]));
        fields.insert("targetDate".into(), date_of(&node["targetDate"]));
        fields.insert("health".into(), string_of(&node["health"]));
        fields.insert(
            "healthUpdatedAt".into(),
            datetime_of(&node["healthUpdatedAt"]),
        );

        let project_states: Option<Vec<String>> = node["projects"]["nodes"].as_array().map(|a| {
            a.iter()
                .filter_map(|p| {
                    p["state"]
                        .as_str()
                        .or_else(|| p["status"]["name"].as_str())
                        .map(str::to_string)
                })
                .collect()
        });
        let linked = node["linkedProjects"]
            .as_u64()
            .or_else(|| project_states.as_ref().map(|s| s.len() as u64))
            .unwrap_or(0);
        fields.insert("linkedProjects".into(), FieldValue::Number(linked as f64));

        let name = str_or_empty(&node["name"]);
        EntityModel {
            kind: EntityKind::Initiative,
            id: str_or_empty(&node["id"]),
            identifier: node["slugId"]
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| name.clone()),
            title: name,
            url: node["url"].as_str().map(str::to_string),
            owner: owner_ref(&node["owner"]),
            fields,
            side: SideData {
                project_issue_count: None,
                initiative_project_states: project_states,
            },
        }
    }
}

fn str_or_empty(value: &Value) -> String {
    value.as_str().unwrap_or("").to_string()
}

fn string_of(value: &Value) -> FieldValue {
    match value.as_str() {
        Some(s) => FieldValue::String(s.to_string()),
        None => FieldValue::Null,
    }
}

fn string_at(node: &Value, path: &[&str]) -> FieldValue {
    let mut current = node;
    for key in path {
        current = match key.parse::<usize>() {
            Ok(index) => &current[index],
            Err(_) => &current[*key],
        };
    }
    string_of(current)
}

fn number_of(value: &Value) -> FieldValue {
    match value.as_f64() {
        Some(n) => FieldValue::Number(n),
        None => FieldValue::Null,
    }
}

fn person_name(value: &Value) -> FieldValue {
    let name = value["displayName"]
        .as_str()
        .or_else(|| value["name"].as_str());
    match name {
        Some(s) => FieldValue::String(s.to_string()),
        None => FieldValue::Null,
    }
}

fn owner_ref(value: &Value) -> Option<OwnerRef> {
    if !value.is_object() {
        return None;
    }
    Some(OwnerRef {
        id: value["id"].as_str().map(str::to_string),
        name: value["name"].as_str().map(str::to_string),
        display_name: value["displayName"].as_str().map(str::to_string),
    })
}

fn label_names(value: &Value) -> FieldValue {
    let names: Vec<String> = value["nodes"]
        .as_array()
        .map(|nodes| {
            nodes
                .iter()
                .filter_map(|n| n["name"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    FieldValue::StringList(names)
}

fn cycle_name(value: &Value) -> FieldValue {
    if let Some(name) = value["name"].as_str() {
        return FieldValue::String(name.to_string());
    }
    if let Some(number) = value["number"].as_u64() {
        return FieldValue::String(number.to_string());
    }
    FieldValue::Null
}

/// Parse an RFC 3339 timestamp into a datetime field value.
pub fn datetime_of(value: &Value) -> FieldValue {
    value
        .as_str()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| FieldValue::DateTime(dt.with_timezone(&Utc)))
        .unwrap_or(FieldValue::Null)
}

/// Parse a `YYYY-MM-DD` date (tolerating full timestamps) into a date value.
pub fn date_of(value: &Value) -> FieldValue {
    let Some(s) = value.as_str() else {
        return FieldValue::Null;
    };
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return FieldValue::Date(d);
    }
    DateTime::parse_from_rfc3339(s)
        .map(|dt| FieldValue::Date(dt.with_timezone(&Utc).date_naive()))
        .unwrap_or(FieldValue::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_model_matches_design_table() {
        // Spot-check the §4.1 matrix.
        assert!(field_spec(EntityKind::Issue, "status").is_some());
        assert!(field_spec(EntityKind::Project, "status").is_none());
        assert!(field_spec(EntityKind::Project, "state").is_some());
        assert!(field_spec(EntityKind::Initiative, "linkedProjects").is_some());
        assert!(field_spec(EntityKind::Issue, "linkedProjects").is_none());
        assert!(field_spec(EntityKind::Issue, "dueDate").is_some());
        assert!(field_spec(EntityKind::Project, "dueDate").is_none());
        assert!(field_spec(EntityKind::Issue, "estimate").unwrap().nullable);
        // Priority is exposed as nullable: Linear's 0 ("no priority") maps to
        // missing in the model.
        assert!(field_spec(EntityKind::Issue, "priority").unwrap().nullable);
        assert_eq!(
            field_spec(EntityKind::Initiative, "healthUpdatedAt")
                .unwrap()
                .ty,
            FieldType::DateTime
        );
    }

    #[test]
    fn settable_fields_have_flags() {
        assert_eq!(settable_field_flag(EntityKind::Issue, "status"), Some("-s"));
        assert_eq!(
            settable_field_flag(EntityKind::Project, "targetDate"),
            Some("--target-date")
        );
        // Project labels map to `p update -l`; initiatives have no label flag.
        assert_eq!(
            settable_field_flag(EntityKind::Project, "labels"),
            Some("-l")
        );
        assert_eq!(settable_field_flag(EntityKind::Initiative, "labels"), None);
        assert_eq!(settable_field_flag(EntityKind::Issue, "team"), None);
        assert!(settable_fields(EntityKind::Issue).contains(&"priority"));
        assert!(!settable_fields(EntityKind::Initiative).contains(&"owner"));
    }

    #[test]
    fn schema_json_shape() {
        let schema = schema_json();
        let issue_fields = schema["entities"]["issue"].as_array().unwrap();
        assert!(issue_fields.iter().any(|f| f["name"] == "status"));
        let ops = schema["operators"].as_array().unwrap();
        assert!(ops.iter().any(|o| o["operator"] == "missing_group"));
        assert_eq!(ops.len(), OPERATORS.len());
    }

    #[test]
    fn issue_conversion_full() {
        let node = json!({
            "id": "uuid-1",
            "identifier": "ENG-123",
            "title": "Fix login flow",
            "url": "https://linear.app/org/issue/ENG-123",
            "priority": 2,
            "estimate": 3.0,
            "description": "Some text",
            "state": { "name": "In Review" },
            "assignee": { "id": "u1", "name": "chris", "displayName": "Chris" },
            "team": { "key": "ENG" },
            "project": { "name": "Auth" },
            "cycle": { "number": 4 },
            "labels": { "nodes": [ { "name": "bug" }, { "name": "payments" } ] },
            "createdAt": "2026-06-01T09:00:00Z",
            "updatedAt": "2026-06-26T09:00:00.000Z",
            "dueDate": "2026-07-10",
        });
        let entity = EntityModel::issue_from_json(&node);
        assert_eq!(entity.kind, EntityKind::Issue);
        assert_eq!(entity.identifier, "ENG-123");
        assert_eq!(
            entity.field("status"),
            FieldValue::String("In Review".into())
        );
        assert_eq!(entity.field("priority"), FieldValue::Number(2.0));
        assert_eq!(entity.field("team"), FieldValue::String("ENG".into()));
        assert_eq!(entity.field("cycle"), FieldValue::String("4".into()));
        assert_eq!(
            entity.labels(),
            vec!["bug".to_string(), "payments".to_string()]
        );
        assert_eq!(
            entity.field("dueDate"),
            FieldValue::Date(NaiveDate::from_ymd_opt(2026, 7, 10).unwrap())
        );
        assert!(matches!(entity.field("updatedAt"), FieldValue::DateTime(_)));
        let owner = entity.owner.unwrap();
        assert_eq!(owner.display_name.as_deref(), Some("Chris"));
    }

    #[test]
    fn issue_conversion_tolerates_missing_fields() {
        let entity = EntityModel::issue_from_json(&json!({ "identifier": "ENG-1" }));
        assert_eq!(entity.field("status"), FieldValue::Null);
        assert_eq!(entity.field("assignee"), FieldValue::Null);
        assert_eq!(entity.field("labels"), FieldValue::StringList(vec![]));
        assert_eq!(entity.field("updatedAt"), FieldValue::Null);
        assert!(entity.owner.is_none());
        assert!(entity.url.is_none());
    }

    #[test]
    fn issue_priority_zero_maps_to_missing() {
        let entity = EntityModel::issue_from_json(&json!({ "identifier": "ENG-1", "priority": 0 }));
        assert_eq!(entity.field("priority"), FieldValue::Null);
        assert!(entity.field("priority").is_missing());
        // Real priorities stay numeric.
        let entity = EntityModel::issue_from_json(&json!({ "identifier": "ENG-2", "priority": 1 }));
        assert_eq!(entity.field("priority"), FieldValue::Number(1.0));
        assert!(!entity.field("priority").is_missing());
    }

    #[test]
    fn project_conversion() {
        let node = json!({
            "id": "p-uuid",
            "name": "Payments revamp",
            "slugId": "payments-revamp",
            "state": "started",
            "lead": { "id": "u2", "name": "sam" },
            "targetDate": "2026-09-01",
            "createdAt": "2026-01-01T00:00:00Z",
            "updatedAt": "2026-06-01T00:00:00Z",
            "health": "onTrack",
            "healthUpdatedAt": "2026-06-20T10:00:00Z",
            "issueCount": 7,
            "initiative": { "name": "Platform" },
        });
        let entity = EntityModel::project_from_json(&node);
        assert_eq!(entity.kind, EntityKind::Project);
        assert_eq!(entity.identifier, "payments-revamp");
        assert_eq!(entity.field("state"), FieldValue::String("started".into()));
        assert_eq!(entity.field("lead"), FieldValue::String("sam".into()));
        assert_eq!(
            entity.field("initiative"),
            FieldValue::String("Platform".into())
        );
        assert_eq!(entity.side.project_issue_count, Some(7));
    }

    #[test]
    fn project_conversion_state_fallback_and_issue_nodes() {
        let node = json!({
            "id": "p2",
            "name": "Solo",
            "status": { "name": "backlog" },
            "issues": { "nodes": [ { "id": "i1" } ] },
        });
        let entity = EntityModel::project_from_json(&node);
        assert_eq!(entity.identifier, "Solo"); // name fallback when no slugId
        assert_eq!(entity.field("state"), FieldValue::String("backlog".into()));
        assert_eq!(entity.side.project_issue_count, Some(1));
    }

    #[test]
    fn initiative_conversion() {
        let node = json!({
            "id": "init-uuid",
            "name": "Platform Migration",
            "slugId": "platform-migration",
            "status": "started",
            "owner": { "id": "u3", "displayName": "Dana" },
            "createdAt": "2025-01-01T00:00:00Z",
            "updatedAt": "2026-06-01T00:00:00Z",
            "projects": { "nodes": [
                { "state": "completed" },
                { "status": { "name": "completed" } },
            ]},
        });
        let entity = EntityModel::initiative_from_json(&node);
        assert_eq!(entity.kind, EntityKind::Initiative);
        assert_eq!(entity.identifier, "platform-migration");
        assert_eq!(entity.field("state"), FieldValue::String("started".into()));
        assert_eq!(entity.field("linkedProjects"), FieldValue::Number(2.0));
        assert_eq!(
            entity.side.initiative_project_states,
            Some(vec!["completed".to_string(), "completed".to_string()])
        );
        assert_eq!(entity.owner.unwrap().display_name.as_deref(), Some("Dana"));
    }

    #[test]
    fn field_value_missing_semantics() {
        assert!(FieldValue::Null.is_missing());
        assert!(FieldValue::String("".into()).is_missing());
        assert!(FieldValue::String("  ".into()).is_missing());
        assert!(!FieldValue::String("x".into()).is_missing());
        assert!(FieldValue::StringList(vec![]).is_missing());
        assert!(!FieldValue::StringList(vec!["a".into()]).is_missing());
        assert!(!FieldValue::Number(0.0).is_missing());
    }

    #[test]
    fn date_of_tolerates_timestamps() {
        assert_eq!(
            date_of(&json!("2026-07-01")),
            FieldValue::Date(NaiveDate::from_ymd_opt(2026, 7, 1).unwrap())
        );
        assert_eq!(
            date_of(&json!("2026-07-01T10:30:00Z")),
            FieldValue::Date(NaiveDate::from_ymd_opt(2026, 7, 1).unwrap())
        );
        assert_eq!(date_of(&json!("garbage")), FieldValue::Null);
        assert_eq!(date_of(&Value::Null), FieldValue::Null);
    }

    #[test]
    fn field_value_json_and_display() {
        assert_eq!(FieldValue::Number(2.0).display(), "2");
        assert_eq!(FieldValue::Number(2.5).display(), "2.5");
        assert_eq!(FieldValue::Null.to_json(), Value::Null);
        assert_eq!(
            FieldValue::StringList(vec!["a".into(), "b".into()]).to_json(),
            json!(["a", "b"])
        );
    }
}
