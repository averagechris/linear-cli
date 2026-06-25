use anyhow::Result;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::time::sleep;

use crate::api::{resolve_team_id, LinearClient};
use crate::output::{is_quiet, matches_filters, parse_filters, print_json_owned, OutputOptions};
use crate::pagination::paginate_nodes;
use crate::text::is_uuid;

/// Watch for changes to an issue and print updates
pub async fn watch_issue(id: &str, interval_secs: u64, output: &OutputOptions) -> Result<()> {
    let interval_secs = interval_secs.max(5);
    let client = LinearClient::new()?;

    let query = r#"
        query($id: String!) {
            issue(id: $id) {
                id
                identifier
                title
                updatedAt
                state { name }
                assignee { name }
                priority
                labels { nodes { name } }
            }
        }
    "#;

    let mut last_updated: Option<String> = None;
    let mut iteration = 0;

    eprintln!("Watching {} for changes (Ctrl+C to stop)...\n", id);

    loop {
        let result = client.query(query, Some(json!({ "id": id }))).await?;
        let issue = &result["data"]["issue"];

        if issue.is_null() {
            anyhow::bail!("Issue not found: {}", id);
        }

        let current_updated = issue["updatedAt"].as_str().map(|s| s.to_string());

        // Check if updated
        if last_updated.as_ref() != current_updated.as_ref() {
            if iteration > 0 {
                // Not the first iteration, so this is a change
                if output.is_json() {
                    print_json_owned(
                        json!({
                            "event": "updated",
                            "issue": issue,
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                        }),
                        output,
                    )?;
                } else {
                    println!(
                        "[{}] {} updated - Status: {}, Assignee: {}",
                        chrono::Utc::now().format("%H:%M:%S"),
                        issue["identifier"].as_str().unwrap_or(id),
                        issue["state"]["name"].as_str().unwrap_or("-"),
                        issue["assignee"]["name"].as_str().unwrap_or("Unassigned"),
                    );
                }
            } else if output.is_json() {
                print_json_owned(
                    json!({
                        "event": "initial",
                        "issue": issue,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    }),
                    output,
                )?;
            } else {
                println!(
                    "Initial state: {} - {}",
                    issue["identifier"].as_str().unwrap_or(id),
                    issue["title"].as_str().unwrap_or("")
                );
                println!(
                    "  Status: {}, Assignee: {}",
                    issue["state"]["name"].as_str().unwrap_or("-"),
                    issue["assignee"]["name"].as_str().unwrap_or("Unassigned"),
                );
            }

            last_updated = current_updated;
        }

        iteration += 1;
        sleep(Duration::from_secs(interval_secs)).await;
    }
}

/// Watch for changes to a project and print updates
pub async fn watch_project(id: &str, interval_secs: u64, output: &OutputOptions) -> Result<()> {
    let interval_secs = interval_secs.max(5);
    let client = LinearClient::new()?;

    let query = r#"
        query($id: String!) {
            project(id: $id) {
                id
                name
                state
                progress
                updatedAt
                teams { nodes { key } }
            }
        }
    "#;

    let mut last_updated: Option<String> = None;
    let mut iteration = 0;

    eprintln!("Watching project {} for changes (Ctrl+C to stop)...\n", id);

    loop {
        let result = client.query(query, Some(json!({ "id": id }))).await?;
        let project = &result["data"]["project"];

        if project.is_null() {
            anyhow::bail!("Project not found: {}", id);
        }

        let current_updated = project["updatedAt"].as_str().map(|s| s.to_string());

        if last_updated.as_ref() != current_updated.as_ref() {
            if iteration > 0 {
                if output.is_json() {
                    print_json_owned(
                        json!({
                            "event": "updated",
                            "project": project,
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                        }),
                        output,
                    )?;
                } else {
                    println!(
                        "[{}] {} updated - State: {}, Progress: {:.0}%",
                        chrono::Utc::now().format("%H:%M:%S"),
                        project["name"].as_str().unwrap_or(id),
                        project["state"].as_str().unwrap_or("-"),
                        project["progress"].as_f64().unwrap_or(0.0) * 100.0,
                    );
                }
            } else if output.is_json() {
                print_json_owned(
                    json!({
                        "event": "initial",
                        "project": project,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    }),
                    output,
                )?;
            } else {
                println!(
                    "Initial state: {} - {}",
                    project["name"].as_str().unwrap_or(id),
                    project["state"].as_str().unwrap_or("")
                );
                println!(
                    "  Progress: {:.0}%",
                    project["progress"].as_f64().unwrap_or(0.0) * 100.0,
                );
            }

            last_updated = current_updated;
        }

        iteration += 1;
        sleep(Duration::from_secs(interval_secs)).await;
    }
}

/// Watch for changes to a team and print updates
pub async fn watch_team(team: &str, interval_secs: u64, output: &OutputOptions) -> Result<()> {
    let interval_secs = interval_secs.max(5);
    let client = LinearClient::new()?;

    let query = r#"
        query($id: String!) {
            team(id: $id) {
                id
                name
                key
                updatedAt
                activeCycle {
                    id
                    name
                    progress
                }
                issues(first: 1) {
                    __typename
                }
            }
        }
    "#;

    // Resolve team key to UUID
    let team_id = crate::api::resolve_team_id(&client, team, &output.cache).await?;

    let mut last_updated: Option<String> = None;
    let mut iteration = 0;

    eprintln!("Watching team {} for changes (Ctrl+C to stop)...\n", team);

    loop {
        let result = client.query(query, Some(json!({ "id": team_id }))).await?;
        let team_data = &result["data"]["team"];

        if team_data.is_null() {
            anyhow::bail!("Team not found: {}", team);
        }

        let current_updated = team_data["updatedAt"].as_str().map(|s| s.to_string());

        if last_updated.as_ref() != current_updated.as_ref() {
            if iteration > 0 {
                if output.is_json() {
                    print_json_owned(
                        json!({
                            "event": "updated",
                            "team": team_data,
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                        }),
                        output,
                    )?;
                } else {
                    let cycle_name = team_data["activeCycle"]["name"].as_str().unwrap_or("-");
                    let cycle_progress =
                        team_data["activeCycle"]["progress"].as_f64().unwrap_or(0.0);
                    println!(
                        "[{}] {} updated - Cycle: {} ({:.0}%)",
                        chrono::Utc::now().format("%H:%M:%S"),
                        team_data["name"].as_str().unwrap_or(team),
                        cycle_name,
                        cycle_progress * 100.0,
                    );
                }
            } else if output.is_json() {
                print_json_owned(
                    json!({
                        "event": "initial",
                        "team": team_data,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    }),
                    output,
                )?;
            } else {
                let cycle_name = team_data["activeCycle"]["name"].as_str().unwrap_or("none");
                println!(
                    "Initial state: {} ({})",
                    team_data["name"].as_str().unwrap_or(team),
                    team_data["key"].as_str().unwrap_or(""),
                );
                println!("  Active cycle: {}", cycle_name);
            }

            last_updated = current_updated;
        }

        iteration += 1;
        sleep(Duration::from_secs(interval_secs)).await;
    }
}

#[derive(Debug, Clone)]
pub struct WatchCommentsOptions {
    pub issue_ids: Vec<String>,
    pub interval_secs: u64,
    pub refresh_issues_interval_secs: u64,
    pub mine: bool,
    pub team: Option<String>,
    pub state: Option<String>,
    pub assignee: Option<String>,
    pub project: Option<String>,
    pub label: Option<String>,
    pub search: Option<String>,
    pub view: Option<String>,
    pub subscribed: bool,
    pub created_by_me: bool,
    pub include_archived: bool,
    pub sources: Vec<String>,
    pub comment_filters: Vec<String>,
    pub replay_existing: bool,
    pub since: String,
    pub state_file: Option<PathBuf>,
    pub comment_limit: i64,
}

#[derive(Debug)]
struct WatchState {
    seen_comment_ids: HashSet<String>,
}

/// Watch issue comments and emit new comment events suitable for piping.
pub async fn watch_comments(opts: WatchCommentsOptions, output: &OutputOptions) -> Result<()> {
    validate_watch_comments_options(&opts)?;

    let interval_secs = opts.interval_secs;
    let refresh_issues_interval_secs = opts.refresh_issues_interval_secs.max(interval_secs);
    let comment_limit = opts.comment_limit.max(1).min(100);
    let client = LinearClient::new()?;
    let mut state = load_watch_state(opts.state_file.as_ref())?;
    let parsed_since = parse_watch_since(&opts.since)?;
    let since_cutoff = if opts.replay_existing {
        parsed_since
    } else {
        parsed_since.or_else(|| Some(Utc::now()))
    };
    let source_filters = normalize_sources(&opts.sources);
    let mut event_filters = output.filters.clone();
    event_filters.extend(parse_filters(&opts.comment_filters)?);
    let mut issue_cache: Vec<Value> = Vec::new();
    let mut last_issue_refresh: Option<Instant> = None;

    if is_quiet() {
        // Keep stdout clean for NDJSON and keep daemon logs quiet unless errors occur.
    } else if !output.is_ndjson() && !output.is_json() {
        eprintln!(
            "Watching comments for changes (Ctrl+C to stop). For automation, use --output ndjson.\n"
        );
    } else {
        eprintln!("Watching comments for changes (Ctrl+C to stop)...");
    }

    loop {
        let mut refreshed_issues = false;
        if should_refresh_issues(
            &opts,
            last_issue_refresh,
            refresh_issues_interval_secs,
            &issue_cache,
        ) {
            issue_cache = fetch_watch_issues(&client, &opts, output, comment_limit).await?;
            last_issue_refresh = Some(Instant::now());
            refreshed_issues = true;
        }

        // Refresh comment data every polling interval. For filtered watches,
        // the expensive issue-set membership query runs only on
        // --refresh-issues-interval, while the cached issue IDs are still
        // re-fetched each loop so new comments are discovered promptly.
        if !refreshed_issues {
            issue_cache =
                refetch_cached_issues_with_comments(&client, &issue_cache, comment_limit).await?;
        }

        let events = collect_comment_events(
            &issue_cache,
            &source_filters,
            since_cutoff,
            &event_filters,
            &mut state,
        );

        for event in events {
            emit_comment_event(event, output)?;
        }

        if let Some(path) = opts.state_file.as_ref() {
            save_watch_state(path, &state)?;
        }

        sleep(Duration::from_secs(interval_secs)).await;
    }
}

fn validate_watch_comments_options(opts: &WatchCommentsOptions) -> Result<()> {
    if opts.interval_secs < 5 {
        anyhow::bail!("--interval must be at least 5 seconds");
    }

    if opts.issue_ids.is_empty() && !has_issue_selector(opts) {
        anyhow::bail!(
            "watch comments requires issue IDs or an issue selector such as --mine, --team, --assignee, --subscribed, --search, or --view"
        );
    }

    Ok(())
}

fn has_issue_selector(opts: &WatchCommentsOptions) -> bool {
    opts.mine
        || opts.team.is_some()
        || opts.state.is_some()
        || opts.assignee.is_some()
        || opts.project.is_some()
        || opts.label.is_some()
        || opts.search.is_some()
        || opts.view.is_some()
        || opts.subscribed
        || opts.created_by_me
}

fn should_refresh_issues(
    opts: &WatchCommentsOptions,
    last_refresh: Option<Instant>,
    refresh_secs: u64,
    issue_cache: &[Value],
) -> bool {
    if !opts.issue_ids.is_empty() {
        return issue_cache.is_empty();
    }
    match last_refresh {
        None => true,
        Some(last) => last.elapsed() >= Duration::from_secs(refresh_secs),
    }
}

async fn fetch_watch_issues(
    client: &LinearClient,
    opts: &WatchCommentsOptions,
    output: &OutputOptions,
    comment_limit: i64,
) -> Result<Vec<Value>> {
    if !opts.issue_ids.is_empty() {
        let mut issues = Vec::new();
        for id in &opts.issue_ids {
            let issue = fetch_issue_with_comments(client, id, comment_limit).await?;
            issues.push(issue);
        }
        return Ok(issues);
    }

    fetch_filtered_issues_with_comments(client, opts, output, comment_limit).await
}

async fn refetch_cached_issues_with_comments(
    client: &LinearClient,
    issue_cache: &[Value],
    comment_limit: i64,
) -> Result<Vec<Value>> {
    let mut issues = Vec::with_capacity(issue_cache.len());
    for issue in issue_cache {
        let id = issue["id"]
            .as_str()
            .or_else(|| issue["identifier"].as_str())
            .ok_or_else(|| anyhow::anyhow!("Cached watch issue is missing id and identifier"))?;
        issues.push(fetch_issue_with_comments(client, id, comment_limit).await?);
    }
    Ok(issues)
}

async fn fetch_issue_with_comments(
    client: &LinearClient,
    id: &str,
    comment_limit: i64,
) -> Result<Value> {
    let query = r#"
        query($id: String!, $commentLimit: Int!) {
            issue(id: $id) {
                id
                identifier
                title
                url
                updatedAt
                team { id key name }
                state { id name type }
                assignee { id name email }
                creator { id name email }
                labels { nodes { id name color } }
                comments(first: $commentLimit) {
                    nodes {
                        id
                        body
                        createdAt
                        updatedAt
                        editedAt
                        url
                        parent { id }
                        user { id name email }
                        externalUser { id name displayName email }
                        externalThread {
                            id
                            type
                            subType
                            name
                            displayName
                            url
                            isConnected
                        }
                        syncedWith { service id }
                    }
                }
            }
        }
    "#;

    let result = client
        .query(
            query,
            Some(json!({ "id": id, "commentLimit": comment_limit })),
        )
        .await?;
    let issue = result["data"]["issue"].clone();
    if issue.is_null() {
        anyhow::bail!("Issue not found: {}", id);
    }
    Ok(issue)
}

async fn fetch_filtered_issues_with_comments(
    client: &LinearClient,
    opts: &WatchCommentsOptions,
    output: &OutputOptions,
    comment_limit: i64,
) -> Result<Vec<Value>> {
    let query = r#"
        query($filter: IssueFilter, $includeArchived: Boolean, $commentLimit: Int!, $first: Int, $after: String, $last: Int, $before: String) {
            issues(
                first: $first,
                after: $after,
                last: $last,
                before: $before,
                includeArchived: $includeArchived,
                filter: $filter
            ) {
                nodes {
                    id
                    identifier
                    title
                    url
                    updatedAt
                    team { id key name }
                    state { id name type }
                    assignee { id name email }
                    creator { id name email }
                    labels { nodes { id name color } }
                    comments(first: $commentLimit) {
                        nodes {
                            id
                            body
                            createdAt
                            updatedAt
                            editedAt
                            url
                            parent { id }
                            user { id name email }
                            externalUser { id name displayName email }
                            externalThread {
                                id
                                type
                                subType
                                name
                                displayName
                                url
                                isConnected
                            }
                            syncedWith { service id }
                        }
                    }
                }
                pageInfo {
                    hasNextPage
                    endCursor
                    hasPreviousPage
                    startCursor
                }
            }
        }
    "#;

    let mut variables = Map::new();
    variables.insert("includeArchived".to_string(), json!(opts.include_archived));
    variables.insert("commentLimit".to_string(), json!(comment_limit));

    let mut filter = if let Some(ref view_name) = opts.view {
        super::views::fetch_view_filter(client, view_name, &output.cache).await?
    } else {
        json!({})
    };

    if let Some(ref team) = opts.team {
        let team_id = resolve_team_id(client, team, &output.cache).await?;
        filter["team"] = json!({ "id": { "eq": team_id } });
    }
    if let Some(ref state) = opts.state {
        filter["state"] = json!({ "name": { "eqIgnoreCase": state } });
    }
    if opts.mine {
        filter["assignee"] = build_user_filter("me");
    } else if let Some(ref assignee) = opts.assignee {
        filter["assignee"] = build_user_filter(assignee);
    }
    if let Some(ref project) = opts.project {
        filter["project"] = json!({ "name": { "eqIgnoreCase": project } });
    }
    if let Some(ref label) = opts.label {
        filter["labels"] = json!({ "name": { "eqIgnoreCase": label } });
    }
    if let Some(ref search) = opts.search {
        filter["searchableContent"] = json!({ "contains": search });
    }
    if opts.subscribed {
        filter["subscribers"] = json!({ "some": { "isMe": { "eq": true } } });
    }
    if opts.created_by_me {
        filter["creator"] = build_user_filter("me");
    }

    if filter.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
        variables.insert("filter".to_string(), filter);
    }

    let pagination = output.pagination.with_default_limit(50);
    paginate_nodes(
        client,
        query,
        variables,
        &["data", "issues", "nodes"],
        &["data", "issues", "pageInfo"],
        &pagination,
        50,
    )
    .await
}

fn build_user_filter(user: &str) -> Value {
    if user.eq_ignore_ascii_case("me") {
        json!({ "isMe": { "eq": true } })
    } else if is_uuid(user) {
        json!({ "id": { "eq": user } })
    } else if user.contains('@') {
        json!({ "email": { "eqIgnoreCase": user } })
    } else {
        json!({ "name": { "eqIgnoreCase": user } })
    }
}

fn collect_comment_events(
    issues: &[Value],
    source_filters: &[String],
    since_cutoff: Option<DateTime<Utc>>,
    event_filters: &[crate::output::FilterExpr],
    state: &mut WatchState,
) -> Vec<Value> {
    let mut events = Vec::new();

    for issue in issues {
        let comments = issue["comments"]["nodes"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let thread_by_comment = external_threads_by_comment_id(&comments);

        // Linear returns newest comments first; emit older events first for consumers.
        for comment in comments.iter().rev() {
            let Some(comment_id) = comment["id"].as_str() else {
                continue;
            };

            if state.seen_comment_ids.contains(comment_id) {
                continue;
            }

            if let Some(cutoff) = since_cutoff {
                if !comment_is_newer_than(comment, cutoff) {
                    state.seen_comment_ids.insert(comment_id.to_string());
                    continue;
                }
            }

            let event = build_comment_event(issue, comment, &thread_by_comment);
            if !matches_source_filters(&event, source_filters) {
                state.seen_comment_ids.insert(comment_id.to_string());
                continue;
            }
            if !matches_filters(&event, event_filters) {
                state.seen_comment_ids.insert(comment_id.to_string());
                continue;
            }

            state.seen_comment_ids.insert(comment_id.to_string());
            events.push(event);
        }
    }

    events
}

fn build_comment_event(
    issue: &Value,
    comment: &Value,
    thread_by_comment: &HashMap<String, Value>,
) -> Value {
    let comment_id = comment["id"].as_str().unwrap_or_default();
    let parent_id = comment["parent"]["id"].as_str();
    let external_thread = comment
        .get("externalThread")
        .filter(|v| !v.is_null())
        .cloned()
        .or_else(|| parent_id.and_then(|id| thread_by_comment.get(id).cloned()));
    let synced_services = synced_services(comment);
    let source_kind = if synced_services.is_empty() {
        external_thread
            .as_ref()
            .and_then(|thread| {
                thread["subType"]
                    .as_str()
                    .or_else(|| thread["name"].as_str())
            })
            .unwrap_or("linear")
            .to_string()
    } else {
        synced_services.join(",")
    };

    json!({
        "schemaVersion": 1,
        "event": "comment.created",
        "emittedAt": Utc::now().to_rfc3339(),
        "source": {
            "kind": source_kind,
            "syncedServices": synced_services,
            "externalThread": external_thread,
        },
        "issue": {
            "id": issue["id"].clone(),
            "identifier": issue["identifier"].clone(),
            "title": issue["title"].clone(),
            "url": issue["url"].clone(),
            "updatedAt": issue["updatedAt"].clone(),
            "team": issue["team"].clone(),
            "state": issue["state"].clone(),
            "assignee": issue["assignee"].clone(),
            "creator": issue["creator"].clone(),
            "labels": issue["labels"]["nodes"].clone(),
        },
        "comment": {
            "id": comment_id,
            "body": comment["body"].clone(),
            "createdAt": comment["createdAt"].clone(),
            "updatedAt": comment["updatedAt"].clone(),
            "editedAt": comment["editedAt"].clone(),
            "url": comment["url"].clone(),
            "parentId": parent_id,
            "author": comment_author(comment),
            "syncedWith": comment["syncedWith"].clone(),
        }
    })
}

fn comment_author(comment: &Value) -> Value {
    if !comment["user"].is_null() {
        json!({
            "kind": "linearUser",
            "id": comment["user"]["id"].clone(),
            "name": comment["user"]["name"].clone(),
            "email": comment["user"]["email"].clone(),
        })
    } else if !comment["externalUser"].is_null() {
        json!({
            "kind": "externalUser",
            "id": comment["externalUser"]["id"].clone(),
            "name": comment["externalUser"]["name"].clone(),
            "displayName": comment["externalUser"]["displayName"].clone(),
            "email": comment["externalUser"]["email"].clone(),
        })
    } else {
        Value::Null
    }
}

fn external_threads_by_comment_id(comments: &[Value]) -> HashMap<String, Value> {
    comments
        .iter()
        .filter_map(|comment| {
            let id = comment["id"].as_str()?;
            let thread = comment.get("externalThread")?;
            if thread.is_null() {
                None
            } else {
                Some((id.to_string(), thread.clone()))
            }
        })
        .collect()
}

fn synced_services(comment: &Value) -> Vec<String> {
    let mut services = Vec::new();
    if let Some(items) = comment["syncedWith"].as_array() {
        for item in items {
            if let Some(service) = item["service"].as_str() {
                if !services.iter().any(|s| s == service) {
                    services.push(service.to_string());
                }
            }
        }
    }
    services
}

fn normalize_sources(sources: &[String]) -> Vec<String> {
    sources
        .iter()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

fn matches_source_filters(event: &Value, source_filters: &[String]) -> bool {
    if source_filters.is_empty() {
        return true;
    }
    let mut sources: Vec<String> = event["source"]["syncedServices"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|service| service.as_str().map(|s| s.to_string()))
        .collect();

    for path in [
        &["source", "externalThread", "subType"][..],
        &["source", "externalThread", "name"][..],
    ] {
        if let Some(value) = crate::json_path::get_path(event, path).and_then(|v| v.as_str()) {
            sources.push(value.to_string());
        }
    }

    sources.iter().any(|source| {
        source_filters
            .iter()
            .any(|wanted| source.eq_ignore_ascii_case(wanted))
    })
}

fn comment_is_newer_than(comment: &Value, cutoff: DateTime<Utc>) -> bool {
    comment["createdAt"]
        .as_str()
        .and_then(|created| DateTime::parse_from_rfc3339(created).ok())
        .map(|created| created.with_timezone(&Utc) >= cutoff)
        .unwrap_or(false)
}

fn parse_watch_since(input: &str) -> Result<Option<DateTime<Utc>>> {
    let trimmed = input.trim();
    if trimmed.eq_ignore_ascii_case("now") || trimmed.is_empty() {
        return Ok(None);
    }

    if let Some(relative) = trimmed.strip_prefix('-') {
        let (amount, unit) = relative.split_at(relative.len().saturating_sub(1));
        let amount: i64 = amount.parse()?;
        let duration = match unit {
            "s" => ChronoDuration::seconds(amount),
            "m" => ChronoDuration::minutes(amount),
            "h" => ChronoDuration::hours(amount),
            "d" => ChronoDuration::days(amount),
            _ => anyhow::bail!(
                "Invalid --since '{}'. Use now, -1h, -24h, or RFC3339.",
                input
            ),
        };
        return Ok(Some(Utc::now() - duration));
    }

    let parsed = DateTime::parse_from_rfc3339(trimmed).map_err(|_| {
        anyhow::anyhow!(
            "Invalid --since '{}'. Use now, -1h, -24h, or RFC3339.",
            input
        )
    })?;
    Ok(Some(parsed.with_timezone(&Utc)))
}

fn emit_comment_event(event: Value, output: &OutputOptions) -> Result<()> {
    if output.is_json() || output.has_template() {
        return print_json_owned(event, output);
    }

    println!(
        "[{}] {} comment from {}: {}",
        Utc::now().format("%H:%M:%S"),
        event["issue"]["identifier"].as_str().unwrap_or("-"),
        event["comment"]["author"]["name"]
            .as_str()
            .or_else(|| event["comment"]["author"]["displayName"].as_str())
            .unwrap_or("unknown"),
        event["comment"]["body"]
            .as_str()
            .unwrap_or("")
            .replace('\n', " ")
    );
    Ok(())
}

fn load_watch_state(path: Option<&PathBuf>) -> Result<WatchState> {
    let Some(path) = path else {
        return Ok(WatchState {
            seen_comment_ids: HashSet::new(),
        });
    };
    if !path.exists() {
        return Ok(WatchState {
            seen_comment_ids: HashSet::new(),
        });
    }
    let text = std::fs::read_to_string(path)?;
    let value: Value = serde_json::from_str(&text)?;
    let seen_comment_ids = value["seenCommentIds"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();
    Ok(WatchState { seen_comment_ids })
}

fn save_watch_state(path: &PathBuf, state: &WatchState) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut seen: Vec<&String> = state.seen_comment_ids.iter().collect();
    seen.sort();
    let value = json!({
        "schemaVersion": 1,
        "seenCommentIds": seen,
        "updatedAt": Utc::now().to_rfc3339(),
    });
    let tmp_path = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("json")
    ));
    std::fs::write(&tmp_path, serde_json::to_vec_pretty(&value)?)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}
