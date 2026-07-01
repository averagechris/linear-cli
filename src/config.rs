use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[cfg(unix)]
use std::io::Write;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub token_type: String,
    #[serde(default)]
    pub scopes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Workspace {
    pub api_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth: Option<OAuthConfig>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Config {
    pub current: Option<String>,
    #[serde(default)]
    pub workspaces: HashMap<String, Workspace>,
    /// Non-secret user defaults and agent/project policy hints.
    #[serde(default, skip_serializing_if = "LinearContextConfig::is_empty")]
    pub context: LinearContextConfig,
    // Legacy field for backward compatibility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct LinearContextConfig {
    #[serde(default, skip_serializing_if = "is_zero")]
    pub version: u32,
    #[serde(default, skip_serializing_if = "ContextDefaults::is_empty")]
    pub defaults: ContextDefaults,
    #[serde(default, skip_serializing_if = "IssueCreateContext::is_empty")]
    pub issue_create: IssueCreateContext,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub label_groups: Vec<LabelGroupPolicy>,
    #[serde(default, skip_serializing_if = "ContextCacheConfig::is_empty")]
    pub cache: ContextCacheConfig,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agent_instructions: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct ContextDefaults {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Explicit labels that are always safe to apply in this context. Do not use
    /// this for inferred/category labels such as a domain taxonomy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
}

impl ContextDefaults {
    pub fn is_empty(&self) -> bool {
        self.team.is_none() && self.status.is_none() && self.labels.is_empty()
    }

    fn merge(&self, override_defaults: &ContextDefaults) -> ContextDefaults {
        ContextDefaults {
            team: override_defaults.team.clone().or_else(|| self.team.clone()),
            status: override_defaults
                .status
                .clone()
                .or_else(|| self.status.clone()),
            labels: if override_defaults.labels.is_empty() {
                self.labels.clone()
            } else {
                override_defaults.labels.clone()
            },
        }
    }
}

/// How agents should populate a field. Kept intentionally small: `infer_or_ask`
/// is the canonical "infer only when clearly implied, otherwise ask" mode.
/// Legacy synonyms from earlier drafts are accepted as aliases on read.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FieldMode {
    Off,
    Default,
    Infer,
    Suggest,
    #[serde(alias = "ask_or_leave_unset")]
    Ask,
    #[default]
    #[serde(alias = "ask_if_ambiguous", alias = "ask_or_infer")]
    InferOrAsk,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct FieldPolicy {
    /// Always serialized so agents see an explicit mode contract.
    #[serde(default)]
    pub mode: FieldMode,
    #[serde(default, skip_serializing_if = "is_false")]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_when: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovery: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guidance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guidance_ref: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct EstimationPolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guidance: Option<String>,
}

impl EstimationPolicy {
    pub fn is_empty(&self) -> bool {
        self.scale.is_none() && self.values.is_empty() && self.guidance.is_none()
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct IssueCreateContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_ambiguity: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub fields: HashMap<String, FieldPolicy>,
    #[serde(default, skip_serializing_if = "EstimationPolicy::is_empty")]
    pub estimation: EstimationPolicy,
}

impl IssueCreateContext {
    pub fn is_empty(&self) -> bool {
        self.on_ambiguity.is_none() && self.fields.is_empty() && self.estimation.is_empty()
    }

    fn merge(&self, override_context: &IssueCreateContext) -> IssueCreateContext {
        let mut fields = self.fields.clone();
        for (key, policy) in &override_context.fields {
            fields.insert(key.clone(), policy.clone());
        }
        IssueCreateContext {
            on_ambiguity: override_context
                .on_ambiguity
                .clone()
                .or_else(|| self.on_ambiguity.clone()),
            fields,
            estimation: if override_context.estimation.is_empty() {
                self.estimation.clone()
            } else {
                override_context.estimation.clone()
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct LabelOption {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct LabelHint {
    pub label: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repos: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct LabelGroupPolicy {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linear_group: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cardinality: Option<String>,
    /// Always serialized so agents see an explicit mode contract.
    #[serde(default)]
    pub mode: FieldMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovery: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ask_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guidance: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<LabelOption>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hints: Vec<LabelHint>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct ContextCacheConfig {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub ttl: HashMap<String, String>,
}

impl ContextCacheConfig {
    pub fn is_empty(&self) -> bool {
        self.ttl.is_empty()
    }
}

impl LinearContextConfig {
    pub fn merge(&self, override_config: &LinearContextConfig) -> LinearContextConfig {
        let mut label_groups = self.label_groups.clone();
        for group in &override_config.label_groups {
            if let Some(existing) = label_groups.iter_mut().find(|g| g.key == group.key) {
                *existing = group.clone();
            } else {
                label_groups.push(group.clone());
            }
        }
        let mut ttl = self.cache.ttl.clone();
        ttl.extend(override_config.cache.ttl.clone());
        LinearContextConfig {
            version: if override_config.version == 0 {
                self.version
            } else {
                override_config.version
            },
            defaults: self.defaults.merge(&override_config.defaults),
            issue_create: self.issue_create.merge(&override_config.issue_create),
            label_groups,
            cache: ContextCacheConfig { ttl },
            agent_instructions: {
                let mut instructions = self.agent_instructions.clone();
                instructions.extend(override_config.agent_instructions.clone());
                instructions
            },
        }
    }

    pub fn is_empty(&self) -> bool {
        self.version == 0
            && self.defaults.is_empty()
            && self.issue_create.is_empty()
            && self.label_groups.is_empty()
            && self.cache.is_empty()
            && self.agent_instructions.is_empty()
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ProjectContextFile {
    #[serde(default, skip_serializing_if = "LinearContextConfig::is_empty")]
    pub context: LinearContextConfig,
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_zero(value: &u32) -> bool {
    *value == 0
}

#[derive(Debug, Clone)]
pub struct ResolvedLinearContext {
    pub user: LinearContextConfig,
    pub project: Option<LinearContextConfig>,
    pub project_file: Option<PathBuf>,
    pub resolved: LinearContextConfig,
}

#[derive(Debug, Clone, Default)]
struct RuntimeOverrides {
    api_key: Option<String>,
    profile: Option<String>,
    api_url: Option<String>,
}

static RUNTIME_OVERRIDES: OnceLock<RuntimeOverrides> = OnceLock::new();

pub fn set_runtime_overrides(
    api_key: Option<String>,
    profile: Option<String>,
    api_url: Option<String>,
) {
    let _ = RUNTIME_OVERRIDES.set(RuntimeOverrides {
        api_key: api_key.filter(|value| !value.trim().is_empty()),
        profile: profile.filter(|value| !value.trim().is_empty()),
        api_url: api_url.filter(|value| !value.trim().is_empty()),
    });
}

fn runtime_overrides() -> &'static RuntimeOverrides {
    RUNTIME_OVERRIDES.get_or_init(RuntimeOverrides::default)
}

fn profile_override() -> Option<String> {
    runtime_overrides().profile.clone()
}

fn api_key_override() -> Option<String> {
    runtime_overrides().api_key.clone()
}

pub fn api_key_override_present() -> bool {
    api_key_override().is_some()
}

/// Custom GraphQL endpoint for this invocation (tests/debugging only).
/// Only honored together with an explicit `--api-key`; keyring credentials
/// are never sent to a non-default endpoint.
pub fn api_url_override() -> Option<String> {
    runtime_overrides().api_url.clone()
}

pub fn profile_override_name() -> Option<String> {
    profile_override()
}

fn config_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .context("Could not find config directory")?
        .join("linear-cli");

    fs::create_dir_all(&config_dir)?;
    Ok(config_dir.join("config.toml"))
}

pub fn load_config() -> Result<Config> {
    let path = config_path()?;
    if path.exists() {
        let content = fs::read_to_string(&path)?;
        let mut config: Config = toml::from_str(&content)?;

        // Migrate legacy api_key to workspaces if needed
        if let Some(legacy_key) = config.api_key.take() {
            if !config.workspaces.contains_key("default") {
                config.workspaces.insert(
                    "default".to_string(),
                    Workspace {
                        api_key: legacy_key,
                        oauth: None,
                    },
                );
                if config.current.is_none() {
                    config.current = Some("default".to_string());
                }
            }
        }

        if migrate_plaintext_secrets(&mut config)? {
            save_config(&config)?;
        }

        Ok(config)
    } else {
        Ok(Config::default())
    }
}

fn config_for_disk(config: &Config) -> Config {
    let mut sanitized = config.clone();
    sanitized.api_key = None;
    for workspace in sanitized.workspaces.values_mut() {
        workspace.api_key.clear();
        if let Some(oauth) = workspace.oauth.clone() {
            workspace.oauth = Some(oauth_metadata_only(&oauth));
        }
    }
    sanitized
}

fn migrate_plaintext_secrets(config: &mut Config) -> Result<bool> {
    let mut changed = false;

    for (profile, workspace) in &mut config.workspaces {
        if !workspace.api_key.trim().is_empty() {
            crate::keyring::set_key(profile, &workspace.api_key)?;
            workspace.api_key.clear();
            changed = true;
        }

        if let Some(oauth) = workspace.oauth.clone() {
            if oauth_config_has_secrets(&oauth) {
                crate::keyring::set_oauth_tokens(profile, &serde_json::to_string(&oauth)?)?;
                workspace.oauth = Some(oauth_metadata_only(&oauth));
                changed = true;
            }
        }
    }

    Ok(changed)
}

pub fn save_config(config: &Config) -> Result<()> {
    let path = config_path()?;
    let content = toml::to_string_pretty(&config_for_disk(config))?;

    // Write to temp file then rename for atomicity
    let dir = path
        .parent()
        .context("Config path has no parent directory")?;
    let temp_path = dir.join(".config.toml.tmp");

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
        file.flush()?;
    }

    #[cfg(not(unix))]
    {
        fs::write(&temp_path, &content)?;
    }

    fs::rename(&temp_path, &path).context("Failed to atomically update config file")?;
    Ok(())
}

pub fn set_api_key(key: &str) -> Result<()> {
    let mut config = load_config()?;
    let profile = profile_override();
    let workspace_name = profile
        .or_else(|| config.current.clone())
        .unwrap_or_else(|| "default".to_string());
    let existing_oauth = config
        .workspaces
        .get(&workspace_name)
        .and_then(|w| w.oauth.clone());
    config.workspaces.insert(
        workspace_name.clone(),
        Workspace {
            api_key: String::new(),
            oauth: existing_oauth,
        },
    );
    if config.current.is_none() {
        config.current = Some(workspace_name.clone());
    }
    crate::keyring::set_key(&workspace_name, key)?;
    save_config(&config)?;
    Ok(())
}

pub fn get_api_key() -> Result<String> {
    if let Some(api_key) = api_key_override() {
        return Ok(api_key);
    }

    let config = load_config()?;
    let profile = profile_override();
    let current = profile.or(config.current.clone()).context(
        "No workspace selected. Run: linear config workspace-add <name> or use --profile for this invocation",
    )?;
    config.workspaces.get(&current).context(format!(
        "Workspace '{}' not found. Run: linear config workspace-add <name>",
        current
    ))?;
    crate::keyring::get_key(&current)?.context(format!(
        "No API key configured for workspace '{}'. Use 'linear auth login' or 'linear config set-key'.",
        current
    ))
}

pub fn config_file_path() -> Result<PathBuf> {
    config_path()
}

/// Returns the current workspace profile name.
///
/// NOTE: The result is cached via OnceLock for the lifetime of the process.
/// If the profile is changed in-process (e.g., via `workspace_switch`), the
/// cached value will be stale. This is acceptable because profile switches
/// during a single CLI invocation are not a supported use case.
pub fn current_profile() -> Result<String> {
    static PROFILE: OnceLock<String> = OnceLock::new();

    if let Some(cached) = PROFILE.get() {
        return Ok(cached.clone());
    }

    let config = load_config()?;
    let profile = profile_override();
    let resolved = profile
        .or(config.current)
        .context("No workspace selected")?;

    // Store for future calls (ignore if another thread beat us)
    let _ = PROFILE.set(resolved.clone());
    Ok(resolved)
}

pub fn set_workspace_key(name: &str, api_key: &str) -> Result<()> {
    let mut config = load_config()?;
    let existing_oauth = config.workspaces.get(name).and_then(|w| w.oauth.clone());
    config.workspaces.insert(
        name.to_string(),
        Workspace {
            api_key: String::new(),
            oauth: existing_oauth,
        },
    );
    if config.current.is_none() {
        config.current = Some(name.to_string());
    }
    crate::keyring::set_key(name, api_key)?;
    save_config(&config)?;
    Ok(())
}

pub fn config_get(key: &str) -> Result<()> {
    match key.to_lowercase().as_str() {
        "api-key" | "api_key" => {
            if runtime_overrides().api_key.is_some() {
                println!("provided via --api-key for this invocation");
            } else {
                let profile = current_profile()?;
                if crate::keyring::get_key(&profile)?.is_some() {
                    println!("stored in keyring for profile '{}'", profile);
                } else {
                    anyhow::bail!("No API key configured for profile '{}'", profile);
                }
            }
        }
        "profile" => {
            let profile = current_profile()?;
            println!("{}", profile);
        }
        "default-team" | "default_team" | "team" | "context.team" => {
            if let Some(team) = load_config()?.context.defaults.team {
                println!("{}", team);
            } else {
                anyhow::bail!(
                    "No default team configured. Run: linear config set default-team TEAM"
                );
            }
        }
        "default-status" | "default_status" | "status" | "context.status" => {
            if let Some(status) = load_config()?.context.defaults.status {
                println!("{}", status);
            } else {
                anyhow::bail!(
                    "No default status configured. Run: linear config set default-status STATUS"
                );
            }
        }
        "default-labels" | "default_labels" | "labels" | "context.labels" => {
            let labels = load_config()?.context.defaults.labels;
            println!("{}", labels.join(","));
        }
        _ => anyhow::bail!("Unknown config key: {}", key),
    }
    Ok(())
}

pub fn config_set(key: &str, value: &str) -> Result<()> {
    match key.to_lowercase().as_str() {
        "api-key" | "api_key" => anyhow::bail!(
            "Setting API keys via positional arguments is disabled. Use 'linear auth login' or 'linear config set-key'."
        ),
        "profile" => workspace_switch(value),
        "default-team" | "default_team" | "team" | "context.team" => set_context_value(|context| {
            context.version = context.version.max(1);
            context.defaults.team = Some(value.trim().to_string());
            Ok(())
        }),
        "default-status" | "default_status" | "status" | "context.status" => set_context_value(|context| {
            context.version = context.version.max(1);
            context.defaults.status = Some(value.trim().to_string());
            Ok(())
        }),
        "default-labels" | "default_labels" | "labels" | "context.labels" => set_context_value(|context| {
            context.version = context.version.max(1);
            context.defaults.labels = parse_csv_values(value);
            Ok(())
        }),
        _ => anyhow::bail!("Unknown config key: {}", key),
    }
}

fn set_context_value<F>(update: F) -> Result<()>
where
    F: FnOnce(&mut LinearContextConfig) -> Result<()>,
{
    let mut config = load_config()?;
    update(&mut config.context)?;
    save_config(&config)
}

fn parse_csv_values(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect()
}

pub fn show_config() -> Result<()> {
    let config = load_config()?;
    let path = config_path()?;

    println!("Config file: {}", path.display());
    println!();

    if let Some(current) = &config.current {
        println!("Current workspace: {}", current);
        if let Some(workspace) = config.workspaces.get(current) {
            println!("API Key: {}", api_key_status(current)?);
            println!("OAuth: {}", oauth_status(workspace));
        }
    } else {
        println!("No workspace configured. Run: linear workspace add <name>");
    }

    println!();
    println!("Context defaults:");
    println!(
        "  Default team: {}",
        config
            .context
            .defaults
            .team
            .as_deref()
            .unwrap_or("not configured")
    );
    println!(
        "  Default status: {}",
        config
            .context
            .defaults
            .status
            .as_deref()
            .unwrap_or("not configured")
    );
    println!(
        "  Default labels: {}",
        if config.context.defaults.labels.is_empty() {
            "not configured".to_string()
        } else {
            config.context.defaults.labels.join(", ")
        }
    );
    println!(
        "  Label groups: {}",
        if config.context.label_groups.is_empty() {
            "not configured".to_string()
        } else {
            config
                .context
                .label_groups
                .iter()
                .map(|group| group.key.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        }
    );

    Ok(())
}

pub fn resolved_context() -> Result<ResolvedLinearContext> {
    let user = load_config()?.context;
    let (project, project_file) = load_project_context()?;
    let resolved = match &project {
        Some(project) => user.merge(project),
        None => user.clone(),
    };

    Ok(ResolvedLinearContext {
        user,
        project,
        project_file,
        resolved,
    })
}

pub fn load_project_context() -> Result<(Option<LinearContextConfig>, Option<PathBuf>)> {
    let Some(path) = find_project_context_file()? else {
        return Ok((None, None));
    };
    let content = fs::read_to_string(&path)?;
    let project_file: ProjectContextFile = toml::from_str(&content)?;
    Ok((Some(project_file.context), Some(path)))
}

fn find_project_context_file() -> Result<Option<PathBuf>> {
    let mut current = std::env::current_dir()?;
    loop {
        let candidate = current.join(".linear.toml");
        if candidate.exists() {
            return Ok(Some(candidate));
        }
        if !current.pop() {
            return Ok(None);
        }
    }
}

pub fn write_project_context(path: &Path, context: &LinearContextConfig) -> Result<()> {
    let file = ProjectContextFile {
        context: context.clone(),
    };
    fs::write(path, toml::to_string_pretty(&file)?)?;
    Ok(())
}

// Workspace management functions

pub fn workspace_add(name: &str, api_key: Option<&str>) -> Result<()> {
    let mut config = load_config()?;

    if config.workspaces.contains_key(name) {
        anyhow::bail!(
            "Workspace '{}' already exists. Use 'workspace remove' first to replace it.",
            name
        );
    }

    config.workspaces.insert(
        name.to_string(),
        Workspace {
            api_key: String::new(),
            oauth: None,
        },
    );

    if let Some(api_key) = api_key {
        crate::keyring::set_key(name, api_key)?;
    }

    // If this is the first workspace, make it current
    if config.current.is_none() {
        config.current = Some(name.to_string());
    }

    save_config(&config)?;
    println!("Workspace '{}' added successfully!", name);

    if config.current.as_ref() == Some(&name.to_string()) {
        println!("Switched to workspace '{}'", name);
    }

    Ok(())
}

pub fn workspace_list() -> Result<()> {
    let config = load_config()?;

    if config.workspaces.is_empty() {
        println!("No workspaces configured. Run: linear workspace add <name>");
        return Ok(());
    }

    println!("Configured workspaces:");
    println!();

    for (name, workspace) in &config.workspaces {
        let is_current = config.current.as_ref() == Some(name);
        let marker = if is_current { "*" } else { " " };
        let auth_summary = workspace_auth_summary(name, workspace)?;
        println!("{} {} ({})", marker, name, auth_summary);
    }

    println!();
    println!("* = current workspace");

    Ok(())
}

pub fn workspace_switch(name: &str) -> Result<()> {
    let mut config = load_config()?;

    if !config.workspaces.contains_key(name) {
        anyhow::bail!(
            "Workspace '{}' not found. Use 'workspace list' to see available workspaces.",
            name
        );
    }

    config.current = Some(name.to_string());
    save_config(&config)?;
    println!("Switched to workspace '{}'", name);

    Ok(())
}

pub fn workspace_current() -> Result<()> {
    let config = load_config()?;

    if let Some(current) = &config.current {
        println!("Current workspace: {}", current);
        if let Some(workspace) = config.workspaces.get(current) {
            println!("API Key: {}", api_key_status(current)?);
            println!("OAuth: {}", oauth_status(workspace));
        }
    } else {
        println!("No workspace selected. Run: linear workspace add <name>");
    }

    Ok(())
}

pub fn workspace_remove(name: &str) -> Result<()> {
    let mut config = load_config()?;

    if !config.workspaces.contains_key(name) {
        anyhow::bail!("Workspace '{}' not found.", name);
    }

    config.workspaces.remove(name);
    crate::keyring::delete_key(name)?;
    crate::keyring::delete_oauth_tokens(name)?;

    // If we removed the current workspace, clear it or switch to another
    if config.current.as_ref() == Some(&name.to_string()) {
        config.current = config.workspaces.keys().next().cloned();
        if let Some(new_current) = &config.current {
            println!("Switched to workspace '{}'", new_current);
        }
    }

    save_config(&config)?;
    println!("Workspace '{}' removed.", name);

    Ok(())
}

/// Save OAuth config for a profile
pub fn save_oauth_config(profile: &str, oauth_config: &OAuthConfig) -> Result<()> {
    let json = serde_json::to_string(oauth_config)?;
    crate::keyring::set_oauth_tokens(profile, &json)?;

    let mut config = load_config()?;
    let workspace = config
        .workspaces
        .entry(profile.to_string())
        .or_insert_with(|| Workspace {
            api_key: String::new(),
            oauth: None,
        });
    workspace.oauth = Some(oauth_metadata_only(oauth_config));
    if config.current.is_none() {
        config.current = Some(profile.to_string());
    }
    save_config(&config)?;
    Ok(())
}

fn oauth_config_has_secrets(oauth_config: &OAuthConfig) -> bool {
    !oauth_config.access_token.is_empty()
        || oauth_config
            .refresh_token
            .as_ref()
            .map(|token| !token.is_empty())
            .unwrap_or(false)
}

fn oauth_metadata_only(oauth_config: &OAuthConfig) -> OAuthConfig {
    OAuthConfig {
        client_id: oauth_config.client_id.clone(),
        access_token: String::new(),
        refresh_token: None,
        expires_at: oauth_config.expires_at,
        token_type: oauth_config.token_type.clone(),
        scopes: oauth_config.scopes.clone(),
    }
}

/// Get OAuth metadata for a profile from config, even if secrets are stored elsewhere.
pub fn get_oauth_metadata(profile: &str) -> Result<Option<OAuthConfig>> {
    let config = load_config()?;
    Ok(config.workspaces.get(profile).and_then(|w| w.oauth.clone()))
}

/// Get OAuth config for a profile
pub fn get_oauth_config(profile: &str) -> Result<Option<OAuthConfig>> {
    if let Some(json_str) = crate::keyring::get_oauth_tokens(profile)? {
        let oauth = serde_json::from_str::<OAuthConfig>(&json_str)
            .context("OAuth token payload stored in keyring was invalid")?;
        return Ok(Some(oauth));
    }

    Ok(None)
}

/// Clear OAuth config for a profile
pub fn clear_oauth_config(profile: &str) -> Result<()> {
    crate::keyring::delete_oauth_tokens(profile)?;

    let mut config = load_config()?;
    if let Some(workspace) = config.workspaces.get_mut(profile) {
        workspace.oauth = None;
    }
    save_config(&config)?;
    Ok(())
}

fn api_key_status(profile: &str) -> Result<&'static str> {
    Ok(if crate::keyring::get_key(profile)?.is_some() {
        "stored in keyring"
    } else {
        "not configured"
    })
}

fn oauth_status(workspace: &Workspace) -> &'static str {
    if workspace.oauth.is_some() {
        "configured"
    } else {
        "not configured"
    }
}

fn workspace_auth_summary(name: &str, workspace: &Workspace) -> Result<String> {
    let mut parts = Vec::new();
    if crate::keyring::get_key(name)?.is_some() {
        parts.push("api-key:keyring");
    }
    if workspace.oauth.is_some() {
        parts.push("oauth:keyring");
    }
    if parts.is_empty() {
        parts.push("no credentials");
    }
    Ok(parts.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(config.current.is_none());
        assert!(config.workspaces.is_empty());
        assert!(config.api_key.is_none());
    }

    #[test]
    fn test_config_serialize_deserialize() {
        let mut config = Config {
            current: Some("prod".to_string()),
            ..Default::default()
        };
        config.workspaces.insert(
            "prod".to_string(),
            Workspace {
                api_key: "lin_api_prod123".to_string(),
                oauth: None,
            },
        );
        config.workspaces.insert(
            "staging".to_string(),
            Workspace {
                api_key: "lin_api_staging456".to_string(),
                oauth: None,
            },
        );

        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();

        assert_eq!(parsed.current, Some("prod".to_string()));
        assert_eq!(parsed.workspaces.len(), 2);
        assert_eq!(parsed.workspaces["prod"].api_key, "lin_api_prod123");
        assert_eq!(parsed.workspaces["staging"].api_key, "lin_api_staging456");
    }

    #[test]
    fn test_config_legacy_migration_parse() {
        // Legacy config format with top-level api_key
        let toml_str = r#"
            api_key = "lin_api_legacy_key"
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.api_key, Some("lin_api_legacy_key".to_string()));
        assert!(config.workspaces.is_empty());
        assert!(config.current.is_none());
    }

    #[test]
    fn test_config_with_workspaces_parse() {
        let toml_str = r#"
            current = "default"

            [workspaces.default]
            api_key = "lin_api_key1"

            [workspaces.staging]
            api_key = "lin_api_key2"
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.current, Some("default".to_string()));
        assert_eq!(config.workspaces.len(), 2);
        assert!(config.api_key.is_none());
    }

    #[test]
    fn test_context_defaults_parse() {
        let toml_str = r#"
            current = "default"

            [context.defaults]
            team = "EPD"
            status = "Spec"
            labels = ["agentic"]

            [[context.label_groups]]
            key = "domain"
            linear_group = "domain"
            required = true
            cardinality = "exactly_one"
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.context.defaults.team.as_deref(), Some("EPD"));
        assert_eq!(config.context.defaults.status.as_deref(), Some("Spec"));
        assert_eq!(config.context.defaults.labels, vec!["agentic"]);
        assert_eq!(config.context.label_groups.len(), 1);
        assert_eq!(config.context.label_groups[0].key, "domain");
        assert!(config.context.label_groups[0].required);
    }

    #[test]
    fn test_context_merge_project_overrides_user() {
        let user = LinearContextConfig {
            defaults: ContextDefaults {
                team: Some("EPD".to_string()),
                status: Some("Triage".to_string()),
                labels: vec!["user".to_string()],
            },
            label_groups: vec![LabelGroupPolicy {
                key: "domain".to_string(),
                required: true,
                ..Default::default()
            }],
            ..Default::default()
        };
        let project = LinearContextConfig {
            defaults: ContextDefaults {
                team: None,
                status: Some("Spec".to_string()),
                labels: vec!["repo".to_string()],
            },
            label_groups: vec![LabelGroupPolicy {
                key: "type".to_string(),
                required: true,
                ..Default::default()
            }],
            ..Default::default()
        };

        let merged = user.merge(&project);
        assert_eq!(merged.defaults.team.as_deref(), Some("EPD"));
        assert_eq!(merged.defaults.status.as_deref(), Some("Spec"));
        assert_eq!(merged.defaults.labels, vec!["repo"]);
        assert_eq!(merged.label_groups.len(), 2);
    }

    #[test]
    fn test_config_api_key_not_serialized_when_none() {
        let config = Config {
            current: Some("default".to_string()),
            workspaces: HashMap::new(),
            context: LinearContextConfig::default(),
            api_key: None,
        };

        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(!toml_str.contains("api_key"));
    }

    #[test]
    fn test_config_with_oauth_parse() {
        let toml_str = r#"
            current = "oauth-profile"

            [workspaces.oauth-profile]
            api_key = ""

            [workspaces.oauth-profile.oauth]
            client_id = "abc123"
            access_token = "lin_oauth_xxx"
            refresh_token = "lin_refresh_yyy"
            expires_at = 1700000000
            token_type = "Bearer"
            scopes = ["read", "write"]
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.current, Some("oauth-profile".to_string()));
        let ws = &config.workspaces["oauth-profile"];
        assert!(ws.oauth.is_some());
        let oauth = ws.oauth.as_ref().unwrap();
        assert_eq!(oauth.client_id, "abc123");
        assert_eq!(oauth.access_token, "lin_oauth_xxx");
        assert_eq!(oauth.refresh_token, Some("lin_refresh_yyy".to_string()));
        assert_eq!(oauth.expires_at, Some(1700000000));
        assert_eq!(oauth.scopes, vec!["read", "write"]);
    }

    #[test]
    fn test_config_without_oauth_still_parses() {
        let toml_str = r#"
            current = "default"

            [workspaces.default]
            api_key = "lin_api_key1"
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        let ws = &config.workspaces["default"];
        assert!(ws.oauth.is_none());
    }

    #[test]
    fn test_oauth_config_serialize() {
        let oauth = OAuthConfig {
            client_id: "test".to_string(),
            access_token: "acc".to_string(),
            refresh_token: Some("ref".to_string()),
            expires_at: Some(1700000000),
            token_type: "Bearer".to_string(),
            scopes: vec!["read".to_string()],
        };
        let json = serde_json::to_string(&oauth).unwrap();
        let parsed: OAuthConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.client_id, "test");
    }

    #[test]
    fn test_config_with_mixed_profiles() {
        let toml_str = r#"
            current = "default"

            [workspaces.default]
            api_key = "lin_api_key1"

            [workspaces.oauth-ws]
            api_key = ""

            [workspaces.oauth-ws.oauth]
            client_id = "cid"
            access_token = "at"
            token_type = "Bearer"
            scopes = ["read"]
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.workspaces["default"].oauth.is_none());
        assert!(config.workspaces["oauth-ws"].oauth.is_some());
    }

    #[test]
    fn test_oauth_config_roundtrip_toml() {
        let mut config = Config {
            current: Some("oauth-test".to_string()),
            ..Default::default()
        };
        config.workspaces.insert(
            "oauth-test".to_string(),
            Workspace {
                api_key: String::new(),
                oauth: Some(OAuthConfig {
                    client_id: "my-app".to_string(),
                    access_token: "lin_oauth_token123".to_string(),
                    refresh_token: Some("lin_refresh_abc".to_string()),
                    expires_at: Some(1700000000),
                    token_type: "Bearer".to_string(),
                    scopes: vec!["read".to_string(), "write".to_string()],
                }),
            },
        );

        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();

        let ws = &parsed.workspaces["oauth-test"];
        let oauth = ws.oauth.as_ref().unwrap();
        assert_eq!(oauth.client_id, "my-app");
        assert_eq!(oauth.access_token, "lin_oauth_token123");
        assert_eq!(oauth.refresh_token.as_deref(), Some("lin_refresh_abc"));
        assert_eq!(oauth.expires_at, Some(1700000000));
        assert_eq!(oauth.token_type, "Bearer");
        assert_eq!(oauth.scopes, vec!["read", "write"]);
    }

    #[test]
    fn test_oauth_not_serialized_when_none() {
        let mut config = Config {
            current: Some("default".to_string()),
            ..Default::default()
        };
        config.workspaces.insert(
            "default".to_string(),
            Workspace {
                api_key: "lin_api_key".to_string(),
                oauth: None,
            },
        );

        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(
            !toml_str.contains("[workspaces.default.oauth]"),
            "oauth section should not appear when None"
        );
        assert!(!toml_str.contains("client_id"));
        assert!(!toml_str.contains("access_token"));
    }

    #[test]
    fn test_oauth_config_empty_scopes() {
        let toml_str = r#"
            current = "test"

            [workspaces.test]
            api_key = ""

            [workspaces.test.oauth]
            client_id = "cid"
            access_token = "tok"
            token_type = "Bearer"
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        let oauth = config.workspaces["test"].oauth.as_ref().unwrap();
        assert!(
            oauth.scopes.is_empty(),
            "scopes should default to empty vec"
        );
        assert!(oauth.refresh_token.is_none());
        assert!(oauth.expires_at.is_none());
    }

    #[test]
    fn test_oauth_config_json_roundtrip() {
        let oauth = OAuthConfig {
            client_id: "app-id".to_string(),
            access_token: "access".to_string(),
            refresh_token: Some("refresh".to_string()),
            expires_at: Some(1700086400),
            token_type: "Bearer".to_string(),
            scopes: vec![
                "read".to_string(),
                "write".to_string(),
                "issues:create".to_string(),
            ],
        };
        let json = serde_json::to_string(&oauth).unwrap();
        let parsed: OAuthConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.client_id, "app-id");
        assert_eq!(parsed.scopes.len(), 3);
        assert_eq!(parsed.scopes[2], "issues:create");
    }

    #[test]
    fn test_workspace_with_both_apikey_and_oauth() {
        let toml_str = r#"
            current = "dual"

            [workspaces.dual]
            api_key = "lin_api_key_primary"

            [workspaces.dual.oauth]
            client_id = "cid"
            access_token = "oauth_tok"
            token_type = "Bearer"
            scopes = ["read"]
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        let ws = &config.workspaces["dual"];
        assert_eq!(ws.api_key, "lin_api_key_primary");
        assert!(ws.oauth.is_some());
        assert_eq!(ws.oauth.as_ref().unwrap().access_token, "oauth_tok");
    }
}
