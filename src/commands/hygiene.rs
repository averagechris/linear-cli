//! `linear hygiene` (alias `hy`): the command layer over the pure rule engine
//! in `crate::hygiene` (see `docs/hygiene.md`, R19–R36).
//!
//! This module owns scope resolution, fresh entity fetching (R36), building
//! [`EvalInputs`] from context metadata caches (R35), run-artifact/snooze
//! persistence, and the `fix`/`apply` mutation pipelines. All rule evaluation
//! stays in the engine.

use std::collections::{BTreeMap, BTreeSet};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::{Args, Subcommand};
use colored::Colorize;
use futures::stream::{self, StreamExt};
use serde_json::{json, Map, Value};
use tabled::Table;

use crate::api::{
    resolve_label_id, resolve_project_id, resolve_state_id_cached, resolve_user_id, LinearClient,
};
use crate::cache::{Cache, CacheType};
use crate::config;
use crate::dates::parse_duration_seconds;
use crate::error::CliError;
use crate::hygiene::config::{
    hygiene_rules_source, load_effective_hygiene_config, BuiltinKind, HygieneConfig, Predicate,
    Rule, DEFAULT_APPLY_TTL,
};
use crate::hygiene::engine::{
    filter_snoozed, format_duration_compact, run_rules, EvalInputs, Finding, Fix,
};
use crate::hygiene::model::{
    format_number, schema_json, settable_field_flag, settable_fields, EntityKind, EntityModel,
};
use crate::hygiene::state::{
    load_artifact, load_snoozes, save_artifact, save_snoozes, MarkOutcome, RunArtifact,
    StoredFinding,
};
use crate::output::{print_json_owned, OutputOptions};
use crate::pagination::{paginate_nodes, PaginationOptions};
use crate::text::truncate;
use crate::AgentOptions;

/// Concurrency for `fix --yes` mutation execution (matches `bulk`).
const FIX_CONCURRENCY: usize = 8;

/// Per-entity fetch page sizes, sized against Linear's ~10,000 GraphQL
/// complexity budget per request. Linear roughly charges
/// `pageSize × (ownFields + Σ nestedFirst × nestedFields)` for a paginated
/// query, and nested connections without an explicit `first` default to ~50.
/// Worst cases for the selections in [`fetch_entities`]:
///
///   issues:      100 × (~20 fields + 25×1 labels)                  ≈ 4,500
///   projects:     50 × (~20 fields + 25×1 labels + 10×2 initiatives
///                       + 100×1 issue ids when a rule needs counts) ≈ 8,250
///   initiatives:  50 × (~15 fields + 25×1 labels
///                       + 50×2 projects when a rule needs states)   ≈ 7,000
///
/// All comfortably under budget. The previous project fetch (100 per page,
/// nested `labels`/`initiatives` connections left at the ~50 default) cost a
/// constant ≈ 11,740 and was rejected with "Query too complex".
const ISSUE_FETCH_PAGE_SIZE: usize = 100;
const PROJECT_FETCH_PAGE_SIZE: usize = 50;
const INITIATIVE_FETCH_PAGE_SIZE: usize = 50;

/// Pagination for one entity fetch: hygiene always paginates fully
/// (`all: true`), honoring the global `--page-size` clamped to the
/// per-entity complexity-safe maximum above.
fn fetch_pagination(requested: Option<usize>, max: usize) -> PaginationOptions {
    PaginationOptions {
        all: true,
        page_size: Some(requested.unwrap_or(max).clamp(1, max)),
        ..Default::default()
    }
}

/// Shared scoping flags (R19). The org-wide axis reuses the global `--all`
/// flag (which also means "fetch all pages"; hygiene always paginates fully).
#[derive(Args, Debug, Clone, Default)]
pub struct ScopeArgs {
    /// Scope to entities owned by the authenticated user (assignee/lead/owner)
    #[arg(long)]
    pub mine: bool,
    /// Scope to entities owned by a user (name, email, or ID)
    #[arg(long, conflicts_with = "mine")]
    pub user: Option<String>,
    /// Scope to a team by key (repeatable)
    #[arg(long = "team", short = 't')]
    pub teams: Vec<String>,
    /// Scope to a single project (name or slug); checks issues + projects only
    #[arg(long)]
    pub project: Option<String>,
    /// Scope to a single initiative (name or slug); checks projects + initiatives only
    #[arg(long, conflicts_with = "project")]
    pub initiative: Option<String>,
    /// Evaluate only these entity types: issue, project, initiative (repeatable)
    #[arg(long = "entity")]
    pub entities: Vec<String>,
    /// Evaluate only these rule ids (repeatable)
    #[arg(long = "rule")]
    pub rules: Vec<String>,
}

#[derive(Subcommand, Debug)]
pub enum HygieneCommands {
    /// Run hygiene rules over the scoped entities and print findings
    #[command(after_help = r#"EXAMPLES:
    linear hygiene check                       # Default scope (repo/team config, else --mine)
    linear hy check -t ENG --output json       # Team scope, agent-readable findings
    linear hy check --mine --rule stale-in-review
    linear hy check --all --fail-if-findings   # Org-wide, exit 1 on findings

The global --all flag makes the scope org-wide."#)]
    Check {
        #[command(flatten)]
        scope: ScopeArgs,
        /// Exit 1 when findings exist (default: findings exit 0)
        #[arg(long)]
        fail_if_findings: bool,
    },
    /// List, validate, and scaffold hygiene rules
    #[command(after_help = r#"EXAMPLES:
    linear hygiene rules                       # List effective rules (validates config)
    linear hy rules --schema                   # Field model + operators as JSON
    linear hy rules --init                     # Write a commented starter hygiene.toml

`--schema` is also available as the global flag of the same name."#)]
    Rules {
        /// Write a commented starter hygiene.toml (refuses to overwrite)
        #[arg(long)]
        init: bool,
        /// Overwrite an existing hygiene.toml with --init
        #[arg(long, requires = "init")]
        force: bool,
    },
    /// Aggregate a check run into counts by rule, owner, team, or entity
    #[command(after_help = r#"EXAMPLES:
    linear hygiene report                      # Counts by rule
    linear hy report --by owner -t ENG --output json"#)]
    Report {
        /// Group counts by: rule, owner, team, or entity
        #[arg(long, default_value = "rule")]
        by: String,
        #[command(flatten)]
        scope: ScopeArgs,
    },
    /// Remediate findings in bulk (only deterministic fixes run unattended)
    #[command(after_help = r#"EXAMPLES:
    linear hygiene fix --dry-run               # Preview intended mutations
    linear hy fix --rule urgent-in-backlog --yes
    linear hy fix -t ENG --yes                 # Executes command-kind fixes only

With --yes, options-kind fixes are skipped (needs_choice) and needs_input-kind
fixes are skipped (needs_input); remediate those one-by-one via `hygiene apply`."#)]
    Fix {
        #[command(flatten)]
        scope: ScopeArgs,
    },
    /// Execute the stored fix for findings from the last check artifact
    #[command(after_help = r#"EXAMPLES:
    linear hygiene apply stale-in-review:ENG-123 --option move_back
    linear hy apply urgent-in-backlog:ENG-9    # command-kind: no extra flags
    linear hy apply needs-update:proj-1 --input "Status: on track"
    linear hy apply KEY --dry-run              # Preview resolved mutations

The artifact must be fresher than [hygiene] apply_ttl (default 30m)."#)]
    Apply {
        /// Finding dedupe keys from `hygiene check` (rule-id:entity-identifier)
        #[arg(required = true)]
        keys: Vec<String>,
        /// Action name selecting among an options-kind fix
        #[arg(long)]
        option: Option<String>,
        /// Authored content for a needs_input-kind fix
        #[arg(long)]
        input: Option<String>,
    },
    /// Suppress a finding locally until expiry
    #[command(after_help = r#"EXAMPLES:
    linear hygiene snooze stale-in-review:ENG-123 --for 2w
    linear hy snooze --list                    # Active snoozes
    linear hy snooze --clear                   # Remove all snoozes"#)]
    Snooze {
        /// Finding dedupe key to snooze
        #[arg(required_unless_present_any = ["list", "clear"])]
        key: Option<String>,
        /// Snooze duration (e.g. 90m, 6h, 7d, 2w)
        #[arg(long = "for", value_name = "DURATION")]
        duration: Option<String>,
        /// List active snoozes
        #[arg(long, conflicts_with_all = ["key", "duration", "clear"])]
        list: bool,
        /// Clear all snoozes
        #[arg(long, conflicts_with_all = ["key", "duration"])]
        clear: bool,
    },
}

pub async fn handle(
    cmd: HygieneCommands,
    rules_path: Option<PathBuf>,
    output: &OutputOptions,
    agent_opts: AgentOptions,
    schema_flag: bool,
) -> Result<()> {
    let rules_path = rules_path.as_deref();
    match cmd {
        HygieneCommands::Check {
            scope,
            fail_if_findings,
        } => check(scope, fail_if_findings, rules_path, output, agent_opts).await,
        HygieneCommands::Rules { init, force } => {
            rules_cmd(schema_flag, init, force, rules_path, output).await
        }
        HygieneCommands::Report { by, scope } => {
            report(&by, scope, rules_path, output, agent_opts).await
        }
        HygieneCommands::Fix { scope } => fix(scope, rules_path, output, agent_opts).await,
        HygieneCommands::Apply {
            keys,
            option,
            input,
        } => apply(keys, option, input, rules_path, output, agent_opts).await,
        HygieneCommands::Snooze {
            key,
            duration,
            list,
            clear,
        } => snooze(key, duration, list, clear, output, agent_opts),
    }
}

// ---------------------------------------------------------------------------
// Scope resolution (R19/R20)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
struct ResolvedScope {
    /// Resolved owner-axis user: (display label, UUID).
    owner: Option<(String, String)>,
    teams: Vec<String>,
    project: Option<String>,
    initiative: Option<String>,
    org_wide: bool,
}

impl ResolvedScope {
    fn to_json(&self, args: &ScopeArgs) -> Value {
        json!({
            "owner": self.owner.as_ref().map(|(label, _)| label.clone()),
            "teams": self.teams,
            "project": self.project,
            "initiative": self.initiative,
            "orgWide": self.org_wide,
            "entities": args.entities,
            "rules": args.rules,
        })
    }
}

/// Compose the two scope axes (R19) with the R20 default resolution:
/// repo/user `[hygiene.scope] teams` → context default team → else `--mine`.
async fn resolve_scope(
    args: &ScopeArgs,
    hygiene_config: &HygieneConfig,
    client: &LinearClient,
    output: &OutputOptions,
) -> Result<ResolvedScope> {
    let org_wide = output.pagination.all;
    let mut scope = ResolvedScope {
        teams: args.teams.clone(),
        project: args.project.clone(),
        initiative: args.initiative.clone(),
        org_wide,
        ..Default::default()
    };

    let mut mine = args.mine;
    let container_given =
        !args.teams.is_empty() || args.project.is_some() || args.initiative.is_some() || org_wide;
    if !mine && args.user.is_none() && !container_given {
        if !hygiene_config.scope.teams.is_empty() {
            scope.teams = hygiene_config.scope.teams.clone();
        } else if let Some(team) = config::resolved_context()?.resolved.defaults.team {
            scope.teams = vec![team];
        } else {
            mine = true;
        }
    }

    if mine {
        let id = resolve_user_id(client, "me", &output.cache).await?;
        scope.owner = Some(("me".to_string(), id));
    } else if let Some(user) = &args.user {
        let id = resolve_user_id(client, user, &output.cache).await?;
        scope.owner = Some((user.clone(), id));
    }

    Ok(scope)
}

/// Select the enabled rules to run, narrowed by `--rule` / `--entity` (R19).
fn select_rules(config: &HygieneConfig, args: &ScopeArgs) -> Result<Vec<Rule>> {
    let known: BTreeSet<&str> = config.rules.iter().map(|r| r.id()).collect();
    let unknown: Vec<&String> = args
        .rules
        .iter()
        .filter(|id| !known.contains(id.as_str()))
        .collect();
    if !unknown.is_empty() {
        return Err(CliError::not_found(format!(
            "Unknown rule id(s): {}. Known rules: {}",
            unknown
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            known.into_iter().collect::<Vec<_>>().join(", ")
        ))
        .into());
    }

    let mut entity_kinds = BTreeSet::new();
    for raw in &args.entities {
        let Some(kind) = EntityKind::parse(raw) else {
            return Err(CliError::general(format!(
                "Invalid --entity '{raw}' (expected issue, project, or initiative)"
            ))
            .into());
        };
        entity_kinds.insert(kind);
    }

    Ok(config
        .enabled_rules()
        .filter(|rule| args.rules.is_empty() || args.rules.iter().any(|id| id == rule.id()))
        .filter(|rule| entity_kinds.is_empty() || entity_kinds.contains(&rule.entity()))
        .cloned()
        .collect())
}

/// Entity kinds to fetch: kinds of the selected rules, narrowed by the
/// container axis (`--project` covers issues+projects; `--initiative`
/// covers projects+initiatives).
fn kinds_to_fetch(rules: &[Rule], scope: &ResolvedScope) -> BTreeSet<EntityKind> {
    let mut kinds: BTreeSet<EntityKind> = rules.iter().map(|r| r.entity()).collect();
    if scope.project.is_some() {
        kinds.retain(|k| matches!(k, EntityKind::Issue | EntityKind::Project));
    }
    if scope.initiative.is_some() {
        kinds.retain(|k| matches!(k, EntityKind::Project | EntityKind::Initiative));
    }
    kinds
}

// ---------------------------------------------------------------------------
// Entity fetching (R36: always fresh, never cached)
// ---------------------------------------------------------------------------

async fn fetch_entities(
    client: &LinearClient,
    kinds: &BTreeSet<EntityKind>,
    scope: &ResolvedScope,
    config: &HygieneConfig,
    rules: &[Rule],
    requested_page_size: Option<usize>,
) -> Result<Vec<EntityModel>> {
    let mut entities = Vec::new();

    if kinds.contains(&EntityKind::Issue) {
        let query = r#"
            query($filter: IssueFilter, $first: Int, $after: String, $includeArchived: Boolean) {
                issues(first: $first, after: $after, filter: $filter, includeArchived: $includeArchived) {
                    nodes {
                        id identifier title description url priority estimate dueDate createdAt updatedAt
                        state { name }
                        assignee { id name displayName }
                        team { key }
                        project { name }
                        cycle { name number }
                        labels(first: 25) { nodes { name } }
                    }
                    pageInfo { hasNextPage endCursor }
                }
            }
        "#;
        let mut filters = Vec::new();
        if !scope.teams.is_empty() {
            filters.push(json!({ "team": { "key": { "in": scope.teams } } }));
        }
        if let Some((_, id)) = &scope.owner {
            filters.push(json!({ "assignee": { "id": { "eq": id } } }));
        }
        if let Some(project) = &scope.project {
            filters.push(json!({ "project": { "or": [
                { "name": { "eqIgnoreCase": project } },
                { "slugId": { "eq": project } },
            ] } }));
        }
        let mut vars = Map::new();
        if let Some(filter) = and_filter(filters) {
            vars.insert("filter".to_string(), filter);
        }
        vars.insert(
            "includeArchived".to_string(),
            json!(config.scope.include_archived),
        );
        let nodes = paginate_nodes(
            client,
            query,
            vars,
            &["data", "issues", "nodes"],
            &["data", "issues", "pageInfo"],
            &fetch_pagination(requested_page_size, ISSUE_FETCH_PAGE_SIZE),
            ISSUE_FETCH_PAGE_SIZE,
        )
        .await?;
        entities.extend(nodes.iter().map(EntityModel::issue_from_json));
    }

    if kinds.contains(&EntityKind::Project) {
        // Project issue counts are side data for the `project-single-issue`
        // builtin only; fetch the (heavier) issues connection only when needed.
        let need_issue_counts = rules
            .iter()
            .any(|r| matches!(r, Rule::Builtin(b) if b.kind == BuiltinKind::ProjectSingleIssue));
        let issues_selection = if need_issue_counts {
            "issues(first: 100) { nodes { id } }"
        } else {
            ""
        };
        let query = format!(
            r#"
            query($filter: ProjectFilter, $first: Int, $after: String, $includeArchived: Boolean) {{
                projects(first: $first, after: $after, filter: $filter, includeArchived: $includeArchived) {{
                    nodes {{
                        id name slugId url description state createdAt updatedAt
                        startDate targetDate health healthUpdatedAt
                        lead {{ id name displayName }}
                        labels(first: 25) {{ nodes {{ name }} }}
                        initiatives(first: 10) {{ nodes {{ name }} }}
                        {issues_selection}
                    }}
                    pageInfo {{ hasNextPage endCursor }}
                }}
            }}
        "#
        );
        let mut filters = Vec::new();
        if !scope.teams.is_empty() {
            filters
                .push(json!({ "accessibleTeams": { "some": { "key": { "in": scope.teams } } } }));
        }
        if let Some((_, id)) = &scope.owner {
            filters.push(json!({ "lead": { "id": { "eq": id } } }));
        }
        if let Some(project) = &scope.project {
            filters.push(json!({ "or": [
                { "name": { "eqIgnoreCase": project } },
                { "slugId": { "eq": project } },
            ] }));
        }
        if let Some(initiative) = &scope.initiative {
            filters.push(
                json!({ "initiatives": { "some": { "name": { "eqIgnoreCase": initiative } } } }),
            );
        }
        let mut vars = Map::new();
        if let Some(filter) = and_filter(filters) {
            vars.insert("filter".to_string(), filter);
        }
        vars.insert(
            "includeArchived".to_string(),
            json!(config.scope.include_archived),
        );
        let nodes = paginate_nodes(
            client,
            &query,
            vars,
            &["data", "projects", "nodes"],
            &["data", "projects", "pageInfo"],
            &fetch_pagination(requested_page_size, PROJECT_FETCH_PAGE_SIZE),
            PROJECT_FETCH_PAGE_SIZE,
        )
        .await?;
        entities.extend(nodes.iter().map(EntityModel::project_from_json));
    }

    if kinds.contains(&EntityKind::Initiative) {
        // Linked-project states are side data for the
        // `initiative-completed-but-active` builtin and the `linkedProjects`
        // field; fetch the projects connection only when a rule needs it.
        let need_projects = rules.iter().any(|r| match r {
            Rule::Builtin(b) => b.kind == BuiltinKind::InitiativeCompletedButActive,
            Rule::When(w) => {
                w.entity == EntityKind::Initiative
                    && w.when.iter().any(|c| c.field == "linkedProjects")
            }
        });
        let projects_selection = if need_projects {
            "projects(first: 50) { nodes { id state } }"
        } else {
            ""
        };
        let query = format!(
            r#"
            query($filter: InitiativeFilter, $first: Int, $after: String) {{
                initiatives(first: $first, after: $after, filter: $filter) {{
                    nodes {{
                        id name slugId url description status createdAt updatedAt
                        targetDate health healthUpdatedAt
                        owner {{ id name displayName }}
                        labels(first: 25) {{ nodes {{ name }} }}
                        {projects_selection}
                    }}
                    pageInfo {{ hasNextPage endCursor }}
                }}
            }}
        "#
        );
        let mut filters = Vec::new();
        if !scope.teams.is_empty() {
            filters.push(json!({ "teams": { "some": { "key": { "in": scope.teams } } } }));
        }
        if let Some((_, id)) = &scope.owner {
            filters.push(json!({ "owner": { "id": { "eq": id } } }));
        }
        if let Some(initiative) = &scope.initiative {
            filters.push(json!({ "or": [
                { "name": { "eqIgnoreCase": initiative } },
                { "slugId": { "eq": initiative } },
            ] }));
        }
        let mut vars = Map::new();
        if let Some(filter) = and_filter(filters) {
            vars.insert("filter".to_string(), filter);
        }
        let nodes = paginate_nodes(
            client,
            &query,
            vars,
            &["data", "initiatives", "nodes"],
            &["data", "initiatives", "pageInfo"],
            &fetch_pagination(requested_page_size, INITIATIVE_FETCH_PAGE_SIZE),
            INITIATIVE_FETCH_PAGE_SIZE,
        )
        .await?;
        entities.extend(nodes.iter().map(EntityModel::initiative_from_json));
    }

    Ok(entities)
}

fn and_filter(mut filters: Vec<Value>) -> Option<Value> {
    match filters.len() {
        0 => None,
        1 => Some(filters.remove(0)),
        _ => Some(json!({ "and": filters })),
    }
}

// ---------------------------------------------------------------------------
// Evaluation inputs (R35: reuse context option caches)
// ---------------------------------------------------------------------------

/// Build the label-group candidates and field candidates the engine needs.
///
/// Label groups come from the context label-group policies plus the labels
/// option cache (fetched fresh when the cache is missing/stale or `--no-cache`
/// is set). Rules referencing a `missing_group` not present in the context
/// config are a hard error, matching the engine's contract.
///
/// Field candidates: `priority` 1–4 and `estimate` values from the context
/// estimation policy. Workflow-status candidates are intentionally omitted:
/// `status` is non-nullable in the field model, so `missing`-based auto-derived
/// fixes (the only consumer of candidates) can never reference it — and a
/// multi-team scope would otherwise need per-team candidate sets to avoid
/// suggesting another team's states.
async fn build_eval_metadata(
    client: &LinearClient,
    rules: &[Rule],
    output: &OutputOptions,
) -> Result<(BTreeMap<String, Vec<String>>, BTreeMap<String, Vec<String>>)> {
    let context = config::resolved_context()?;

    let mut needed_groups = BTreeSet::new();
    for rule in rules {
        if let Rule::When(when) = rule {
            for condition in &when.when {
                if let Predicate::MissingGroup(group) = &condition.predicate {
                    needed_groups.insert(group.clone());
                }
            }
        }
    }

    let mut label_groups = BTreeMap::new();
    if !needed_groups.is_empty() {
        let policies = &context.resolved.label_groups;
        let missing: Vec<&String> = needed_groups
            .iter()
            .filter(|g| !policies.iter().any(|p| &p.key == *g))
            .collect();
        if !missing.is_empty() {
            return Err(CliError::general(format!(
                "Rules reference unknown label group(s): {}. Configure them in .linear.toml \
                 (see `linear context init --required-label-group GROUP`).",
                missing
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
            .into());
        }

        let labels = fetch_labels_cached(client, output).await?;
        for policy in policies {
            if !needed_groups.contains(&policy.key) {
                continue;
            }
            let group_name = policy
                .linear_group
                .as_deref()
                .unwrap_or(policy.key.as_str())
                .to_lowercase();
            let mut candidates: Vec<String> = labels
                .iter()
                .filter(|label| {
                    label["parent"]["name"]
                        .as_str()
                        .map(|name| name.eq_ignore_ascii_case(&group_name))
                        .unwrap_or(false)
                })
                .filter_map(|label| label["name"].as_str().map(str::to_string))
                .collect();
            for option in &policy.options {
                if !candidates
                    .iter()
                    .any(|c| c.eq_ignore_ascii_case(&option.name))
                {
                    candidates.push(option.name.clone());
                }
            }
            label_groups.insert(policy.key.clone(), candidates);
        }
    }

    let mut field_candidates = BTreeMap::new();
    field_candidates.insert(
        "priority".to_string(),
        vec!["1".into(), "2".into(), "3".into(), "4".into()],
    );
    let estimate_values = &context.resolved.issue_create.estimation.values;
    if !estimate_values.is_empty() {
        field_candidates.insert(
            "estimate".to_string(),
            estimate_values.iter().map(|v| format_number(*v)).collect(),
        );
    }

    Ok((label_groups, field_candidates))
}

/// Labels from the context option cache, refreshed from the API when missing
/// or stale (honoring `--no-cache` / `--cache-ttl`, R35).
async fn fetch_labels_cached(client: &LinearClient, output: &OutputOptions) -> Result<Vec<Value>> {
    let ttl = output.cache.ttl_seconds.unwrap_or(7 * 24 * 60 * 60);
    if !output.cache.no_cache {
        let cache = Cache::with_ttl(ttl)?;
        if let Some(cached) = cache
            .get(CacheType::Labels)
            .and_then(|data| data.as_array().cloned())
        {
            return Ok(cached);
        }
    }
    let query = r#"
        query($first: Int, $after: String) {
            issueLabels(first: $first, after: $after) {
                nodes { id name color description parent { id name } }
                pageInfo { hasNextPage endCursor }
            }
        }
    "#;
    let labels = paginate_nodes(
        client,
        query,
        Map::new(),
        &["data", "issueLabels", "nodes"],
        &["data", "issueLabels", "pageInfo"],
        &PaginationOptions {
            all: true,
            ..Default::default()
        },
        250,
    )
    .await?;
    if !output.cache.no_cache {
        let cache = Cache::with_ttl(ttl)?;
        let _ = cache.set(CacheType::Labels, json!(labels));
    }
    Ok(labels)
}

// ---------------------------------------------------------------------------
// Check pipeline (shared by check / report / fix)
// ---------------------------------------------------------------------------

struct PipelineRun {
    findings: Vec<Finding>,
    scope_json: Value,
    now: DateTime<Utc>,
}

const NO_RULES_HINT: &str =
    "No hygiene rules configured. Run `linear hygiene rules --init` to create a starter hygiene.toml.";

/// Run the full read-only pipeline. Returns `None` when no config exists or
/// zero rules are enabled (R8).
async fn run_pipeline(
    args: &ScopeArgs,
    rules_path: Option<&Path>,
    output: &OutputOptions,
) -> Result<Option<PipelineRun>> {
    let Some(config) = load_effective_hygiene_config(rules_path)? else {
        return Ok(None);
    };
    if config.enabled_rules().next().is_none() {
        return Ok(None);
    }
    let rules = select_rules(&config, args)?;

    let client = LinearClient::new()?;
    let scope = resolve_scope(args, &config, &client, output).await?;
    let kinds = kinds_to_fetch(&rules, &scope);
    let entities = fetch_entities(
        &client,
        &kinds,
        &scope,
        &config,
        &rules,
        output.pagination.page_size,
    )
    .await?;
    let (label_groups, field_candidates) = build_eval_metadata(&client, &rules, output).await?;

    let now = Utc::now();
    let inputs = EvalInputs {
        now,
        label_groups: &label_groups,
        field_candidates: &field_candidates,
    };
    let findings = run_rules(&rules, &entities, &config.scope, &inputs);
    let snoozes = load_snoozes()?;
    let findings = filter_snoozed(findings, &snoozes.active_keys(now));

    Ok(Some(PipelineRun {
        scope_json: scope.to_json(args),
        findings,
        now,
    }))
}

/// Emit the R8 zero-rules hint and succeed.
fn print_no_rules_hint(output: &OutputOptions, agent_opts: AgentOptions) -> Result<()> {
    if output.is_json() || output.has_template() {
        if agent_opts.quiet {
            print_json_owned(json!([]), output)?;
        } else {
            print_json_owned(json!({ "hint": NO_RULES_HINT, "findings": [] }), output)?;
        }
    } else {
        if !agent_opts.quiet {
            eprintln!("{}", NO_RULES_HINT);
        }
        println!("No hygiene findings.");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// `hygiene check` (R20/R21)
// ---------------------------------------------------------------------------

struct FindingRow {
    severity: String,
    key: String,
    owner: String,
    summary: String,
}

impl_tabled!(FindingRow {
    severity => "Severity",
    key => "Key",
    owner => "Owner",
    summary => "Summary",
});

async fn check(
    args: ScopeArgs,
    fail_if_findings: bool,
    rules_path: Option<&Path>,
    output: &OutputOptions,
    agent_opts: AgentOptions,
) -> Result<()> {
    let Some(run) = run_pipeline(&args, rules_path, output).await? else {
        return print_no_rules_hint(output, agent_opts);
    };

    // Persist the full run before any output truncation (R32).
    let artifact = RunArtifact::new(run.scope_json.clone(), run.findings.clone(), run.now);
    save_artifact(&artifact)?;

    let mut findings = run.findings;
    if let Some(limit) = output.pagination.limit {
        findings.truncate(limit);
    }

    if output.is_json() || output.has_template() {
        print_json_owned(serde_json::to_value(&findings)?, output)?;
    } else if findings.is_empty() {
        println!("No hygiene findings.");
    } else {
        let width = crate::display_options().max_width(60);
        let rows: Vec<FindingRow> = findings.iter().map(|f| finding_row(f, width)).collect();
        println!("{}", Table::new(rows));
        if !agent_opts.quiet {
            println!();
            println!(
                "{} {} finding(s). Fix in bulk with `linear hygiene fix`, or one-by-one with `linear hygiene apply KEY`.",
                ">>".cyan(),
                findings.len()
            );
        }
    }

    if fail_if_findings && !findings.is_empty() {
        return Err(CliError::general(format!(
            "{} hygiene finding(s) (--fail-if-findings)",
            findings.len()
        ))
        .into());
    }
    Ok(())
}

fn finding_row(finding: &Finding, width: Option<usize>) -> FindingRow {
    FindingRow {
        severity: finding.severity.as_str().to_string(),
        key: finding.dedupe_key.clone(),
        owner: finding
            .owner
            .as_ref()
            .and_then(|o| o.display_name.clone().or_else(|| o.name.clone()))
            .unwrap_or_else(|| "-".to_string()),
        summary: truncate(&finding.summary, width),
    }
}

// ---------------------------------------------------------------------------
// `hygiene rules` (R22)
// ---------------------------------------------------------------------------

struct RuleRow {
    id: String,
    kind: String,
    entity: String,
    severity: String,
    enabled: String,
    source: String,
}

impl_tabled!(RuleRow {
    id => "ID",
    kind => "Kind",
    entity => "Entity",
    severity => "Severity",
    enabled => "Enabled",
    source => "Source",
});

async fn rules_cmd(
    schema: bool,
    init: bool,
    force: bool,
    rules_path: Option<&Path>,
    output: &OutputOptions,
) -> Result<()> {
    if schema {
        print_json_owned(schema_json(), output)?;
        return Ok(());
    }
    if init {
        return rules_init(force, rules_path, output);
    }

    // Loading validates and reports *all* errors at once (R7/R22). The
    // effective source honors `--rules` / LINEAR_CLI_HYGIENE_RULES.
    let path = hygiene_rules_source(rules_path)?.path().to_path_buf();
    let Some(config) = load_effective_hygiene_config(rules_path)? else {
        if output.is_json() || output.has_template() {
            print_json_owned(json!({ "hint": NO_RULES_HINT, "rules": [] }), output)?;
        } else {
            eprintln!("{}", NO_RULES_HINT);
        }
        return Ok(());
    };

    let source = path.display().to_string();
    if output.is_json() || output.has_template() {
        let rules: Vec<Value> = config
            .rules
            .iter()
            .map(|rule| {
                json!({
                    "id": rule.id(),
                    "kind": match rule { Rule::When(_) => "when", Rule::Builtin(_) => "builtin" },
                    "entity": rule.entity().as_str(),
                    "severity": rule.severity().as_str(),
                    "enabled": rule.enabled(),
                    "source": source,
                })
            })
            .collect();
        print_json_owned(json!(rules), output)?;
    } else if config.rules.is_empty() {
        eprintln!("{}", NO_RULES_HINT);
    } else {
        let rows: Vec<RuleRow> = config
            .rules
            .iter()
            .map(|rule| RuleRow {
                id: rule.id().to_string(),
                kind: match rule {
                    Rule::When(_) => "when".to_string(),
                    Rule::Builtin(_) => "builtin".to_string(),
                },
                entity: rule.entity().to_string(),
                severity: rule.severity().as_str().to_string(),
                enabled: rule.enabled().to_string(),
                source: source.clone(),
            })
            .collect();
        println!("{}", Table::new(rows));
    }
    Ok(())
}

/// Commented starter `hygiene.toml` demonstrating every operator, a config-
/// declared fix, and one builtin (R22).
const STARTER_HYGIENE_TOML: &str = r#"# Hygiene rules for `linear hygiene check` (see `linear hygiene rules --schema`
# for the entity field model and operator vocabulary).
#
# Rules are ANDed conditions per entity; OR across conditions is expressed as
# separate rules. Durations use 90m / 6h / 7d / 2w (calendar time).

[hygiene]
# How long `hygiene apply` trusts the last check artifact.
apply_ttl = "30m"

[hygiene.scope]
# Entities carrying any of these labels are skipped by all rules.
exempt_labels = ["ignore-audit"]
# Team keys to include by default (empty = all accessible).
teams = []
include_archived = false

# --- Urgent work parked in the backlog; declares its own fix (numeric `eq`,
# --- `in`, and a config-declared `set` fix producing a command-kind fix).
[[hygiene.rules]]
id = "urgent-in-backlog"
entity = "issue"
severity = "high"
[hygiene.rules.when]
priority = { eq = 1 }
status = { in = ["Backlog"] }
[hygiene.rules.fix]
set = { status = "Todo" }

# --- Missing priority. Linear stores "no priority" as 0, but the field model
# --- exposes 0 as missing, so use `missing = true` (numeric comparisons never
# --- match an unset priority). Auto-derives p1-p4 fix options.
[[hygiene.rules]]
id = "missing-priority"
entity = "issue"
severity = "medium"
[hygiene.rules.when]
status = { not_in = ["Triage", "Backlog", "Done", "Canceled", "Duplicate"] }
priority = { missing = true }

# --- Stale review: `older_than` staleness plus an options fix where one
# --- option needs authored content (`comment = true`).
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

# --- Missing estimate on active work (`not_in`, `missing`). Without a [fix]
# --- block, missing-field rules auto-derive options from context metadata.
[[hygiene.rules]]
id = "missing-estimate"
entity = "issue"
severity = "low"
[hygiene.rules.when]
status = { not_in = ["Triage", "Backlog", "Done", "Canceled"] }
estimate = { missing = true }

# --- Thin descriptions and title conventions (`shorter_than`, `longer_than`,
# --- `matches`, `not_matches`). Visibility only: no automated fix.
[[hygiene.rules]]
id = "thin-description"
entity = "issue"
severity = "low"
enabled = false
[hygiene.rules.when]
description = { shorter_than = 20 }
title = { not_matches = "^\\[" }

# --- Overdue issues (`past`) and numeric bounds (`gte`, `lt`, `lte`, `gt`).
[[hygiene.rules]]
id = "overdue"
entity = "issue"
severity = "medium"
enabled = false
[hygiene.rules.when]
dueDate = { past = true }
priority = { gte = 1 }

# --- Recently churned projects (`newer_than`) with no lead.
[[hygiene.rules]]
id = "project-no-lead"
entity = "project"
severity = "medium"
enabled = false
[hygiene.rules.when]
state = { in = ["started"] }
updatedAt = { newer_than = "30d" }
lead = { missing = true }

# --- Label-group coverage (`missing_group` resolves candidates through the
# --- context label-group config). Enable after configuring label groups via
# --- `linear context init --required-label-group domain`.
[[hygiene.rules]]
id = "issue-missing-domain"
entity = "issue"
enabled = false
[hygiene.rules.when]
status = { not_in = ["Triage", "Backlog", "Done", "Canceled", "Duplicate"] }
labels = { missing_group = "domain" }

# --- Stale initiative health updates.
[[hygiene.rules]]
id = "initiative-stale-health"
entity = "initiative"
severity = "high"
enabled = false
[hygiene.rules.when]
state = { in = ["Active"] }
healthUpdatedAt = { older_than = "14d" }

# --- Builtin: one finding per assignee over the WIP limit.
[[hygiene.rules]]
builtin = "wip-limit"
severity = "medium"
[hygiene.rules.params]
max_in_progress_per_assignee = 3
"#;

fn rules_init(force: bool, rules_path: Option<&Path>, output: &OutputOptions) -> Result<()> {
    // `--rules` / LINEAR_CLI_HYGIENE_RULES redirect where the starter lands.
    let path = hygiene_rules_source(rules_path)?.path().to_path_buf();
    if path.exists() && !force {
        return Err(CliError::general(format!(
            "{} already exists. Use --force to overwrite.",
            path.display()
        ))
        .into());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, STARTER_HYGIENE_TOML)?;
    if output.is_json() || output.has_template() {
        print_json_owned(
            json!({ "created": true, "path": path.display().to_string() }),
            output,
        )?;
    } else {
        println!("Wrote {}", path.display());
        println!("Edit the rules, then run `linear hygiene check`.");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// `hygiene report` (R23)
// ---------------------------------------------------------------------------

struct ReportRow {
    group: String,
    count: usize,
}

impl_tabled!(ReportRow {
    group => "Group",
    count => "Findings",
});

async fn report(
    by: &str,
    args: ScopeArgs,
    rules_path: Option<&Path>,
    output: &OutputOptions,
    agent_opts: AgentOptions,
) -> Result<()> {
    if !matches!(by, "rule" | "owner" | "team" | "entity") {
        return Err(CliError::general(format!(
            "Invalid --by '{by}' (expected rule, owner, team, or entity)"
        ))
        .into());
    }
    let Some(run) = run_pipeline(&args, rules_path, output).await? else {
        return print_no_rules_hint(output, agent_opts);
    };

    let mut groups: BTreeMap<String, usize> = BTreeMap::new();
    for finding in &run.findings {
        *groups.entry(report_group_key(finding, by)).or_insert(0) += 1;
    }

    if output.is_json() || output.has_template() {
        // JSON object keyed by group (design doc OQ2).
        print_json_owned(json!(groups), output)?;
    } else if groups.is_empty() {
        println!("No hygiene findings.");
    } else {
        let mut rows: Vec<ReportRow> = groups
            .into_iter()
            .map(|(group, count)| ReportRow { group, count })
            .collect();
        rows.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.group.cmp(&b.group)));
        println!("{}", Table::new(rows));
    }
    Ok(())
}

fn report_group_key(finding: &Finding, by: &str) -> String {
    match by {
        "owner" => finding
            .owner
            .as_ref()
            .and_then(|o| o.display_name.clone().or_else(|| o.name.clone()))
            .unwrap_or_else(|| "unassigned".to_string()),
        // Team is not part of the finding shape; for issues the identifier
        // prefix (ENG-123 → ENG) is the team key, other entities group as "-".
        "team" => match finding.entity.entity_type {
            EntityKind::Issue => finding
                .entity
                .identifier
                .split_once('-')
                .map(|(team, _)| team.to_string())
                .unwrap_or_else(|| "-".to_string()),
            _ => "-".to_string(),
        },
        "entity" => finding.entity.entity_type.as_str().to_string(),
        _ => finding.rule.clone(),
    }
}

// ---------------------------------------------------------------------------
// Fix execution plans (shared by `fix` and `apply`)
// ---------------------------------------------------------------------------

/// One resolved mutation step, decoded from an engine-generated fix command
/// string. `fix`/`apply` execute these via direct GraphQL mutations (R24/R25);
/// the command strings themselves exist for out-of-band consumers (R29).
#[derive(Debug, Clone, PartialEq)]
enum ExecStep {
    UpdateIssue { sets: Vec<(String, String)> },
    UpdateProject { sets: Vec<(String, String)> },
    UpdateInitiative { sets: Vec<(String, String)> },
    CommentIssue { body: String },
}

/// Split an engine-generated command string into shell-style tokens
/// (double quotes with `\` escapes, matching the engine's `shell_arg`).
fn tokenize_command(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    let mut in_quotes = false;
    let mut chars = command.chars();
    while let Some(c) = chars.next() {
        if in_quotes {
            match c {
                '\\' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                }
                '"' => in_quotes = false,
                other => current.push(other),
            }
        } else {
            match c {
                '"' => {
                    in_quotes = true;
                    in_token = true;
                }
                c if c.is_whitespace() => {
                    if in_token {
                        tokens.push(std::mem::take(&mut current));
                        in_token = false;
                    }
                }
                other => {
                    current.push(other);
                    in_token = true;
                }
            }
        }
    }
    if in_token {
        tokens.push(current);
    }
    tokens
}

/// Decode one engine-generated fix command (possibly `cmd1 && cmd2`) into
/// executable steps. The grammar is closed: only the engine writes these.
fn plan_from_command(command: &str) -> Result<Vec<ExecStep>> {
    let tokens = tokenize_command(command);
    let mut steps = Vec::new();
    for segment in tokens.split(|t| t == "&&") {
        if segment.is_empty() {
            continue;
        }
        let step = decode_segment(segment).ok_or_else(|| {
            CliError::general(format!(
                "Unsupported fix command: {command}. Re-run `linear hygiene check` and retry."
            ))
        })?;
        steps.push(step);
    }
    if steps.is_empty() {
        return Err(CliError::general(format!("Empty fix command: {command}")).into());
    }
    Ok(steps)
}

fn decode_segment(tokens: &[String]) -> Option<ExecStep> {
    let (head, rest) = tokens.split_at(tokens.len().min(4));
    let [bin, noun, verb, _target] = head else {
        return None;
    };
    if bin != "linear" {
        return None;
    }
    match (noun.as_str(), verb.as_str()) {
        ("i" | "issues", "update") => Some(ExecStep::UpdateIssue {
            sets: decode_sets(rest, EntityKind::Issue)?,
        }),
        ("p" | "projects", "update") => Some(ExecStep::UpdateProject {
            sets: decode_sets(rest, EntityKind::Project)?,
        }),
        ("init" | "initiatives", "update") => Some(ExecStep::UpdateInitiative {
            sets: decode_sets(rest, EntityKind::Initiative)?,
        }),
        ("cm" | "comments", "create") => match rest {
            [flag, body] if flag == "-b" => Some(ExecStep::CommentIssue { body: body.clone() }),
            _ => None,
        },
        _ => None,
    }
}

/// Reverse the engine's flag mapping back into `(field, value)` pairs.
fn decode_sets(tokens: &[String], kind: EntityKind) -> Option<Vec<(String, String)>> {
    let flag_to_field: BTreeMap<&str, &str> = settable_fields(kind)
        .into_iter()
        .filter_map(|field| settable_field_flag(kind, field).map(|flag| (flag, field)))
        .collect();
    let mut sets = Vec::new();
    let mut iter = tokens.iter();
    while let Some(flag) = iter.next() {
        let field = flag_to_field.get(flag.as_str())?;
        let value = iter.next()?;
        sets.push((field.to_string(), value.clone()));
    }
    if sets.is_empty() {
        return None;
    }
    Some(sets)
}

/// Execute the steps of one fix against the API. `entity_id` is the entity
/// UUID from the finding; comment bodies must already be resolved.
async fn execute_steps(
    client: &LinearClient,
    entity_id: &str,
    steps: &[ExecStep],
    output: &OutputOptions,
) -> Result<()> {
    for step in steps {
        match step {
            ExecStep::UpdateIssue { sets } => {
                execute_issue_update(client, entity_id, sets, output).await?
            }
            ExecStep::UpdateProject { sets } => {
                execute_project_update(client, entity_id, sets, output).await?
            }
            ExecStep::UpdateInitiative { sets } => {
                execute_initiative_update(client, entity_id, sets).await?
            }
            ExecStep::CommentIssue { body } => execute_comment(client, entity_id, body).await?,
        }
    }
    Ok(())
}

async fn execute_issue_update(
    client: &LinearClient,
    issue_id: &str,
    sets: &[(String, String)],
    output: &OutputOptions,
) -> Result<()> {
    let mut input = json!({});
    for (field, value) in sets {
        match field.as_str() {
            "status" => {
                let team_query = r#"query($id: String!) { issue(id: $id) { team { id } } }"#;
                let result = client
                    .query(team_query, Some(json!({ "id": issue_id })))
                    .await?;
                let team_id = result["data"]["issue"]["team"]["id"]
                    .as_str()
                    .ok_or_else(|| {
                        CliError::not_found(format!(
                            "Could not determine team for issue {issue_id}"
                        ))
                    })?
                    .to_string();
                let state_id =
                    resolve_state_id_cached(client, &team_id, value, &output.cache).await?;
                input["stateId"] = json!(state_id);
            }
            "priority" => input["priority"] = json!(value.parse::<f64>().unwrap_or(0.0) as i64),
            "estimate" => input["estimate"] = json!(value.parse::<f64>().unwrap_or(0.0)),
            "assignee" => {
                input["assigneeId"] = json!(resolve_user_id(client, value, &output.cache).await?)
            }
            "project" => {
                input["projectId"] = json!(resolve_project_id(client, value, &output.cache).await?)
            }
            "dueDate" => input["dueDate"] = json!(value),
            "title" => input["title"] = json!(value),
            "description" => input["description"] = json!(value),
            "labels" => {
                // `linear i update -l` replaces the label set, so applying a
                // label fix must merge with the issue's existing labels.
                let label_id = resolve_label_id(client, value, &output.cache).await?;
                let query = r#"query($id: String!) { issue(id: $id) { labels { nodes { id } } } }"#;
                let result = client.query(query, Some(json!({ "id": issue_id }))).await?;
                let mut label_ids: Vec<String> = result["data"]["issue"]["labels"]["nodes"]
                    .as_array()
                    .map(|nodes| {
                        nodes
                            .iter()
                            .filter_map(|n| n["id"].as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                if !label_ids.contains(&label_id) {
                    label_ids.push(label_id);
                }
                input["labelIds"] = json!(label_ids);
            }
            other => {
                return Err(
                    CliError::general(format!("Unsupported issue fix field '{other}'")).into(),
                )
            }
        }
    }
    let mutation = r#"mutation($id: String!, $input: IssueUpdateInput!) { issueUpdate(id: $id, input: $input) { success } }"#;
    let result = client
        .mutate(mutation, Some(json!({ "id": issue_id, "input": input })))
        .await?;
    if result["data"]["issueUpdate"]["success"].as_bool() != Some(true) {
        anyhow::bail!("issueUpdate reported failure");
    }
    Ok(())
}

async fn execute_project_update(
    client: &LinearClient,
    project_id: &str,
    sets: &[(String, String)],
    output: &OutputOptions,
) -> Result<()> {
    let mut input = json!({});
    for (field, value) in sets {
        match field.as_str() {
            "state" => input["statusId"] = json!(resolve_project_status_id(client, value).await?),
            "lead" => input["leadId"] = json!(resolve_user_id(client, value, &output.cache).await?),
            "startDate" => input["startDate"] = json!(value),
            "targetDate" => input["targetDate"] = json!(value),
            "name" => input["name"] = json!(value),
            "description" => input["description"] = json!(value),
            other => {
                return Err(
                    CliError::general(format!("Unsupported project fix field '{other}'")).into(),
                )
            }
        }
    }
    let mutation = r#"mutation($id: String!, $input: ProjectUpdateInput!) { projectUpdate(id: $id, input: $input) { success } }"#;
    let result = client
        .mutate(mutation, Some(json!({ "id": project_id, "input": input })))
        .await?;
    if result["data"]["projectUpdate"]["success"].as_bool() != Some(true) {
        anyhow::bail!("projectUpdate reported failure");
    }
    Ok(())
}

async fn resolve_project_status_id(client: &LinearClient, status: &str) -> Result<String> {
    let query = r#"query { projectStatuses { id name } }"#;
    let result = client.query(query, None).await?;
    result["data"]["projectStatuses"]
        .as_array()
        .and_then(|statuses| {
            statuses.iter().find_map(|s| {
                s["name"]
                    .as_str()
                    .filter(|name| name.eq_ignore_ascii_case(status))
                    .and_then(|_| s["id"].as_str().map(str::to_string))
            })
        })
        .ok_or_else(|| CliError::not_found(format!("Project status not found: {status}")).into())
}

async fn execute_initiative_update(
    client: &LinearClient,
    initiative_id: &str,
    sets: &[(String, String)],
) -> Result<()> {
    let mut input = json!({});
    for (field, value) in sets {
        match field.as_str() {
            "state" => input["status"] = json!(value),
            "name" => input["name"] = json!(value),
            "description" => input["description"] = json!(value),
            other => {
                return Err(CliError::general(format!(
                    "Unsupported initiative fix field '{other}'"
                ))
                .into())
            }
        }
    }
    let mutation = r#"mutation($id: String!, $input: InitiativeUpdateInput!) { initiativeUpdate(id: $id, input: $input) { success } }"#;
    let result = client
        .mutate(
            mutation,
            Some(json!({ "id": initiative_id, "input": input })),
        )
        .await?;
    if result["data"]["initiativeUpdate"]["success"].as_bool() != Some(true) {
        anyhow::bail!("initiativeUpdate reported failure");
    }
    Ok(())
}

async fn execute_comment(client: &LinearClient, issue_id: &str, body: &str) -> Result<()> {
    let mutation =
        r#"mutation($input: CommentCreateInput!) { commentCreate(input: $input) { success } }"#;
    let result = client
        .mutate(
            mutation,
            Some(json!({ "input": { "issueId": issue_id, "body": body } })),
        )
        .await?;
    if result["data"]["commentCreate"]["success"].as_bool() != Some(true) {
        anyhow::bail!("commentCreate reported failure");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// `hygiene fix` (R24/R31)
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct FixResult {
    key: String,
    status: &'static str,
    command: Option<String>,
    detail: Option<String>,
}

async fn fix(
    args: ScopeArgs,
    rules_path: Option<&Path>,
    output: &OutputOptions,
    agent_opts: AgentOptions,
) -> Result<()> {
    let Some(run) = run_pipeline(&args, rules_path, output).await? else {
        return print_no_rules_hint(output, agent_opts);
    };
    let dry_run = output.dry_run || agent_opts.dry_run;

    // Fix runs replace the artifact like check runs so successful executions
    // can be marked resolved for later `apply` idempotence (R32/R34).
    let mut artifact = RunArtifact::new(run.scope_json.clone(), run.findings.clone(), run.now);
    if !dry_run {
        save_artifact(&artifact)?;
    }

    // Classify: only command-kind fixes are eligible for unattended bulk
    // execution; the CLI never chooses among options or invents content (R31).
    let mut executable: Vec<(String, String)> = Vec::new(); // (key, command)
    let mut results: Vec<FixResult> = Vec::new();
    for finding in &run.findings {
        match &finding.fix {
            Some(Fix::Command { command }) => {
                executable.push((finding.dedupe_key.clone(), command.clone()));
            }
            Some(Fix::Options { .. }) => results.push(FixResult {
                key: finding.dedupe_key.clone(),
                status: "skipped",
                command: None,
                detail: Some("needs_choice".to_string()),
            }),
            Some(Fix::NeedsInput { .. }) => results.push(FixResult {
                key: finding.dedupe_key.clone(),
                status: "skipped",
                command: None,
                detail: Some("needs_input".to_string()),
            }),
            None => results.push(FixResult {
                key: finding.dedupe_key.clone(),
                status: "skipped",
                command: None,
                detail: Some("no_fix".to_string()),
            }),
        }
    }

    if dry_run {
        for (key, command) in executable {
            results.push(FixResult {
                key,
                status: "would_execute",
                command: Some(command),
                detail: None,
            });
        }
        print_fix_results(&results, true, output, agent_opts);
        return Ok(());
    }

    let entity_by_key: BTreeMap<&str, &str> = run
        .findings
        .iter()
        .map(|f| (f.dedupe_key.as_str(), f.entity.id.as_str()))
        .collect();
    let client = LinearClient::new()?;

    let interactive = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    if !agent_opts.yes {
        if output.is_json() || output.has_template() || !interactive {
            return Err(CliError::general(
                "hygiene fix mutates issues; re-run with --yes to execute command-kind fixes, \
                 or --dry-run to preview. Ambiguous fixes stay skipped either way \
                 (apply them one-by-one via `linear hygiene apply KEY --option ACTION`).",
            )
            .into());
        }
        // Interactive table mode: prompt per finding (R24).
        for (key, command) in executable {
            let confirmed = dialoguer::Confirm::new()
                .with_prompt(format!("Fix {key} ({command})?"))
                .default(false)
                .interact()?;
            if !confirmed {
                results.push(FixResult {
                    key,
                    status: "skipped",
                    command: Some(command),
                    detail: Some("declined".to_string()),
                });
                continue;
            }
            let entity_id = entity_by_key.get(key.as_str()).copied().unwrap_or_default();
            results.push(run_one_fix(&client, &key, entity_id, &command, output).await);
        }
    } else {
        // Unattended: execute command-kind fixes concurrently (bulk pattern).
        let executed: Vec<FixResult> = stream::iter(executable)
            .map(|(key, command)| {
                let client = &client;
                let entity_id = entity_by_key.get(key.as_str()).copied().unwrap_or_default();
                async move { run_one_fix(client, &key, entity_id, &command, output).await }
            })
            .buffer_unordered(FIX_CONCURRENCY)
            .collect()
            .await;
        results.extend(executed);
    }

    let mut failed = 0;
    for result in &results {
        if result.status == "executed" {
            artifact.mark_resolved(&result.key);
        } else if result.status == "failed" {
            failed += 1;
        }
    }
    save_artifact(&artifact)?;

    results.sort_by(|a, b| a.key.cmp(&b.key));
    print_fix_results(&results, false, output, agent_opts);
    if failed > 0 {
        return Err(
            CliError::general(format!("{failed} of {} fix(es) failed", results.len())).into(),
        );
    }
    Ok(())
}

async fn run_one_fix(
    client: &LinearClient,
    key: &str,
    entity_id: &str,
    command: &str,
    output: &OutputOptions,
) -> FixResult {
    let outcome = match plan_from_command(command) {
        Ok(steps) => execute_steps(client, entity_id, &steps, output).await,
        Err(e) => Err(e),
    };
    match outcome {
        Ok(()) => FixResult {
            key: key.to_string(),
            status: "executed",
            command: Some(command.to_string()),
            detail: None,
        },
        Err(e) => FixResult {
            key: key.to_string(),
            status: "failed",
            command: Some(command.to_string()),
            detail: Some(e.to_string()),
        },
    }
}

fn print_fix_results(
    results: &[FixResult],
    dry_run: bool,
    output: &OutputOptions,
    agent_opts: AgentOptions,
) {
    let executed = results.iter().filter(|r| r.status == "executed").count();
    let skipped = results.iter().filter(|r| r.status == "skipped").count();
    let failed = results.iter().filter(|r| r.status == "failed").count();

    if output.is_json() || output.has_template() {
        let json_results: Vec<Value> = results
            .iter()
            .map(|r| {
                json!({
                    "dedupeKey": r.key,
                    "status": r.status,
                    "command": r.command,
                    "reason": r.detail,
                })
            })
            .collect();
        let payload = json!({
            "action": "fix",
            "dryRun": dry_run,
            "results": json_results,
            "summary": {
                "total": results.len(),
                "executed": executed,
                "skipped": skipped,
                "failed": failed,
            }
        });
        if let Err(err) = print_json_owned(payload, output) {
            eprintln!("Error: {err}");
        }
        return;
    }

    for result in results {
        match result.status {
            "executed" => println!("  {} {} fixed", "+".green(), result.key.cyan()),
            "would_execute" => println!(
                "  {} {} would execute: {}",
                ">>".cyan(),
                result.key.cyan(),
                result.command.as_deref().unwrap_or("-")
            ),
            "failed" => println!(
                "  {} {} failed: {}",
                "x".red(),
                result.key.cyan(),
                result.detail.as_deref().unwrap_or("unknown error").dimmed()
            ),
            _ => println!(
                "  {} {} skipped ({})",
                "-".yellow(),
                result.key.cyan(),
                result.detail.as_deref().unwrap_or("skipped")
            ),
        }
    }
    if !agent_opts.quiet {
        println!();
        println!(
            "{} Summary: {} executed, {} skipped, {} failed",
            ">>".cyan(),
            executed.to_string().green(),
            skipped,
            if failed > 0 {
                failed.to_string().red().to_string()
            } else {
                failed.to_string()
            }
        );
        if skipped > 0 && !dry_run {
            println!("   Skipped findings need a choice or content: `linear hygiene apply KEY --option ACTION` / `--input \"text\"`.");
        }
    }
}

// ---------------------------------------------------------------------------
// `hygiene apply` (R25/R33/R34)
// ---------------------------------------------------------------------------

struct ApplyPlan {
    key: String,
    /// `None` for already-resolved findings (no-op, R34).
    steps: Option<Vec<ExecStep>>,
    command: String,
    entity_id: String,
}

async fn apply(
    keys: Vec<String>,
    option: Option<String>,
    input: Option<String>,
    rules_path: Option<&Path>,
    output: &OutputOptions,
    agent_opts: AgentOptions,
) -> Result<()> {
    let dry_run = output.dry_run || agent_opts.dry_run;
    let ttl_seconds = load_effective_hygiene_config(rules_path)?
        .map(|c| c.apply_ttl_seconds)
        .unwrap_or_else(|| parse_duration_seconds(DEFAULT_APPLY_TTL).unwrap());

    let Some(mut artifact) = load_artifact()? else {
        return Err(CliError::general(
            "No hygiene run artifact found. Run `linear hygiene check` first.",
        )
        .into());
    };
    let now = Utc::now();
    if artifact.is_expired(ttl_seconds, now) {
        return Err(CliError::general(format!(
            "The last hygiene check is {} old (apply_ttl {}). Re-run `linear hygiene check` and retry.",
            format_duration_compact(artifact.age_seconds(now)),
            format_duration_compact(ttl_seconds),
        ))
        .into());
    }

    // Unknown keys are exit 2 (R27); validate all before mutating anything.
    let unknown: Vec<&String> = keys
        .iter()
        .filter(|key| artifact.find(key).is_none())
        .collect();
    if !unknown.is_empty() {
        return Err(CliError::not_found(format!(
            "Finding(s) not in the last check artifact: {}. Run `linear hygiene check` to refresh.",
            unknown
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))
        .into());
    }

    let mut plans = Vec::new();
    for key in &keys {
        let stored = artifact.find(key).expect("validated above");
        plans.push(build_apply_plan(
            stored,
            option.as_deref(),
            input.as_deref(),
        )?);
    }

    let mut results: Vec<Value> = Vec::new();
    let mut failed = 0;
    let client = if plans.iter().any(|p| p.steps.is_some()) && !dry_run {
        Some(LinearClient::new()?)
    } else {
        None
    };

    for plan in &plans {
        let Some(steps) = &plan.steps else {
            results.push(json!({
                "dedupeKey": plan.key,
                "status": "already_resolved",
            }));
            continue;
        };
        if dry_run {
            results.push(json!({
                "dedupeKey": plan.key,
                "status": "would_apply",
                "command": plan.command,
            }));
            continue;
        }
        let client = client.as_ref().expect("client created for execution");
        match execute_steps(client, &plan.entity_id, steps, output).await {
            Ok(()) => {
                let outcome = artifact.mark_resolved(&plan.key);
                debug_assert_ne!(outcome, MarkOutcome::NotFound);
                results.push(json!({
                    "dedupeKey": plan.key,
                    "status": "applied",
                    "command": plan.command,
                }));
            }
            Err(e) => {
                failed += 1;
                results.push(json!({
                    "dedupeKey": plan.key,
                    "status": "failed",
                    "command": plan.command,
                    "error": e.to_string(),
                }));
            }
        }
    }

    if !dry_run {
        save_artifact(&artifact)?;
    }

    if output.is_json() || output.has_template() {
        print_json_owned(
            json!({ "action": "apply", "dryRun": dry_run, "results": results }),
            output,
        )?;
    } else {
        for result in &results {
            let key = result["dedupeKey"].as_str().unwrap_or("-");
            match result["status"].as_str().unwrap_or("") {
                "applied" => println!("  {} {} applied", "+".green(), key.cyan()),
                "already_resolved" => {
                    println!("  {} {} already resolved (no-op)", "-".yellow(), key.cyan())
                }
                "would_apply" => println!(
                    "  {} {} would apply: {}",
                    ">>".cyan(),
                    key.cyan(),
                    result["command"].as_str().unwrap_or("-")
                ),
                _ => println!(
                    "  {} {} failed: {}",
                    "x".red(),
                    key.cyan(),
                    result["error"].as_str().unwrap_or("unknown error").dimmed()
                ),
            }
        }
    }

    if failed > 0 {
        return Err(
            CliError::general(format!("{failed} of {} apply(s) failed", keys.len())).into(),
        );
    }
    Ok(())
}

/// Resolve one stored finding + `--option`/`--input` into an executable plan,
/// enforcing the per-kind flag requirements of R25.
fn build_apply_plan(
    stored: &StoredFinding,
    option: Option<&str>,
    input: Option<&str>,
) -> Result<ApplyPlan> {
    let key = stored.finding.dedupe_key.clone();
    let entity_id = stored.finding.entity.id.clone();
    if stored.resolved {
        return Ok(ApplyPlan {
            key,
            steps: None,
            command: String::new(),
            entity_id,
        });
    }

    let command = match &stored.finding.fix {
        None => {
            return Err(CliError::general(format!(
                "Finding {key} has no automated fix (visibility only)."
            ))
            .into())
        }
        Some(Fix::Command { command }) => command.clone(),
        Some(Fix::NeedsInput { command }) => {
            if input.is_none() {
                return Err(CliError::general(format!(
                    "Finding {key} needs authored content; pass --input \"text\"."
                ))
                .into());
            }
            command.clone()
        }
        Some(Fix::Options { options }) => {
            let actions: Vec<&str> = options.iter().map(|o| o.action.as_str()).collect();
            let Some(chosen) = option else {
                return Err(CliError::general(format!(
                    "Finding {key} has multiple fix options; pass --option ACTION. Valid actions: {}",
                    actions.join(", ")
                ))
                .into());
            };
            let Some(fix_option) = options.iter().find(|o| o.action == chosen) else {
                return Err(CliError::general(format!(
                    "Invalid --option '{chosen}' for {key}. Valid actions: {}",
                    actions.join(", ")
                ))
                .into());
            };
            if fix_option.needs_input && input.is_none() {
                return Err(CliError::general(format!(
                    "Option '{chosen}' for {key} needs authored content; pass --input \"text\"."
                ))
                .into());
            }
            fix_option.command.clone()
        }
    };

    let mut steps = plan_from_command(&command)?;
    let mut resolved_command = command;
    for step in &mut steps {
        if let ExecStep::CommentIssue { body } = step {
            let Some(input) = input else {
                return Err(CliError::general(format!(
                    "Finding {key} posts a comment; pass --input \"text\"."
                ))
                .into());
            };
            resolved_command = resolved_command.replace(&format!("\"{body}\""), "\"<input>\"");
            *body = input.to_string();
        }
    }

    Ok(ApplyPlan {
        key,
        steps: Some(steps),
        command: resolved_command,
        entity_id,
    })
}

// ---------------------------------------------------------------------------
// `hygiene snooze` (R26)
// ---------------------------------------------------------------------------

struct SnoozeRow {
    key: String,
    until: String,
}

impl_tabled!(SnoozeRow {
    key => "Key",
    until => "Until",
});

fn snooze(
    key: Option<String>,
    duration: Option<String>,
    list: bool,
    clear: bool,
    output: &OutputOptions,
    agent_opts: AgentOptions,
) -> Result<()> {
    let now = Utc::now();
    let mut snoozes = load_snoozes()?;

    if list {
        let active = snoozes.list_active(now);
        if output.is_json() || output.has_template() {
            let items: Vec<Value> = active
                .iter()
                .map(|(key, until)| {
                    json!({
                        "dedupeKey": key,
                        "until": until.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                    })
                })
                .collect();
            print_json_owned(json!(items), output)?;
        } else if active.is_empty() {
            println!("No active snoozes.");
        } else {
            let rows: Vec<SnoozeRow> = active
                .into_iter()
                .map(|(key, until)| SnoozeRow {
                    key,
                    until: until.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                })
                .collect();
            println!("{}", Table::new(rows));
        }
        return Ok(());
    }

    if clear {
        let removed = snoozes.entries.len();
        snoozes.clear();
        save_snoozes(&snoozes)?;
        if output.is_json() || output.has_template() {
            print_json_owned(json!({ "cleared": removed }), output)?;
        } else if !agent_opts.quiet {
            println!("Cleared {removed} snooze(s).");
        }
        return Ok(());
    }

    let key = key.expect("clap enforces key unless --list/--clear");
    let Some(duration) = duration else {
        return Err(CliError::general("Snoozing requires --for DURATION (e.g. --for 2w).").into());
    };
    let Some(seconds) = parse_duration_seconds(&duration) else {
        return Err(CliError::general(format!(
            "Invalid duration '{duration}' (use 90m, 6h, 7d, 2w)."
        ))
        .into());
    };

    // Unknown dedupe keys are exit 2 (R27), validated against the last run.
    let Some(artifact) = load_artifact()? else {
        return Err(CliError::general(
            "No hygiene run artifact found. Run `linear hygiene check` first.",
        )
        .into());
    };
    if artifact.find(&key).is_none() {
        return Err(CliError::not_found(format!(
            "Finding {key} is not in the last check artifact."
        ))
        .into());
    }

    let until = now + chrono::Duration::seconds(seconds as i64);
    snoozes.snooze(&key, until);
    snoozes.prune_expired(now);
    save_snoozes(&snoozes)?;

    if output.is_json() || output.has_template() {
        print_json_owned(
            json!({
                "dedupeKey": key,
                "until": until.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            }),
            output,
        )?;
    } else if !agent_opts.quiet {
        println!("Snoozed {key} for {duration}.");
    }
    Ok(())
}

// Keep an explicit reference so the state module's default dir stays wired.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetch_pagination_clamps_page_size_to_entity_maximum() {
        // Default: entity maximum, full pagination.
        let opts = fetch_pagination(None, PROJECT_FETCH_PAGE_SIZE);
        assert!(opts.all);
        assert_eq!(opts.page_size, Some(PROJECT_FETCH_PAGE_SIZE));
        // Requested sizes are honored below the complexity-safe maximum...
        assert_eq!(
            fetch_pagination(Some(25), ISSUE_FETCH_PAGE_SIZE).page_size,
            Some(25)
        );
        // ...and clamped above it (project fetches must stay under Linear's
        // ~10k complexity budget; see the constant docs for the math).
        assert_eq!(
            fetch_pagination(Some(500), PROJECT_FETCH_PAGE_SIZE).page_size,
            Some(PROJECT_FETCH_PAGE_SIZE)
        );
        assert_eq!(
            fetch_pagination(Some(0), INITIATIVE_FETCH_PAGE_SIZE).page_size,
            Some(1)
        );
    }

    #[test]
    fn tokenize_handles_plain_and_quoted_args() {
        assert_eq!(
            tokenize_command("linear i update ENG-9 -s Todo"),
            vec!["linear", "i", "update", "ENG-9", "-s", "Todo"]
        );
        assert_eq!(
            tokenize_command(r#"linear i update ENG-9 -s "In Progress""#),
            vec!["linear", "i", "update", "ENG-9", "-s", "In Progress"]
        );
        assert_eq!(
            tokenize_command(r#"linear cm create ENG-9 -b "say \"hi\" $now""#),
            vec!["linear", "cm", "create", "ENG-9", "-b", r#"say "hi" $now"#]
        );
    }

    #[test]
    fn plan_decodes_issue_project_initiative_and_comment_commands() {
        assert_eq!(
            plan_from_command("linear i update ENG-9 -s Todo -p 2").unwrap(),
            vec![ExecStep::UpdateIssue {
                sets: vec![
                    ("status".to_string(), "Todo".to_string()),
                    ("priority".to_string(), "2".to_string()),
                ]
            }]
        );
        assert_eq!(
            plan_from_command("linear p update proj-1 --status Completed").unwrap(),
            vec![ExecStep::UpdateProject {
                sets: vec![("state".to_string(), "Completed".to_string())]
            }]
        );
        assert_eq!(
            plan_from_command("linear init update init-1 -s Completed").unwrap(),
            vec![ExecStep::UpdateInitiative {
                sets: vec![("state".to_string(), "Completed".to_string())]
            }]
        );
        assert_eq!(
            plan_from_command(
                r#"linear i update ENG-9 -s "In Progress" && linear cm create ENG-9 -b "<comment>""#
            )
            .unwrap(),
            vec![
                ExecStep::UpdateIssue {
                    sets: vec![("status".to_string(), "In Progress".to_string())]
                },
                ExecStep::CommentIssue {
                    body: "<comment>".to_string()
                },
            ]
        );
    }

    #[test]
    fn plan_rejects_foreign_commands() {
        assert!(plan_from_command("rm -rf /").is_err());
        assert!(plan_from_command("linear i delete ENG-9").is_err());
        assert!(plan_from_command("linear i update ENG-9").is_err());
        assert!(plan_from_command("linear i update ENG-9 --bogus x").is_err());
    }

    #[test]
    fn starter_hygiene_toml_is_valid_and_demonstrates_operators() {
        let config = crate::hygiene::config::parse_hygiene_toml(STARTER_HYGIENE_TOML)
            .expect("starter config must parse cleanly");
        assert!(config.rules.len() >= 8);
        assert!(config
            .rules
            .iter()
            .any(|r| matches!(r, Rule::Builtin(b) if b.kind == BuiltinKind::WipLimit)));
        // Every documented operator appears somewhere in the starter file.
        for op in [
            "eq",
            "in",
            "not_in",
            "missing",
            "older_than",
            "newer_than",
            "past",
            "shorter_than",
            "not_matches",
            "gte",
            "missing_group",
        ] {
            assert!(
                STARTER_HYGIENE_TOML.contains(op),
                "starter file should demonstrate '{op}'"
            );
        }
        assert!(STARTER_HYGIENE_TOML.contains("[hygiene.rules.fix]"));
    }

    #[test]
    fn report_group_keys() {
        use crate::hygiene::config::Severity;
        use crate::hygiene::engine::FindingEntity;
        let finding = Finding {
            dedupe_key: "r:ENG-1".into(),
            rule: "r".into(),
            severity: Severity::High,
            entity: FindingEntity {
                entity_type: EntityKind::Issue,
                id: "uuid".into(),
                identifier: "ENG-1".into(),
                title: "T".into(),
                url: None,
            },
            owner: None,
            summary: "s".into(),
            evidence: json!({}),
            fix: None,
        };
        assert_eq!(report_group_key(&finding, "rule"), "r");
        assert_eq!(report_group_key(&finding, "owner"), "unassigned");
        assert_eq!(report_group_key(&finding, "team"), "ENG");
        assert_eq!(report_group_key(&finding, "entity"), "issue");
    }

    #[test]
    fn and_filter_combinations() {
        assert_eq!(and_filter(vec![]), None);
        assert_eq!(and_filter(vec![json!({"a": 1})]), Some(json!({"a": 1})));
        assert_eq!(
            and_filter(vec![json!({"a": 1}), json!({"b": 2})]),
            Some(json!({"and": [{"a": 1}, {"b": 2}]}))
        );
    }
}
