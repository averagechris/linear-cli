use anyhow::Result;
use clap::{Args, Subcommand};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

use crate::api;
use crate::cache::{Cache, CacheOptions, CacheType};
use crate::config;
use crate::output::{print_json_owned, OutputOptions};
use crate::pagination::{paginate_nodes, PaginationOptions};
use crate::text::is_uuid;
use crate::AgentOptions;

/// Page size for context resource refreshes (Linear's maximum page size).
const CONTEXT_FETCH_PAGE_SIZE: usize = 250;

/// Fetch every page of a context resource. These caches back agent-facing
/// option lists, so truncating large workspaces would silently hide options.
async fn fetch_all_context_nodes(query: &str, root: &str) -> Result<Vec<Value>> {
    let client = api::LinearClient::new()?;
    paginate_nodes(
        &client,
        query,
        Map::new(),
        &["data", root, "nodes"],
        &["data", root, "pageInfo"],
        &PaginationOptions {
            all: true,
            ..Default::default()
        },
        CONTEXT_FETCH_PAGE_SIZE,
    )
    .await
}

#[derive(Subcommand)]
pub enum ContextCommands {
    /// Initialize repo-local context in .linear.toml
    #[command(after_help = r#"EXAMPLES:
    linear context init --team EPD
    linear ctx init --team EPD --status Spec --required-label-group domain --required-label-group type
    linear ctx suggest --team EPD --status Spec > .linear.toml"#)]
    Init(ContextInitArgs),
    /// Print a project config skeleton with suggested policies
    Suggest {
        /// Suggested default team
        #[arg(short, long)]
        team: Option<String>,
        /// Suggested default starting status
        #[arg(short, long)]
        status: Option<String>,
    },
    /// Show cached/discoverable options guidance for labels, projects, initiatives, etc.
    Options {
        /// Resource to list: labels, projects, initiatives, statuses, teams
        resource: String,
        /// Label group key for `labels`, e.g. domain or type
        #[arg(long)]
        group: Option<String>,
        /// Refresh the resource from Linear before returning options
        #[arg(long)]
        refresh: bool,
    },
    /// Refresh cached context option metadata from Linear
    Refresh {
        /// Resources to refresh: labels, projects, initiatives, statuses, teams. Omit for all.
        resources: Vec<String>,
    },
    /// Show context option cache status
    CacheStatus,
}

#[derive(Args, Default)]
pub struct ContextInitArgs {
    /// Default team key/name/id for this repository
    #[arg(short, long)]
    pub team: Option<String>,
    /// Default starting status for new issues in this repository
    #[arg(short, long)]
    pub status: Option<String>,
    /// Explicit labels that are always safe to apply (repeat or comma-separate)
    #[arg(long = "default-label", value_delimiter = ',')]
    pub default_labels: Vec<String>,
    /// Required label group; creates an infer-or-ask exactly-one policy (repeatable)
    #[arg(long = "required-label-group")]
    pub required_label_groups: Vec<String>,
    /// Optional label group; creates a suggest/many policy (repeatable)
    #[arg(long = "optional-label-group")]
    pub optional_label_groups: Vec<String>,
    /// Default label for the execution label group, e.g. agentic
    #[arg(long = "execution-label")]
    pub execution_label: Option<String>,
    /// Estimation scale name, e.g. fibonacci
    #[arg(long = "estimate-scale")]
    pub estimate_scale: Option<String>,
    /// Estimation values (comma-separated)
    #[arg(
        long = "estimate-values",
        value_delimiter = ',',
        default_value = "0,1,2,3,5,8"
    )]
    pub estimate_values: Vec<f64>,
    /// Estimation guidance/rubric text
    #[arg(long = "estimate-guidance")]
    pub estimate_guidance: Option<String>,
    /// Mark estimates as required before entering active/cycle work
    #[arg(long)]
    pub require_estimate: bool,
    /// Add an agent instruction/policy note (repeatable)
    #[arg(long = "agent-instruction")]
    pub agent_instructions: Vec<String>,
    /// Overwrite an existing .linear.toml file
    #[arg(long)]
    pub force: bool,
}

pub async fn handle(
    action: Option<ContextCommands>,
    output: &OutputOptions,
    agent_opts: AgentOptions,
    retry: u32,
) -> Result<()> {
    match action {
        Some(ContextCommands::Init(args)) => init(args, output, agent_opts),
        Some(ContextCommands::Suggest { team, status }) => suggest(team, status, output),
        Some(ContextCommands::Options {
            resource,
            group,
            refresh,
        }) => options(&resource, group.as_deref(), refresh, output).await,
        Some(ContextCommands::Refresh { resources }) => refresh_resources(&resources, output).await,
        Some(ContextCommands::CacheStatus) => cache_status(output),
        None => show(output, agent_opts, retry).await,
    }
}

/// Show resolved context: current branch issue, defaults, policies, and hints.
async fn show(output: &OutputOptions, agent_opts: AgentOptions, retry: u32) -> Result<()> {
    let context = config::resolved_context()?;
    let branch = crate::vcs::run_git_command(&["rev-parse", "--abbrev-ref", "HEAD"]).ok();
    let issue_id = branch
        .as_deref()
        .and_then(crate::vcs::extract_issue_from_branch);

    if agent_opts.id_only {
        let issue_id = issue_id
            .ok_or_else(|| anyhow::anyhow!("No Linear issue ID found in the current branch"))?;
        if output.is_json() || output.has_template() {
            print_json_owned(json!(issue_id), output)?;
        } else {
            println!("{}", issue_id);
        }
        return Ok(());
    }

    let hints = build_context_hints(&context);
    let agent_instructions = build_agent_instructions(&context, &hints);

    if output.is_json() || output.has_template() {
        let issue = match issue_id.as_ref() {
            Some(issue_id) => fetch_context_issue(issue_id, retry).await,
            None => None,
        };
        print_json_owned(
            json!({
                "branch": branch,
                "issue_id": issue_id,
                "found": issue.is_some(),
                "issue": issue,
                "profile": config::current_profile().ok(),
                "sources": {
                    "user_configured": !context.user.is_empty(),
                    "project_configured": context.project.as_ref().is_some_and(|c| !c.is_empty()),
                    "project_file": context.project_file.as_ref().map(|p| p.display().to_string()),
                },
                "contextVersion": if context.resolved.version == 0 { 1 } else { context.resolved.version },
                "defaults": context.resolved.defaults.clone(),
                "issueCreate": {
                    "onAmbiguity": context.resolved.issue_create.on_ambiguity.clone(),
                    "fields": context.resolved.issue_create.fields.clone(),
                    "estimation": context.resolved.issue_create.estimation.clone(),
                    "labelGroups": context.resolved.label_groups.iter().map(label_group_json).collect::<Vec<_>>(),
                },
                "cache": context.resolved.cache.clone(),
                "configured": !context.resolved.is_empty(),
                "completeness": {
                    "hasDefaultTeam": context.resolved.defaults.team.is_some(),
                    "hasDefaultStatus": context.resolved.defaults.status.is_some(),
                    "hasLabelPolicy": !context.resolved.label_groups.is_empty(),
                    "hasIssueCreationPolicy": !context.resolved.issue_create.is_empty()
                        || !context.resolved.label_groups.is_empty(),
                },
                "hints": hints,
                "agentInstructions": agent_instructions,
            }),
            output,
        )?;
    } else {
        print_context_table(
            &context,
            branch.as_deref(),
            issue_id.as_deref(),
            &hints,
            agent_opts,
        );
    }

    Ok(())
}

async fn fetch_context_issue(issue_id: &str, retry: u32) -> Option<Value> {
    let client = api::LinearClient::new_with_retry(retry).ok()?;
    let query = r#"
        query($id: String!) {
            issue(id: $id) {
                id
                identifier
                title
                state { id name type }
                team { id key name }
                assignee { name }
                priority
                url
            }
        }
    "#;

    let data = client
        .query(query, Some(json!({ "id": issue_id })))
        .await
        .ok()?;
    let issue = data["data"]["issue"].clone();
    if issue.is_null() {
        None
    } else {
        Some(issue)
    }
}

fn build_context_hints(context: &config::ResolvedLinearContext) -> Vec<Value> {
    let mut hints = Vec::new();
    if context.resolved.defaults.team.is_none() {
        hints.push(json!({
            "level": "info",
            "code": "missing_default_team",
            "message": "No default team is configured.",
            "action": "linear config set default-team TEAM"
        }));
    }
    if context.project_file.is_none() {
        hints.push(json!({
            "level": "info",
            "code": "missing_project_context",
            "message": "No repository context file was found.",
            "action": "linear context suggest --team TEAM"
        }));
    }
    if context.resolved.label_groups.is_empty() {
        hints.push(json!({
            "level": "info",
            "code": "missing_label_policy",
            "message": "No label group policy is configured.",
            "action": "linear context init --required-label-group domain --required-label-group type"
        }));
    }
    hints
}

/// Shared discovery/presentation contract for a label group policy. Used both by
/// `linear context` (labelGroups) and `linear context options labels` so agents
/// see one consistent shape.
fn label_group_discovery_json(group: &config::LabelGroupPolicy) -> Value {
    let command = group.discovery.clone().unwrap_or_else(|| {
        format!(
            "linear context options labels --group {} --output json --compact",
            group.key
        )
    });
    json!({
        "command": command,
        "cacheKey": format!("labels.{}", group.key),
        "presentationMax": 12,
        "multiSelect": group.cardinality.as_deref() == Some("many"),
        "includeNone": !group.required,
        "prompt": group.ask_prompt.clone(),
    })
}

fn label_group_json(group: &config::LabelGroupPolicy) -> Value {
    json!({
        "key": group.key,
        "linearGroup": group.linear_group.clone(),
        "required": group.required,
        "cardinality": group.cardinality,
        "mode": group.mode.clone(),
        "default": group.default.clone(),
        "guidance": group.guidance.clone(),
        "options": group.options.clone(),
        "hints": group.hints.clone(),
        "discovery": label_group_discovery_json(group),
    })
}

fn build_agent_instructions(
    context: &config::ResolvedLinearContext,
    hints: &[Value],
) -> Vec<String> {
    let mut instructions = Vec::new();
    if let Some(team) = &context.resolved.defaults.team {
        instructions.push(format!(
            "Default to team {} when creating issues unless the user specifies another team.",
            team
        ));
    } else if hints.iter().any(|h| h["code"] == "missing_default_team") {
        instructions.push(
            "No default team is configured; ask the user for a team before creating issues."
                .to_string(),
        );
    }
    if let Some(status) = &context.resolved.defaults.status {
        instructions.push(format!(
            "Default new issues to status {} unless the user specifies another status.",
            status
        ));
    }
    instructions.push(
        "Use `linear i start ISSUE_ID` to begin work, or `linear i update ISSUE_ID -s \"STATUS NAME\"`; the CLI resolves team-scoped status names to Linear's required stateId UUIDs."
            .to_string(),
    );
    for group in &context.resolved.label_groups {
        if group.required {
            instructions.push(format!(
                "Issue creation requires {} label group '{}' ({}); infer only when clear, otherwise fetch options and ask the user.",
                group.cardinality.as_deref().unwrap_or("a"),
                group.key,
                group.ask_prompt.as_deref().unwrap_or("ask if ambiguous")
            ));
        }
    }
    if !context.resolved.issue_create.estimation.is_empty() {
        instructions.push(
            "Use the configured estimation rubric; infer only when reasonably clear and ask when uncertain."
                .to_string(),
        );
    }
    instructions.extend(context.resolved.agent_instructions.clone());
    instructions
}

fn print_context_table(
    context: &config::ResolvedLinearContext,
    branch: Option<&str>,
    issue_id: Option<&str>,
    hints: &[Value],
    agent_opts: AgentOptions,
) {
    println!("Linear context");
    println!();
    println!("Branch: {}", branch.unwrap_or("not detected"));
    println!("Current issue: {}", issue_id.unwrap_or("not detected"));
    println!(
        "Default team: {}",
        context
            .resolved
            .defaults
            .team
            .as_deref()
            .unwrap_or("not configured")
    );
    println!(
        "Default status: {}",
        context
            .resolved
            .defaults
            .status
            .as_deref()
            .unwrap_or("not configured")
    );
    println!(
        "Explicit default labels: {}",
        if context.resolved.defaults.labels.is_empty() {
            "not configured".to_string()
        } else {
            context.resolved.defaults.labels.join(", ")
        }
    );
    println!(
        "Label policies: {}",
        if context.resolved.label_groups.is_empty() {
            "not configured".to_string()
        } else {
            context
                .resolved
                .label_groups
                .iter()
                .map(|group| {
                    format!(
                        "{} ({}, {})",
                        group.key,
                        group.cardinality.as_deref().unwrap_or("many"),
                        if group.required {
                            "required"
                        } else {
                            "optional"
                        }
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        }
    );
    if let Some(scale) = &context.resolved.issue_create.estimation.scale {
        println!("Estimation: {}", scale);
    }
    println!(
        "Project context file: {}",
        context
            .project_file
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "not configured".to_string())
    );

    if !agent_opts.quiet && !hints.is_empty() {
        println!();
        println!("Tip: Configure defaults so agents and commands need less repeated context.");
        for hint in hints.iter().take(2) {
            if let Some(action) = hint["action"].as_str() {
                println!("  {}", action);
            }
        }
    }
}

fn init(args: ContextInitArgs, output: &OutputOptions, agent_opts: AgentOptions) -> Result<()> {
    let path = std::env::current_dir()?.join(".linear.toml");
    if path.exists() && !args.force {
        anyhow::bail!(
            "{} already exists. Use --force to overwrite or edit it directly.",
            path.display()
        );
    }
    let context = build_context_config(args);
    if context.is_empty() {
        anyhow::bail!("Provide at least one context value, e.g. `linear context init --team TEAM`");
    }
    config::write_project_context(&path, &context)?;

    if output.is_json() || output.has_template() {
        print_json_owned(
            json!({
                "created": true,
                "path": path.display().to_string(),
                "context": context,
            }),
            output,
        )?;
    } else if !agent_opts.quiet {
        println!("Wrote {}", path.display());
        println!("Run `linear context` to inspect resolved defaults.");
    }
    Ok(())
}

fn suggest(team: Option<String>, status: Option<String>, output: &OutputOptions) -> Result<()> {
    let context = build_context_config(ContextInitArgs {
        team,
        status,
        required_label_groups: vec!["domain".to_string(), "type".to_string()],
        execution_label: Some("agentic".to_string()),
        estimate_scale: Some("fibonacci".to_string()),
        estimate_values: vec![0.0, 1.0, 2.0, 3.0, 5.0, 8.0],
        estimate_guidance: Some(
            "Use relative complexity, not hours. 1=tiny, 2=small, 3=medium, 5=large, 8=split. Ask when uncertain."
                .to_string(),
        ),
        require_estimate: true,
        agent_instructions: vec![
            "Do not invent projects, initiatives, or labels; fetch options and ask when ambiguous."
                .to_string(),
        ],
        ..Default::default()
    });

    if output.is_json() || output.has_template() {
        print_json_owned(json!({ "context": context }), output)?;
    } else {
        println!(
            "{}",
            toml::to_string_pretty(&config::ProjectContextFile { context })?
        );
    }
    Ok(())
}

async fn options(
    resource: &str,
    group: Option<&str>,
    refresh: bool,
    output: &OutputOptions,
) -> Result<()> {
    let context = config::resolved_context()?;
    let resource = resource.to_lowercase();
    if refresh {
        refresh_context_resource(&resource, &context).await?;
    }
    let value = match resource.as_str() {
        "labels" | "label" => {
            let cached_labels = context_cached_array("labels", &context)?;
            let groups: Vec<_> = context
                .resolved
                .label_groups
                .iter()
                .filter(|policy| group.is_none_or(|wanted| policy.key == wanted))
                .map(|policy| {
                    let options = label_options_for_policy(policy, &cached_labels);
                    json!({
                        "key": policy.key,
                        "linearGroup": policy.linear_group.clone(),
                        "required": policy.required,
                        "cardinality": policy.cardinality.clone(),
                        "mode": policy.mode.clone(),
                        "options": options,
                        "hints": policy.hints.clone(),
                        "discovery": label_group_discovery_json(policy),
                    })
                })
                .collect();
            json!({
                "resource": if let Some(group) = group { format!("labels.{group}") } else { "labels".to_string() },
                "cache": context_cache_json("labels", &context)?,
                "labelGroups": groups,
            })
        }
        "projects" | "project" => json!({
            "resource": "projects",
            "cache": context_cache_json("projects", &context)?,
            "options": context_cached_array("projects", &context)?,
            "discovery": {
                "command": "linear projects list --output json --compact --fields id,name,state.name,url",
                "presentationMax": 8,
                "includeNone": true,
                "prompt": "Which project should this issue belong to?"
            }
        }),
        "initiatives" | "initiative" => json!({
            "resource": "initiatives",
            "cache": context_cache_json("initiatives", &context)?,
            "options": context_cached_array("initiatives", &context)?,
            "discovery": {
                "command": "linear initiatives list --output json --compact --fields id,name,status,url",
                "presentationMax": 6,
                "includeNone": true,
                "prompt": "Should this issue be associated with an initiative?"
            }
        }),
        "statuses" | "status" => json!({
            "resource": "statuses",
            "cache": context_cache_json("statuses", &context)?,
            "options": context_cached_statuses(&context)?,
            "discovery": { "command": "linear statuses list -t TEAM --output json --compact" }
        }),
        "teams" | "team" => json!({
            "resource": "teams",
            "cache": context_cache_json("teams", &context)?,
            "options": context_cached_array("teams", &context)?,
            "discovery": { "command": "linear teams list --output json --compact" }
        }),
        _ => anyhow::bail!("Unknown context options resource: {}", resource),
    };

    if output.is_json() || output.has_template() {
        print_json_owned(value, output)?;
    } else {
        println!("{}", serde_json::to_string_pretty(&value)?);
    }
    Ok(())
}

async fn refresh_resources(resources: &[String], output: &OutputOptions) -> Result<()> {
    let context = config::resolved_context()?;
    let resources: Vec<String> = if resources.is_empty() {
        vec!["labels", "projects", "initiatives", "statuses", "teams"]
            .into_iter()
            .map(ToString::to_string)
            .collect()
    } else {
        resources.iter().map(|r| r.to_lowercase()).collect()
    };

    let mut results = Vec::new();
    for resource in resources {
        let result = match refresh_context_resource(&resource, &context).await {
            Ok(count) => json!({
                "resource": resource,
                "refreshed": true,
                "count": count,
            }),
            Err(error) => json!({
                "resource": resource,
                "refreshed": false,
                "error": error.to_string(),
            }),
        };
        results.push(result);
    }

    // Partial failures are reported per-resource below; only a complete
    // failure (e.g. bad resource name, no connectivity) is a hard error.
    if !results.is_empty()
        && results
            .iter()
            .all(|r| r["refreshed"].as_bool() != Some(true))
    {
        let reasons: Vec<String> = results
            .iter()
            .map(|r| {
                format!(
                    "{}: {}",
                    r["resource"].as_str().unwrap_or("resource"),
                    r["error"].as_str().unwrap_or("refresh failed")
                )
            })
            .collect();
        anyhow::bail!(
            "No context resources could be refreshed ({})",
            reasons.join("; ")
        );
    }

    if output.is_json() || output.has_template() {
        print_json_owned(json!(results), output)?;
    } else {
        for result in &results {
            if result["refreshed"].as_bool().unwrap_or(false) {
                println!(
                    "{}: refreshed {} items",
                    result["resource"].as_str().unwrap_or("resource"),
                    result["count"].as_u64().unwrap_or(0)
                );
            } else {
                println!(
                    "{}: {}",
                    result["resource"].as_str().unwrap_or("resource"),
                    result["error"].as_str().unwrap_or("refresh failed")
                );
            }
        }
    }
    Ok(())
}

fn cache_status(output: &OutputOptions) -> Result<()> {
    let context = config::resolved_context()?;
    let resources = ["labels", "projects", "initiatives", "statuses", "teams"];
    let statuses: Vec<_> = resources
        .iter()
        .map(|resource| {
            context_cache_json(resource, &context).unwrap_or_else(|error| {
                json!({ "resource": resource, "status": "error", "error": error.to_string() })
            })
        })
        .collect();

    if output.is_json() || output.has_template() {
        print_json_owned(json!(statuses), output)?;
    } else {
        for status in statuses {
            println!(
                "{}: {} ({} items, age: {})",
                status["resource"].as_str().unwrap_or("resource"),
                status["status"].as_str().unwrap_or("unknown"),
                status["count"].as_u64().unwrap_or(0),
                status["ageSeconds"]
                    .as_u64()
                    .map(|age| format!("{}s", age))
                    .unwrap_or_else(|| "-".to_string())
            );
        }
    }
    Ok(())
}

async fn refresh_context_resource(
    resource: &str,
    context: &config::ResolvedLinearContext,
) -> Result<usize> {
    let resource = normalize_context_resource(resource)?;
    let ttl = context_ttl_seconds(resource, context);
    let cache = Cache::with_ttl(ttl)?;
    match resource {
        "labels" => {
            let data = fetch_context_labels().await?;
            let count = data.len();
            cache.set(CacheType::Labels, json!(data))?;
            Ok(count)
        }
        "projects" => {
            let data = fetch_context_projects().await?;
            let count = data.len();
            cache.set(CacheType::Projects, json!(data))?;
            Ok(count)
        }
        "initiatives" => {
            let data = fetch_context_initiatives().await?;
            let count = data.len();
            cache.set(CacheType::Initiatives, json!(data))?;
            Ok(count)
        }
        "statuses" => {
            let team =
                context.resolved.defaults.team.as_deref().ok_or_else(|| {
                    anyhow::anyhow!("Cannot refresh statuses without a default team")
                })?;
            // Reuse the canonical statuses cache: keyed by team UUID with a
            // `{team_name, states}` payload shared with `linear statuses list`
            // and status-name resolution during issue create/update.
            let client = api::LinearClient::new()?;
            let team_id = api::resolve_team_id(&client, team, &CacheOptions::default()).await?;
            let data = api::fetch_team_statuses_for_cache(&client, &team_id).await?;
            let count = data["states"].as_array().map(|s| s.len()).unwrap_or(0);
            cache.set_keyed(CacheType::Statuses, &team_id, data)?;
            Ok(count)
        }
        "teams" => {
            let data = fetch_context_teams().await?;
            let count = data.len();
            cache.set(CacheType::Teams, json!(data))?;
            Ok(count)
        }
        _ => unreachable!(),
    }
}

fn normalize_context_resource(resource: &str) -> Result<&'static str> {
    match resource.to_lowercase().as_str() {
        "label" | "labels" => Ok("labels"),
        "project" | "projects" => Ok("projects"),
        "initiative" | "initiatives" => Ok("initiatives"),
        "status" | "statuses" => Ok("statuses"),
        "team" | "teams" => Ok("teams"),
        _ => anyhow::bail!("Unknown context resource: {}", resource),
    }
}

fn context_cache_type(resource: &str) -> Result<CacheType> {
    match normalize_context_resource(resource)? {
        "labels" => Ok(CacheType::Labels),
        "projects" => Ok(CacheType::Projects),
        "initiatives" => Ok(CacheType::Initiatives),
        "statuses" => Ok(CacheType::Statuses),
        "teams" => Ok(CacheType::Teams),
        _ => unreachable!(),
    }
}

fn context_ttl_seconds(resource: &str, context: &config::ResolvedLinearContext) -> u64 {
    context
        .resolved
        .cache
        .ttl
        .get(normalize_context_resource(resource).unwrap_or(resource))
        .and_then(|ttl| parse_duration_seconds(ttl))
        .unwrap_or_else(
            || match normalize_context_resource(resource).unwrap_or(resource) {
                "labels" | "statuses" | "teams" => 7 * 24 * 60 * 60,
                "initiatives" => 24 * 60 * 60,
                "projects" => 6 * 60 * 60,
                _ => 60 * 60,
            },
        )
}

fn parse_duration_seconds(input: &str) -> Option<u64> {
    crate::dates::parse_duration_seconds(input)
}

fn context_cache_json(resource: &str, context: &config::ResolvedLinearContext) -> Result<Value> {
    let normalized = normalize_context_resource(resource)?;
    let ttl = context_ttl_seconds(normalized, context);
    let cache = Cache::with_ttl(ttl)?;
    if normalized == "statuses" {
        let count = cached_statuses_entry(context, &cache)
            .and_then(|data| data["states"].as_array().map(|items| items.len()))
            .unwrap_or(0);
        return Ok(json!({
            "resource": normalized,
            "status": if count > 0 { "fresh" } else { "missing" },
            "count": count,
            "ttlSeconds": ttl,
            "refreshCommand": "linear context refresh statuses",
        }));
    }

    let cache_type = context_cache_type(normalized)?;
    let Some(entry) = cache.get_entry(cache_type) else {
        return Ok(json!({
            "resource": normalized,
            "status": "missing",
            "count": 0,
            "ttlSeconds": ttl,
            "refreshCommand": format!("linear context refresh {}", normalized),
        }));
    };
    let count = entry.data.as_array().map(|items| items.len()).unwrap_or(0);
    let fresh = entry.is_valid_with_ttl(ttl);
    Ok(json!({
        "resource": normalized,
        "status": if fresh { "fresh" } else { "stale" },
        "count": count,
        "ttlSeconds": ttl,
        "ageSeconds": entry.age_seconds(),
        "refreshCommand": format!("linear context refresh {}", normalized),
    }))
}

fn context_cached_array(
    resource: &str,
    context: &config::ResolvedLinearContext,
) -> Result<Vec<Value>> {
    let normalized = normalize_context_resource(resource)?;
    let ttl = context_ttl_seconds(normalized, context);
    let cache = Cache::with_ttl(ttl)?;
    let cache_type = context_cache_type(normalized)?;
    Ok(cache
        .get_entry(cache_type)
        .and_then(|entry| entry.data.as_array().cloned())
        .unwrap_or_default())
}

fn context_cached_statuses(context: &config::ResolvedLinearContext) -> Result<Vec<Value>> {
    let ttl = context_ttl_seconds("statuses", context);
    let cache = Cache::with_ttl(ttl)?;
    Ok(cached_statuses_entry(context, &cache)
        .and_then(|data| data["states"].as_array().cloned())
        .unwrap_or_default())
}

/// Read the canonical team-UUID-keyed statuses cache for the default team,
/// resolving team key/name to UUID via the local teams cache only (no network).
fn cached_statuses_entry(context: &config::ResolvedLinearContext, cache: &Cache) -> Option<Value> {
    let team = context.resolved.defaults.team.as_deref()?;
    let team_id = cached_team_id(team)?;
    cache.get_keyed(CacheType::Statuses, &team_id)
}

fn cached_team_id(team: &str) -> Option<String> {
    if is_uuid(team) {
        return Some(team.to_string());
    }
    let teams = Cache::new().ok()?.get(CacheType::Teams)?;
    teams.as_array()?.iter().find_map(|t| {
        let matches = t["key"]
            .as_str()
            .map(|k| k.eq_ignore_ascii_case(team))
            .unwrap_or(false)
            || t["name"]
                .as_str()
                .map(|n| n.eq_ignore_ascii_case(team))
                .unwrap_or(false);
        if matches {
            t["id"].as_str().map(ToString::to_string)
        } else {
            None
        }
    })
}

fn label_options_for_policy(
    policy: &config::LabelGroupPolicy,
    cached_labels: &[Value],
) -> Vec<Value> {
    let group = policy
        .linear_group
        .as_deref()
        .unwrap_or(policy.key.as_str())
        .to_lowercase();
    let mut options: Vec<_> = cached_labels
        .iter()
        .filter(|label| {
            label
                .get("parent")
                .and_then(|parent| parent.get("name"))
                .and_then(|name| name.as_str())
                .map(|name| name.eq_ignore_ascii_case(&group))
                .unwrap_or(false)
        })
        .cloned()
        .collect();

    for option in &policy.options {
        if !options.iter().any(|label| {
            label
                .get("name")
                .and_then(|name| name.as_str())
                .map(|name| name.eq_ignore_ascii_case(&option.name))
                .unwrap_or(false)
        }) {
            options.push(json!({
                "name": option.name,
                "description": option.description,
                "source": "context_config",
            }));
        }
    }
    options
}

async fn fetch_context_teams() -> Result<Vec<Value>> {
    let query = r#"
        query($first: Int, $after: String) {
            teams(first: $first, after: $after) {
                nodes { id name key }
                pageInfo { hasNextPage endCursor }
            }
        }
    "#;
    fetch_all_context_nodes(query, "teams").await
}

async fn fetch_context_projects() -> Result<Vec<Value>> {
    let query = r#"
        query($first: Int, $after: String) {
            projects(first: $first, after: $after, includeArchived: false, orderBy: updatedAt) {
                nodes { id name state url startDate targetDate }
                pageInfo { hasNextPage endCursor }
            }
        }
    "#;
    fetch_all_context_nodes(query, "projects").await
}

async fn fetch_context_initiatives() -> Result<Vec<Value>> {
    // `Initiative.progress` no longer exists in the live schema; keep it out.
    let query = r#"
        query($first: Int, $after: String) {
            initiatives(first: $first, after: $after) {
                nodes { id name status description targetDate health }
                pageInfo { hasNextPage endCursor }
            }
        }
    "#;
    fetch_all_context_nodes(query, "initiatives").await
}

async fn fetch_context_labels() -> Result<Vec<Value>> {
    let query = r#"
        query($first: Int, $after: String) {
            issueLabels(first: $first, after: $after) {
                nodes { id name color description parent { id name } }
                pageInfo { hasNextPage endCursor }
            }
        }
    "#;
    fetch_all_context_nodes(query, "issueLabels").await
}

fn build_context_config(args: ContextInitArgs) -> config::LinearContextConfig {
    let mut fields = HashMap::new();
    fields.insert(
        "team".to_string(),
        config::FieldPolicy {
            mode: config::FieldMode::Default,
            required: true,
            ..Default::default()
        },
    );
    if args.status.is_some() {
        fields.insert(
            "status".to_string(),
            config::FieldPolicy {
                mode: config::FieldMode::Default,
                required: true,
                ..Default::default()
            },
        );
    }
    fields.insert(
        "project".to_string(),
        config::FieldPolicy {
            mode: config::FieldMode::InferOrAsk,
            discovery: Some("projects".to_string()),
            guidance: Some("Use projects for time-bound efforts. Do not invent projects; fetch options and ask when ambiguous.".to_string()),
            ..Default::default()
        },
    );
    fields.insert(
        "initiative".to_string(),
        config::FieldPolicy {
            mode: config::FieldMode::InferOrAsk,
            discovery: Some("initiatives".to_string()),
            guidance: Some("Use initiatives only when strategically relevant or explicitly implied. Do not guess.".to_string()),
            ..Default::default()
        },
    );
    fields.insert(
        "estimate".to_string(),
        config::FieldPolicy {
            mode: config::FieldMode::InferOrAsk,
            required_when: if args.require_estimate {
                Some("before_cycle".to_string())
            } else {
                None
            },
            guidance_ref: Some("issue_create.estimation".to_string()),
            ..Default::default()
        },
    );

    let mut label_groups = Vec::new();
    for group in args.required_label_groups {
        label_groups.push(config::LabelGroupPolicy {
            key: group.clone(),
            linear_group: Some(group.clone()),
            required: true,
            cardinality: Some("exactly_one".to_string()),
            mode: config::FieldMode::InferOrAsk,
            ask_prompt: Some(format!("Which {} label best matches this work?", group)),
            guidance: Some(format!(
                "Every issue must have exactly one {} label. Infer only when clear; otherwise fetch options and ask.",
                group
            )),
            ..Default::default()
        });
    }
    for group in args.optional_label_groups {
        label_groups.push(config::LabelGroupPolicy {
            key: group.clone(),
            linear_group: Some(group.clone()),
            required: false,
            cardinality: Some("many".to_string()),
            mode: config::FieldMode::Suggest,
            ask_prompt: Some(format!("Any {} labels to apply?", group)),
            guidance: Some(
                "Prefer existing labels. Do not create labels automatically.".to_string(),
            ),
            ..Default::default()
        });
    }
    if let Some(label) = args.execution_label {
        label_groups.push(config::LabelGroupPolicy {
            key: "execution".to_string(),
            linear_group: Some("execution".to_string()),
            required: false,
            cardinality: Some("at_most_one".to_string()),
            mode: config::FieldMode::Default,
            default: Some(label),
            guidance: Some(
                "Apply only when this execution-mode label is meaningful for your org.".to_string(),
            ),
            ..Default::default()
        });
    }

    let mut ttl = HashMap::new();
    ttl.insert("labels".to_string(), "7d".to_string());
    ttl.insert("statuses".to_string(), "7d".to_string());
    ttl.insert("projects".to_string(), "6h".to_string());
    ttl.insert("initiatives".to_string(), "24h".to_string());
    ttl.insert("teams".to_string(), "7d".to_string());

    config::LinearContextConfig {
        version: 1,
        defaults: config::ContextDefaults {
            team: args.team,
            status: args.status,
            labels: args.default_labels,
        },
        issue_create: config::IssueCreateContext {
            on_ambiguity: Some("ask".to_string()),
            fields,
            estimation: config::EstimationPolicy {
                scale: args.estimate_scale,
                values: args.estimate_values,
                guidance: args.estimate_guidance,
            },
        },
        label_groups,
        cache: config::ContextCacheConfig { ttl },
        agent_instructions: args.agent_instructions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duration_seconds_supports_suffixes() {
        assert_eq!(parse_duration_seconds("7d"), Some(7 * 24 * 60 * 60));
        assert_eq!(parse_duration_seconds("6h"), Some(6 * 60 * 60));
        assert_eq!(parse_duration_seconds("15m"), Some(15 * 60));
        assert_eq!(parse_duration_seconds("45s"), Some(45));
        assert_eq!(parse_duration_seconds("90"), Some(90));
        assert_eq!(parse_duration_seconds(""), None);
        assert_eq!(parse_duration_seconds("abc"), None);
    }

    #[test]
    fn label_group_discovery_json_is_shared_shape() {
        let policy = config::LabelGroupPolicy {
            key: "domain".to_string(),
            required: true,
            cardinality: Some("exactly_one".to_string()),
            ..Default::default()
        };
        let discovery = label_group_discovery_json(&policy);
        assert_eq!(
            discovery["command"],
            "linear context options labels --group domain --output json --compact"
        );
        assert_eq!(discovery["cacheKey"], "labels.domain");
        assert_eq!(discovery["multiSelect"], false);
        assert_eq!(discovery["includeNone"], false);
    }

    #[test]
    fn build_context_config_creates_required_group_policies() {
        let context = build_context_config(ContextInitArgs {
            team: Some("EPD".to_string()),
            required_label_groups: vec!["domain".to_string()],
            ..Default::default()
        });
        assert_eq!(context.defaults.team.as_deref(), Some("EPD"));
        assert_eq!(context.label_groups.len(), 1);
        assert!(context.label_groups[0].required);
        assert_eq!(
            context.label_groups[0].cardinality.as_deref(),
            Some("exactly_one")
        );
    }
}
