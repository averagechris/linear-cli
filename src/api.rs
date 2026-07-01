use anyhow::{Context, Result};
use futures::StreamExt;
use reqwest::header::HeaderMap;
use reqwest::redirect::Policy;
use reqwest::{Client, StatusCode, Url};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::cache::{Cache, CacheOptions, CacheType};
use crate::config;
use crate::error::{CliError, ErrorKind};
use crate::pagination::{paginate_nodes, PaginationOptions};
use crate::retry::{with_retry, RetryConfig};
use crate::text::is_uuid;
use std::sync::OnceLock;

const LINEAR_API_URL: &str = "https://api.linear.app/graphql";
const LINEAR_UPLOADS_HOST: &str = "uploads.linear.app";

/// Resolve the GraphQL endpoint for this invocation.
///
/// A custom endpoint (`--api-url`, for tests/mocks) is only honored together
/// with an explicit `--api-key`, so keyring credentials can never be sent to
/// a non-default endpoint. `explicit_key` marks clients constructed with a
/// caller-provided key (e.g. auth validation flows).
fn graphql_endpoint(explicit_key: bool) -> Result<String> {
    match config::api_url_override() {
        Some(url) => {
            if !explicit_key && !config::api_key_override_present() {
                anyhow::bail!(
                    "--api-url requires an explicit --api-key; keyring credentials are never sent to a custom endpoint"
                );
            }
            Ok(url)
        }
        None => Ok(LINEAR_API_URL.to_string()),
    }
}

fn sanitize_remote_error_body(body: &str) -> Option<String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return None;
    }

    let first_line = trimmed.lines().next().unwrap_or_default().trim();
    let truncated: String = first_line.chars().take(160).collect();
    Some(if first_line.chars().count() > 160 {
        format!("{}…", truncated)
    } else {
        truncated
    })
}

/// Configuration for generic ID resolution
struct ResolverConfig<'a> {
    cache_type: CacheType,
    filtered_query: &'a str,
    filtered_var_name: &'a str,
    filtered_nodes_path: &'a [&'a str],
    paginated_query: &'a str,
    paginated_nodes_path: &'a [&'a str],
    paginated_page_info_path: &'a [&'a str],
    not_found_msg: &'a str,
}

/// Generic ID resolver that handles cache, filtered query, and paginated fallback
async fn resolve_id<F>(
    client: &LinearClient,
    input: &str,
    cache_opts: &CacheOptions,
    config: &ResolverConfig<'_>,
    finder: F,
) -> Result<String>
where
    F: Fn(&[Value], &str) -> Option<String>,
{
    // Check cache first
    if !cache_opts.no_cache {
        let cache = Cache::with_ttl(cache_opts.effective_ttl_seconds())?;
        if let Some(cached) = cache
            .get(config.cache_type)
            .and_then(|data| data.as_array().cloned())
        {
            if let Some(id) = finder(&cached, input) {
                return Ok(id);
            }
        }
    }

    // Try filtered query first (fast path)
    let result = client
        .query(
            config.filtered_query,
            Some(json!({ config.filtered_var_name: input })),
        )
        .await?;
    let empty = vec![];
    let nodes = get_nested_array(&result, config.filtered_nodes_path).unwrap_or(&empty);

    if let Some(id) = finder(nodes, input) {
        return Ok(id);
    }

    // Fallback: paginate through all items
    let pagination = PaginationOptions {
        all: true,
        page_size: Some(250),
        ..Default::default()
    };
    let all_items = paginate_nodes(
        client,
        config.paginated_query,
        serde_json::Map::new(),
        config.paginated_nodes_path,
        config.paginated_page_info_path,
        &pagination,
        250,
    )
    .await?;

    if !cache_opts.no_cache {
        let cache = Cache::with_ttl(cache_opts.effective_ttl_seconds())?;
        let _ = cache.set(config.cache_type, json!(all_items));
    }

    if let Some(id) = finder(&all_items, input) {
        return Ok(id);
    }

    anyhow::bail!("{}", config.not_found_msg)
}

/// Helper to get nested array from JSON value
fn get_nested_array<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Vec<Value>> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_array()
}

/// Build a CliError from HTTP status code and headers
fn http_error(status: StatusCode, headers: &HeaderMap, context: &str) -> CliError {
    let retry_after = headers
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    let request_id = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_string());
    let details = json!({
        "status": status.as_u16(),
        "reason": status.canonical_reason().unwrap_or("Unknown error"),
        "request_id": request_id,
    });

    let err = match status.as_u16() {
        401 => CliError::auth("Authentication failed - check your API key"),
        403 => CliError::auth(format!("Access denied - {}", context)),
        404 => CliError::not_found(format!("{} not found", context)),
        429 => CliError::rate_limited("Rate limit exceeded").with_retry_after(retry_after),
        _ => CliError::general(format!(
            "HTTP {} {}",
            status.as_u16(),
            details["reason"].as_str().unwrap_or("Unknown error")
        )),
    };
    err.with_details(details)
}

/// Refine a transport-level HTTP error using its GraphQL payload.
///
/// Linear can return application errors (notably RATELIMITED) as a non-2xx
/// status carrying a GraphQL `errors` array, so the transport status alone
/// under-classifies them. When the payload resolves to a more specific kind
/// than the status, prefer it so the exit code stays precise (e.g. 4 for a
/// rate limit behind HTTP 400). The status-derived message is kept.
fn refine_http_error(http_err: CliError, body: &Value) -> CliError {
    let Some(errors) = body.get("errors") else {
        return http_err;
    };
    let payload = CliError::from_graphql_errors(errors);
    if payload.kind == ErrorKind::General {
        return http_err;
    }
    CliError::new(payload.kind, http_err.message.clone())
        .with_retry_after(http_err.retry_after.or(payload.retry_after))
}

pub fn parse_linear_upload_url(raw: &str) -> Result<Url> {
    let url = Url::parse(raw).context("Failed to parse upload URL")?;

    if url.scheme() != "https" {
        anyhow::bail!("Invalid upload URL: expected https");
    }
    if url.host_str() != Some(LINEAR_UPLOADS_HOST) {
        anyhow::bail!(
            "Invalid upload URL: expected exact host '{}'",
            LINEAR_UPLOADS_HOST
        );
    }
    if !url.username().is_empty() || url.password().is_some() {
        anyhow::bail!("Invalid upload URL: credentials are not allowed");
    }
    if let Some(port) = url.port() {
        if port != 443 {
            anyhow::bail!("Invalid upload URL: non-default ports are not allowed");
        }
    }

    Ok(url)
}

fn upload_redirect_policy() -> Policy {
    Policy::custom(|attempt| {
        let url = attempt.url();
        let is_allowed = url.scheme() == "https"
            && url.host_str() == Some(LINEAR_UPLOADS_HOST)
            && url.username().is_empty()
            && url.password().is_none()
            && url.port_or_known_default() == Some(443);

        if is_allowed {
            attempt.follow()
        } else {
            attempt.stop()
        }
    })
}

/// Resolves a team key (like "SCW") or name to a team UUID.
/// If the input is already a UUID (36 characters with dashes), returns it as-is.
pub async fn resolve_team_id(
    client: &LinearClient,
    team: &str,
    cache_opts: &CacheOptions,
) -> Result<String> {
    if is_uuid(team) {
        return Ok(team.to_string());
    }

    let config = ResolverConfig {
        cache_type: CacheType::Teams,
        filtered_query: r#"
            query($team: String!) {
                teams(first: 50, filter: { or: [{ key: { eqIgnoreCase: $team } }, { name: { eqIgnoreCase: $team } }] }) {
                    nodes { id key name }
                }
            }
        "#,
        filtered_var_name: "team",
        filtered_nodes_path: &["data", "teams", "nodes"],
        paginated_query: r#"
            query($first: Int, $after: String) {
                teams(first: $first, after: $after) {
                    nodes { id key name }
                    pageInfo { hasNextPage endCursor }
                }
            }
        "#,
        paginated_nodes_path: &["data", "teams", "nodes"],
        paginated_page_info_path: &["data", "teams", "pageInfo"],
        not_found_msg: &format!(
            "Team not found: {}. Use linear t list to see available teams.",
            team
        ),
    };

    resolve_id(client, team, cache_opts, &config, find_team_id).await
}

/// Resolve a user identifier to a UUID.
/// Handles "me", UUIDs, names, and emails.
pub async fn resolve_user_id(
    client: &LinearClient,
    user: &str,
    cache_opts: &CacheOptions,
) -> Result<String> {
    if user.eq_ignore_ascii_case("me") {
        let query = r#"query { viewer { id } }"#;
        let result = client.query(query, None).await?;
        let user_id = result["data"]["viewer"]["id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Could not fetch current user ID"))?;
        return Ok(user_id.to_string());
    }

    if is_uuid(user) {
        return Ok(user.to_string());
    }

    let config = ResolverConfig {
        cache_type: CacheType::Users,
        filtered_query: r#"
            query($user: String!) {
                users(first: 50, filter: { or: [{ name: { eqIgnoreCase: $user } }, { email: { eqIgnoreCase: $user } }] }) {
                    nodes { id name email }
                }
            }
        "#,
        filtered_var_name: "user",
        filtered_nodes_path: &["data", "users", "nodes"],
        paginated_query: r#"
            query($first: Int, $after: String) {
                users(first: $first, after: $after) {
                    nodes { id name email }
                    pageInfo { hasNextPage endCursor }
                }
            }
        "#,
        paginated_nodes_path: &["data", "users", "nodes"],
        paginated_page_info_path: &["data", "users", "pageInfo"],
        not_found_msg: &format!("User not found: {}", user),
    };

    resolve_id(client, user, cache_opts, &config, find_user_id).await
}

/// Resolve a label name to a UUID.
pub async fn resolve_label_id(
    client: &LinearClient,
    label: &str,
    cache_opts: &CacheOptions,
) -> Result<String> {
    if is_uuid(label) {
        return Ok(label.to_string());
    }

    let config = ResolverConfig {
        cache_type: CacheType::Labels,
        filtered_query: r#"
            query($label: String!) {
                issueLabels(first: 50, filter: { name: { eqIgnoreCase: $label } }) {
                    nodes { id name }
                }
            }
        "#,
        filtered_var_name: "label",
        filtered_nodes_path: &["data", "issueLabels", "nodes"],
        paginated_query: r#"
            query($first: Int, $after: String) {
                issueLabels(first: $first, after: $after) {
                    nodes { id name }
                    pageInfo { hasNextPage endCursor }
                }
            }
        "#,
        paginated_nodes_path: &["data", "issueLabels", "nodes"],
        paginated_page_info_path: &["data", "issueLabels", "pageInfo"],
        not_found_msg: &format!("Label not found: {}", label),
    };

    resolve_id(client, label, cache_opts, &config, find_label_id).await
}

/// Resolve a project name or slug to a UUID.
/// Falls back to paginated lookup if the filtered query fails (some team/project
/// combinations cause the Linear API to reject name-based lookups).
pub async fn resolve_project_id(
    client: &LinearClient,
    project: &str,
    cache_opts: &CacheOptions,
) -> Result<String> {
    if is_uuid(project) {
        return Ok(project.to_string());
    }

    // Check cache first
    if !cache_opts.no_cache {
        let cache = Cache::with_ttl(cache_opts.effective_ttl_seconds())?;
        if let Some(cached) = cache
            .get(CacheType::Projects)
            .and_then(|data| data.as_array().cloned())
        {
            if let Some(id) = find_project_id(&cached, project) {
                return Ok(id);
            }
        }
    }

    // Try filtered query first (fast path) -- tolerate errors and fall through
    let filtered_result = client
        .query(
            r#"
            query($project: String!) {
                projects(first: 50, filter: { name: { eqIgnoreCase: $project } }) {
                    nodes { id name slugId }
                }
            }
            "#,
            Some(json!({ "project": project })),
        )
        .await;

    if let Ok(result) = filtered_result {
        let empty = vec![];
        let nodes = get_nested_array(&result, &["data", "projects", "nodes"]).unwrap_or(&empty);
        if let Some(id) = find_project_id(nodes, project) {
            return Ok(id);
        }
    }

    // Fallback: paginate through all projects
    let pagination = PaginationOptions {
        all: true,
        page_size: Some(250),
        ..Default::default()
    };
    let all_items = paginate_nodes(
        client,
        r#"
        query($first: Int, $after: String) {
            projects(first: $first, after: $after) {
                nodes { id name slugId }
                pageInfo { hasNextPage endCursor }
            }
        }
        "#,
        serde_json::Map::new(),
        &["data", "projects", "nodes"],
        &["data", "projects", "pageInfo"],
        &pagination,
        250,
    )
    .await?;

    if !cache_opts.no_cache {
        let cache = Cache::with_ttl(cache_opts.effective_ttl_seconds())?;
        let _ = cache.set(CacheType::Projects, json!(all_items));
    }

    if let Some(id) = find_project_id(&all_items, project) {
        return Ok(id);
    }

    anyhow::bail!(
        "Could not resolve project \"{}\". \
         Tip: try using the project's slugId (UUID) instead:\n  \
         linear issues update <ID> --project \"<slugId>\"\n\
         You can find project slugIds via: linear projects list",
        project
    )
}

/// Resolve a state name to a UUID for a given team.
/// States are team-scoped in Linear, so the team_id must be provided.
pub async fn resolve_state_id(client: &LinearClient, team_id: &str, state: &str) -> Result<String> {
    if is_uuid(state) {
        return Ok(state.to_string());
    }

    let query = r#"
        query($teamId: String!) {
            team(id: $teamId) {
                states {
                    nodes {
                        id
                        name
                    }
                }
            }
        }
    "#;

    let result = client
        .query(query, Some(json!({ "teamId": team_id })))
        .await?;
    let empty = vec![];
    let states = result["data"]["team"]["states"]["nodes"]
        .as_array()
        .unwrap_or(&empty);

    for s in states {
        let name = s["name"].as_str().unwrap_or("");
        if name.eq_ignore_ascii_case(state) {
            if let Some(id) = s["id"].as_str() {
                return Ok(id.to_string());
            }
        }
    }

    anyhow::bail!("State '{}' not found for team", state)
}

/// Resolve a team-scoped state name to a UUID using the statuses cache.
///
/// The cache key is the Linear team UUID, so no organization-specific status IDs
/// are hardcoded or shared across teams/workspaces. Entries honor the standard
/// cache TTL and can be bypassed with `--no-cache`.
pub async fn resolve_state_id_cached(
    client: &LinearClient,
    team_id: &str,
    state: &str,
    cache_opts: &CacheOptions,
) -> Result<String> {
    if is_uuid(state) {
        return Ok(state.to_string());
    }

    if !cache_opts.no_cache {
        let cache = Cache::with_ttl(cache_opts.effective_ttl_seconds())?;
        if let Some(cached) = cache.get_keyed(CacheType::Statuses, team_id) {
            if let Some(id) = find_state_id_in_cache(&cached, state) {
                return Ok(id);
            }
        }
    }

    let cached = fetch_team_statuses_for_cache(client, team_id).await?;
    if !cache_opts.no_cache {
        let cache = Cache::with_ttl(cache_opts.effective_ttl_seconds())?;
        let _ = cache.set_keyed(CacheType::Statuses, team_id, cached.clone());
    }

    if let Some(id) = find_state_id_in_cache(&cached, state) {
        return Ok(id);
    }

    anyhow::bail!("State '{}' not found for team", state)
}

pub fn clear_cached_statuses_for_team(team_id: &str, cache_opts: &CacheOptions) {
    if cache_opts.no_cache {
        return;
    }
    if let Ok(cache) = Cache::with_ttl(cache_opts.effective_ttl_seconds()) {
        let _ = cache.clear_keyed(CacheType::Statuses, team_id);
    }
}

/// Fetch all workflow states for a team in the canonical statuses cache shape:
/// `{ "team_name": ..., "states": [...] }`, keyed by team UUID by callers.
pub async fn fetch_team_statuses_for_cache(client: &LinearClient, team_id: &str) -> Result<Value> {
    let query = r#"
        query($teamId: String!, $first: Int, $after: String) {
            team(id: $teamId) {
                name
                states(first: $first, after: $after) {
                    nodes {
                        id
                        name
                        type
                        color
                        position
                        description
                    }
                    pageInfo { hasNextPage endCursor }
                }
            }
        }
    "#;

    let mut team_name = String::new();
    let mut states: Vec<Value> = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let mut vars = json!({ "teamId": team_id, "first": 250 });
        if let Some(cursor) = &after {
            vars["after"] = json!(cursor);
        }
        let result = client.query(query, Some(vars)).await?;
        let team = &result["data"]["team"];
        if team.is_null() {
            anyhow::bail!("Team not found while resolving statuses")
        }
        if let Some(name) = team["name"].as_str() {
            team_name = name.to_string();
        }
        if let Some(nodes) = team["states"]["nodes"].as_array() {
            states.extend(nodes.iter().cloned());
        }
        let page_info = &team["states"]["pageInfo"];
        if page_info["hasNextPage"].as_bool() != Some(true) {
            break;
        }
        match page_info["endCursor"].as_str() {
            Some(cursor) => after = Some(cursor.to_string()),
            None => break,
        }
    }

    Ok(json!({
        "team_name": team_name,
        "states": states,
    }))
}

fn find_state_id_in_cache(cached: &Value, state: &str) -> Option<String> {
    let states = cached.get("states")?.as_array()?;
    states.iter().find_map(|s| {
        let name = s["name"].as_str().unwrap_or("");
        if name.eq_ignore_ascii_case(state) {
            s["id"].as_str().map(ToString::to_string)
        } else {
            None
        }
    })
}

fn find_team_id(teams: &[Value], team: &str) -> Option<String> {
    if let Some(team_data) = teams
        .iter()
        .find(|t| t["key"].as_str().map(|k| k.eq_ignore_ascii_case(team)) == Some(true))
    {
        if let Some(id) = team_data["id"].as_str() {
            return Some(id.to_string());
        }
    }

    if let Some(team_data) = teams
        .iter()
        .find(|t| t["name"].as_str().map(|n| n.eq_ignore_ascii_case(team)) == Some(true))
    {
        if let Some(id) = team_data["id"].as_str() {
            return Some(id.to_string());
        }
    }

    None
}

fn find_user_id(users: &[Value], user: &str) -> Option<String> {
    for u in users {
        let name = u["name"].as_str().unwrap_or("");
        let email = u["email"].as_str().unwrap_or("");
        if name.eq_ignore_ascii_case(user) || email.eq_ignore_ascii_case(user) {
            if let Some(id) = u["id"].as_str() {
                return Some(id.to_string());
            }
        }
    }
    None
}

fn find_label_id(labels: &[Value], label: &str) -> Option<String> {
    for l in labels {
        let name = l["name"].as_str().unwrap_or("");
        if name.eq_ignore_ascii_case(label) {
            if let Some(id) = l["id"].as_str() {
                return Some(id.to_string());
            }
        }
    }
    None
}

fn find_project_id(projects: &[Value], project: &str) -> Option<String> {
    // Match by name (case-insensitive)
    for p in projects {
        let name = p["name"].as_str().unwrap_or("");
        if name.eq_ignore_ascii_case(project) {
            if let Some(id) = p["id"].as_str() {
                return Some(id.to_string());
            }
        }
    }
    // Match by slugId
    for p in projects {
        let slug = p["slugId"].as_str().unwrap_or("");
        if slug.eq_ignore_ascii_case(project) {
            if let Some(id) = p["id"].as_str() {
                return Some(id.to_string());
            }
        }
    }
    None
}
/// Resolve a custom view name to a UUID.
pub async fn resolve_view_id(
    client: &LinearClient,
    view: &str,
    cache_opts: &CacheOptions,
) -> Result<String> {
    if is_uuid(view) {
        return Ok(view.to_string());
    }

    let config = ResolverConfig {
        cache_type: CacheType::Views,
        filtered_query: r#"
            query($view: String!) {
                customViews(first: 50, filter: { name: { eqIgnoreCase: $view } }) {
                    nodes { id name }
                }
            }
        "#,
        filtered_var_name: "view",
        filtered_nodes_path: &["data", "customViews", "nodes"],
        paginated_query: r#"
            query($first: Int, $after: String) {
                customViews(first: $first, after: $after) {
                    nodes { id name }
                    pageInfo { hasNextPage endCursor }
                }
            }
        "#,
        paginated_nodes_path: &["data", "customViews", "nodes"],
        paginated_page_info_path: &["data", "customViews", "pageInfo"],
        not_found_msg: &format!(
            "Custom view not found: {}. Use linear views list to see available views.",
            view
        ),
    };

    resolve_id(client, view, cache_opts, &config, find_view_id).await
}

fn find_view_id(views: &[Value], view: &str) -> Option<String> {
    for v in views {
        let name = v["name"].as_str().unwrap_or("");
        if name.eq_ignore_ascii_case(view) {
            if let Some(id) = v["id"].as_str() {
                return Some(id.to_string());
            }
        }
    }
    None
}

/// Authentication state for the API client
#[derive(Clone, Debug)]
pub enum AuthState {
    /// Personal API key (sent as-is in Authorization header)
    ApiKey(String),
    /// OAuth tokens (sent as "Bearer {token}", supports auto-refresh)
    OAuth {
        access_token: String,
        refresh_token: Option<String>,
        client_id: String,
        expires_at: Option<i64>,
        profile: String,
    },
}

impl AuthState {
    /// Get the Authorization header value
    pub fn auth_header(&self) -> String {
        match self {
            AuthState::ApiKey(key) => key.clone(),
            AuthState::OAuth { access_token, .. } => format!("Bearer {}", access_token),
        }
    }

    /// Check if the current auth needs refreshing
    pub fn needs_refresh(&self) -> bool {
        match self {
            AuthState::ApiKey(_) => false,
            AuthState::OAuth {
                expires_at,
                refresh_token,
                ..
            } => {
                if refresh_token.is_none() {
                    return false;
                }
                match expires_at {
                    Some(exp) => {
                        let buffer = 300; // 5 minutes
                        chrono::Utc::now().timestamp() >= (*exp - buffer)
                    }
                    None => false,
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct LinearClient {
    client: Client,
    auth: Arc<RwLock<AuthState>>,
    retry: RetryConfig,
    endpoint: String,
}

impl LinearClient {
    pub fn new() -> Result<Self> {
        let retry = default_retry_config();
        let auth = Self::resolve_auth()?;
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .user_agent(format!("linear-cli/{}", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            client,
            auth: Arc::new(RwLock::new(auth)),
            retry,
            endpoint: graphql_endpoint(false)?,
        })
    }

    pub fn new_with_retry(retry_count: u32) -> Result<Self> {
        let auth = Self::resolve_auth()?;
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .user_agent(format!("linear-cli/{}", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            client,
            auth: Arc::new(RwLock::new(auth)),
            retry: RetryConfig::new(retry_count),
            endpoint: graphql_endpoint(false)?,
        })
    }

    pub fn with_api_key(api_key: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .user_agent(format!("linear-cli/{}", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            client,
            auth: Arc::new(RwLock::new(AuthState::ApiKey(api_key))),
            retry: default_retry_config(),
            endpoint: graphql_endpoint(true)?,
        })
    }

    pub async fn query(&self, query: &str, variables: Option<Value>) -> Result<Value> {
        with_retry(&self.retry, || {
            let vars = variables.clone();
            async move { self.query_once(query, vars).await }
        })
        .await
    }

    async fn query_once(&self, query: &str, variables: Option<Value>) -> Result<Value> {
        let auth_header = self.ensure_fresh_auth().await?;

        let body = match variables {
            Some(vars) => json!({ "query": query, "variables": vars }),
            None => json!({ "query": query }),
        };

        let response = self
            .client
            .post(&self.endpoint)
            .header("Content-Type", "application/json")
            .header("Authorization", &auth_header)
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        let headers = response.headers().clone();

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            let details = if let Ok(body) = serde_json::from_str::<Value>(&body) {
                body
            } else {
                json!({ "body": body.clone() })
            };
            let mut err = refine_http_error(http_error(status, &headers, "resource"), &details);
            if let Some(summary) = sanitize_remote_error_body(&body) {
                err = err.with_details(json!({ "summary": summary, "details": details }));
            } else if !body.is_empty() {
                err = err.with_details(details);
            }
            return Err(err.into());
        }

        let result: Value = response.json().await?;

        if let Some(errors) = result.get("errors") {
            return Err(CliError::from_graphql_errors(errors).into());
        }

        Ok(result)
    }

    pub async fn mutate(&self, mutation: &str, variables: Option<Value>) -> Result<Value> {
        // Mutations must not be retried to avoid duplicate side effects
        self.query_once(mutation, variables).await
    }

    /// Stream response bytes directly to a writer (for large downloads)
    pub async fn fetch_to_writer(
        &self,
        url: &str,
        writer: &mut impl std::io::Write,
    ) -> Result<u64> {
        let url = parse_linear_upload_url(url)?;
        let auth_header = self.ensure_fresh_auth().await?;
        let upload_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(upload_redirect_policy())
            .build()
            .context("Failed to build upload client")?;

        let response = upload_client
            .get(url)
            .header("Authorization", &auth_header)
            .send()
            .await
            .context("Failed to connect to Linear uploads")?;

        let status = response.status();
        let headers = response.headers().clone();
        if !status.is_success() {
            return Err(http_error(status, &headers, "upload").into());
        }

        let mut total: u64 = 0;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Failed to read response chunk")?;
            writer.write_all(&chunk).context("Failed to write chunk")?;
            total += chunk.len() as u64;
        }
        Ok(total)
    }

    /// Resolve authentication from config (checks OAuth explicitly, then API key)
    fn resolve_auth() -> Result<AuthState> {
        let profile = config::current_profile().unwrap_or_else(|_| "default".to_string());
        let oauth_metadata_present = config::get_oauth_metadata(&profile)
            .ok()
            .flatten()
            .is_some();
        let oauth_config = config::get_oauth_config(&profile).ok().flatten();

        Self::resolve_auth_for_profile(
            &profile,
            oauth_metadata_present,
            oauth_config,
            config::get_api_key(),
        )
    }

    fn resolve_auth_for_profile(
        profile: &str,
        oauth_metadata_present: bool,
        oauth_config: Option<config::OAuthConfig>,
        api_key_result: Result<String>,
    ) -> Result<AuthState> {
        if let Some(oauth) = oauth_config {
            if !oauth.access_token.is_empty() {
                // If token has a refresh_token or isn't expired, use OAuth
                let is_expired = oauth
                    .expires_at
                    .map(|exp| chrono::Utc::now().timestamp() >= (exp - 300))
                    .unwrap_or(false);

                if oauth.refresh_token.is_some() || !is_expired {
                    return Ok(AuthState::OAuth {
                        access_token: oauth.access_token,
                        refresh_token: oauth.refresh_token,
                        client_id: oauth.client_id,
                        expires_at: oauth.expires_at,
                        profile: profile.to_string(),
                    });
                }

                return api_key_result.map(AuthState::ApiKey).or_else(|_| {
                    anyhow::bail!(
                        "OAuth token for workspace '{}' is expired and cannot be refreshed. Re-run 'linear-cli auth oauth' or clear stale OAuth metadata with 'linear-cli auth logout --force'.",
                        profile
                    )
                });
            }
        }

        if oauth_metadata_present {
            return api_key_result.map(AuthState::ApiKey).or_else(|_| {
                anyhow::bail!(
                    "OAuth is configured for workspace '{}' but the OAuth token is missing from the keyring. Re-run 'linear-cli auth oauth' or clear stale OAuth metadata with 'linear-cli auth logout --force'.",
                    profile
                )
            });
        }

        Ok(AuthState::ApiKey(api_key_result?))
    }

    /// Ensure auth is fresh (refresh OAuth token if needed)
    async fn ensure_fresh_auth(&self) -> Result<String> {
        {
            let auth = self.auth.read().await;
            if !auth.needs_refresh() {
                return Ok(auth.auth_header());
            }
        }

        // Need to refresh - acquire write lock
        let mut auth = self.auth.write().await;

        // Double-check after acquiring write lock (another task may have refreshed)
        if !auth.needs_refresh() {
            return Ok(auth.auth_header());
        }

        match &*auth {
            AuthState::OAuth {
                refresh_token,
                client_id,
                profile,
                ..
            } => {
                let refresh_token = refresh_token
                    .as_ref()
                    .context("OAuth token expired but no refresh token available")?;

                let new_tokens = crate::oauth::refresh_tokens(client_id, refresh_token).await?;

                // Persist the new tokens
                let scopes = if let Ok(Some(existing)) = config::get_oauth_metadata(profile) {
                    existing.scopes
                } else {
                    vec![]
                };

                let oauth_config = config::OAuthConfig {
                    client_id: client_id.clone(),
                    access_token: new_tokens.access_token.clone(),
                    refresh_token: new_tokens.refresh_token.clone(),
                    expires_at: new_tokens.expires_at,
                    token_type: new_tokens.token_type.clone(),
                    scopes,
                };
                let persist_result = config::save_oauth_config(profile, &oauth_config);

                if let Err(e) = persist_result {
                    eprintln!("Warning: Failed to persist refreshed OAuth tokens: {}", e);
                }

                let new_auth = AuthState::OAuth {
                    access_token: new_tokens.access_token,
                    refresh_token: new_tokens.refresh_token,
                    client_id: client_id.clone(),
                    expires_at: new_tokens.expires_at,
                    profile: profile.clone(),
                };
                let header = new_auth.auth_header();
                *auth = new_auth;
                Ok(header)
            }
            AuthState::ApiKey(_) => Ok(auth.auth_header()),
        }
    }
}

static DEFAULT_RETRY: OnceLock<RetryConfig> = OnceLock::new();

pub fn set_default_retry(retry_count: u32) {
    let config = if retry_count == 0 {
        RetryConfig::no_retry()
    } else {
        RetryConfig::new(retry_count)
    };
    let _ = DEFAULT_RETRY.set(config);
}

fn default_retry_config() -> RetryConfig {
    DEFAULT_RETRY.get().copied().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refine_http_error_prefers_ratelimited_payload_behind_http_400() {
        let http_err = CliError::general("HTTP 400 Bad Request");
        let body = json!({
            "errors": [{
                "message": "Rate limit exceeded",
                "extensions": { "code": "RATELIMITED", "retryAfter": 12 }
            }]
        });
        let refined = refine_http_error(http_err, &body);
        assert_eq!(refined.kind, ErrorKind::RateLimited);
        assert_eq!(refined.code(), 4);
        assert_eq!(refined.retry_after, Some(12));
    }

    #[test]
    fn refine_http_error_keeps_status_kind_when_payload_is_generic() {
        let http_err = CliError::auth("Authentication failed");
        let body = json!({ "errors": [{ "message": "Unauthorized" }] });
        let refined = refine_http_error(http_err, &body);
        assert_eq!(refined.kind, ErrorKind::Auth);
        assert_eq!(refined.code(), 3);
    }

    #[test]
    fn refine_http_error_passes_through_without_errors_array() {
        let http_err = CliError::not_found("resource not found");
        let body = json!({ "body": "<html>502</html>" });
        let refined = refine_http_error(http_err, &body);
        assert_eq!(refined.kind, ErrorKind::NotFound);
    }

    #[test]
    fn find_state_id_in_cache_matches_status_name_case_insensitively() {
        let cached = json!({
            "team_name": "Engineering",
            "states": [
                { "id": "todo-id", "name": "Todo" },
                { "id": "started-id", "name": "In Progress" }
            ]
        });

        assert_eq!(
            find_state_id_in_cache(&cached, "in progress").as_deref(),
            Some("started-id")
        );
    }

    #[test]
    fn find_state_id_in_cache_returns_none_for_missing_status() {
        let cached = json!({
            "team_name": "Engineering",
            "states": [{ "id": "todo-id", "name": "Todo" }]
        });

        assert!(find_state_id_in_cache(&cached, "Done").is_none());
    }

    #[test]
    fn test_auth_state_api_key_header() {
        let state = AuthState::ApiKey("lin_api_key123".to_string());
        assert_eq!(state.auth_header(), "lin_api_key123");
    }

    #[test]
    fn test_auth_state_oauth_header() {
        let state = AuthState::OAuth {
            access_token: "oauth_token_abc".to_string(),
            refresh_token: None,
            client_id: "cid".to_string(),
            expires_at: None,
            profile: "default".to_string(),
        };
        assert_eq!(state.auth_header(), "Bearer oauth_token_abc");
    }

    #[test]
    fn test_auth_state_api_key_no_refresh() {
        let state = AuthState::ApiKey("key".to_string());
        assert!(!state.needs_refresh(), "API key should never need refresh");
    }

    #[test]
    fn test_auth_state_oauth_no_refresh_token() {
        let state = AuthState::OAuth {
            access_token: "tok".to_string(),
            refresh_token: None,
            client_id: "cid".to_string(),
            expires_at: Some(chrono::Utc::now().timestamp() - 100), // expired
            profile: "default".to_string(),
        };
        assert!(
            !state.needs_refresh(),
            "OAuth without refresh token should not need refresh even if expired"
        );
    }

    #[test]
    fn test_auth_state_oauth_needs_refresh_expired() {
        let state = AuthState::OAuth {
            access_token: "tok".to_string(),
            refresh_token: Some("refresh".to_string()),
            client_id: "cid".to_string(),
            expires_at: Some(chrono::Utc::now().timestamp() - 100), // expired
            profile: "default".to_string(),
        };
        assert!(
            state.needs_refresh(),
            "OAuth with expired token and refresh token should need refresh"
        );
    }

    #[test]
    fn test_auth_state_oauth_needs_refresh_within_buffer() {
        let state = AuthState::OAuth {
            access_token: "tok".to_string(),
            refresh_token: Some("refresh".to_string()),
            client_id: "cid".to_string(),
            expires_at: Some(chrono::Utc::now().timestamp() + 200), // within 5min buffer
            profile: "default".to_string(),
        };
        assert!(
            state.needs_refresh(),
            "OAuth expiring within buffer should need refresh"
        );
    }

    #[test]
    fn test_auth_state_oauth_no_refresh_needed_fresh() {
        let state = AuthState::OAuth {
            access_token: "tok".to_string(),
            refresh_token: Some("refresh".to_string()),
            client_id: "cid".to_string(),
            expires_at: Some(chrono::Utc::now().timestamp() + 3600), // 1 hour from now
            profile: "default".to_string(),
        };
        assert!(
            !state.needs_refresh(),
            "OAuth with fresh token should not need refresh"
        );
    }

    #[test]
    fn test_auth_state_oauth_no_expiry_no_refresh() {
        let state = AuthState::OAuth {
            access_token: "tok".to_string(),
            refresh_token: Some("refresh".to_string()),
            client_id: "cid".to_string(),
            expires_at: None, // no expiry (legacy token)
            profile: "default".to_string(),
        };
        assert!(
            !state.needs_refresh(),
            "OAuth without expiry should not need refresh"
        );
    }

    #[test]
    fn test_auth_state_clone() {
        let state = AuthState::OAuth {
            access_token: "tok".to_string(),
            refresh_token: Some("ref".to_string()),
            client_id: "cid".to_string(),
            expires_at: Some(1700000000),
            profile: "test".to_string(),
        };
        let cloned = state.clone();
        assert_eq!(state.auth_header(), cloned.auth_header());
    }

    #[test]
    fn test_auth_state_debug() {
        let state = AuthState::ApiKey("key".to_string());
        let debug = format!("{:?}", state);
        assert!(
            debug.contains("ApiKey"),
            "Debug output should contain variant name"
        );
    }

    #[test]
    fn test_resolve_auth_prefers_usable_oauth() {
        let oauth = config::OAuthConfig {
            client_id: "cid".to_string(),
            access_token: "oauth_token".to_string(),
            refresh_token: Some("refresh".to_string()),
            expires_at: Some(chrono::Utc::now().timestamp() + 3600),
            token_type: "Bearer".to_string(),
            scopes: vec!["read".to_string(), "write".to_string()],
        };

        let state = LinearClient::resolve_auth_for_profile(
            "default",
            true,
            Some(oauth),
            Ok("lin_api_fallback".to_string()),
        )
        .expect("usable OAuth should be selected");

        assert!(matches!(state, AuthState::OAuth { .. }));
    }

    #[test]
    fn test_resolve_auth_reports_missing_oauth_keyring() {
        let err = LinearClient::resolve_auth_for_profile(
            "default",
            true,
            None,
            Err(anyhow::anyhow!("No API key configured")),
        )
        .expect_err("missing OAuth keyring should produce targeted error");

        assert!(
            err.to_string()
                .contains("OAuth token is missing from the keyring"),
            "expected targeted missing-keyring message, got: {err}"
        );
    }

    #[test]
    fn test_resolve_auth_reports_expired_oauth_without_refresh() {
        let oauth = config::OAuthConfig {
            client_id: "cid".to_string(),
            access_token: "oauth_token".to_string(),
            refresh_token: None,
            expires_at: Some(chrono::Utc::now().timestamp() - 60),
            token_type: "Bearer".to_string(),
            scopes: vec!["read".to_string(), "write".to_string()],
        };

        let err = LinearClient::resolve_auth_for_profile(
            "default",
            true,
            Some(oauth),
            Err(anyhow::anyhow!("No API key configured")),
        )
        .expect_err("expired OAuth without refresh token should produce targeted error");

        assert!(
            err.to_string()
                .contains("OAuth token for workspace 'default' is expired"),
            "expected targeted expiry message, got: {err}"
        );
    }
}
