use anyhow::Result;
use clap::Subcommand;
use dialoguer::{Confirm, Password};
use serde_json::json;

use crate::api::LinearClient;
use crate::config;
use crate::oauth;
use crate::output::{print_json_owned, OutputOptions};

#[derive(Subcommand)]
pub enum AuthCommands {
    /// Store API key for the current profile
    Login {
        /// API key to store (if omitted, prompt interactively)
        #[arg(long, value_name = "KEY")]
        key: Option<String>,
        /// Validate the API key before saving
        #[arg(long)]
        validate: bool,
    },
    /// Remove API key for the current profile
    Logout {
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
        /// Remove the workspace profile after clearing credentials
        #[arg(long)]
        remove_profile: bool,
    },
    /// Show current auth status
    Status {
        /// Validate API access
        #[arg(long)]
        validate: bool,
    },
    /// Authenticate via OAuth 2.0 (browser-based)
    Oauth {
        /// OAuth client ID (uses default if not specified)
        #[arg(long)]
        client_id: Option<String>,
        /// OAuth scopes (comma-separated, defaults to read,write)
        #[arg(long, default_value = "read,write")]
        scopes: String,
        /// Add admin scope explicitly
        #[arg(long)]
        admin: bool,
        /// Port for localhost callback server
        #[arg(long, default_value = "8484")]
        port: u16,
    },
    /// Revoke OAuth tokens for the current profile
    Revoke {
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },
}

pub async fn handle(cmd: AuthCommands, output: &OutputOptions) -> Result<()> {
    match cmd {
        AuthCommands::Login { key, validate } => login(key, validate, output).await,
        AuthCommands::Logout {
            force,
            remove_profile,
        } => logout(force, remove_profile, output).await,
        AuthCommands::Status { validate } => status(validate, output).await,
        AuthCommands::Oauth {
            client_id,
            scopes,
            admin,
            port,
        } => oauth_login(client_id, scopes, admin, port, output).await,
        AuthCommands::Revoke { force } => revoke(force, output).await,
    }
}

async fn login(key: Option<String>, validate: bool, output: &OutputOptions) -> Result<()> {
    let key = match key {
        Some(key) => key,
        None => Password::new().with_prompt("Linear API key").interact()?,
    };

    if validate {
        validate_key(&key).await?;
    }

    let profile = resolve_profile_for_write()?;
    config::set_workspace_key(&profile, &key)?;

    if output.is_json() || output.has_template() {
        print_json_owned(
            json!({
                "profile": profile,
                "saved": true,
                "storage": "keyring"
            }),
            output,
        )?;
        return Ok(());
    }

    println!("API key saved to keyring for profile '{}'", profile);
    Ok(())
}

async fn logout(force: bool, remove_profile: bool, output: &OutputOptions) -> Result<()> {
    let profile = config::current_profile()?;

    if !force && !crate::is_yes() {
        let confirmed = Confirm::new()
            .with_prompt(format!(
                "Remove stored credentials for profile '{}' ?",
                profile
            ))
            .default(false)
            .interact()?;
        if !confirmed {
            return Ok(());
        }
    }

    let _ = crate::keyring::delete_key(&profile);
    let _ = crate::keyring::delete_oauth_tokens(&profile);

    config::clear_oauth_config(&profile)?;
    if remove_profile {
        config::workspace_remove(&profile)?;
    }

    if output.is_json() || output.has_template() {
        print_json_owned(
            json!({
                "profile": profile,
                "cleared": true,
                "profile_removed": remove_profile,
            }),
            output,
        )?;
        return Ok(());
    }

    if remove_profile {
        println!(
            "Removed stored credentials and deleted profile '{}'",
            profile
        );
    } else {
        println!("Removed stored credentials for profile '{}'", profile);
    }
    Ok(())
}

async fn status(validate: bool, output: &OutputOptions) -> Result<()> {
    let config_data = config::load_config()?;
    let profile = config::current_profile().ok();
    let key_override = config::api_key_override_present();
    let profile_override = config::profile_override_name();

    let config_file_configured = profile
        .as_ref()
        .and_then(|p| config_data.workspaces.get(p))
        .map(|w| !w.api_key.is_empty())
        .unwrap_or(false);

    // Check keyring storage
    let api_key_keyring_configured = profile
        .as_ref()
        .and_then(|p| crate::keyring::get_key(p).ok())
        .flatten()
        .is_some();

    let oauth_keyring_configured = profile
        .as_ref()
        .and_then(|p| crate::keyring::get_oauth_tokens(p).ok())
        .flatten()
        .is_some();

    let keyring_available = crate::keyring::is_available();

    let keyring_configured = api_key_keyring_configured || oauth_keyring_configured;

    // Check OAuth config
    let oauth_metadata = profile
        .as_ref()
        .and_then(|p| config::get_oauth_metadata(p).ok())
        .flatten();
    let oauth_configured = oauth_metadata.is_some();
    let oauth_config = profile
        .as_ref()
        .and_then(|p| config::get_oauth_config(p).ok())
        .flatten();
    let oauth_usable = oauth_config
        .as_ref()
        .map(|oauth| !oauth.access_token.is_empty())
        .unwrap_or(false);
    let auth_type = if oauth_usable {
        "oauth"
    } else if oauth_configured {
        "oauth_metadata_only"
    } else if key_override || api_key_keyring_configured {
        "api_key"
    } else {
        "none"
    };
    let configured = config_file_configured || keyring_configured || oauth_configured;
    let oauth_warning = if oauth_configured && !oauth_usable {
        Some(
            "OAuth metadata exists for this profile, but the OAuth token is missing from the keyring. Re-run 'linear-cli auth oauth' or clear stale auth with 'linear-cli auth logout --force'.",
        )
    } else {
        None
    };

    let mut validated = None;
    if validate {
        if oauth_usable {
            // Validate OAuth by querying the viewer with the access token
            if let Some(ref oauth) = oauth_config {
                let client = LinearClient::with_api_key(format!("Bearer {}", oauth.access_token));
                validated = match client {
                    Ok(c) => {
                        let query = r#"query { viewer { id } }"#;
                        Some(
                            c.query(query, None)
                                .await
                                .map(|r| !r["data"]["viewer"].is_null())
                                .unwrap_or(false),
                        )
                    }
                    Err(_) => Some(false),
                };
            } else {
                validated = Some(false);
            }
        } else {
            // Validate API key using the priority: env > keyring > config
            let key = config::get_api_key().ok();
            validated = match key {
                Some(key) => Some(validate_key(&key).await.is_ok()),
                None => Some(false),
            };
        }
    }

    if output.is_json() || output.has_template() {
        print_json_owned(
            json!({
                "profile": profile,
                "configured": configured,
                "keyring_configured": keyring_configured,
                "oauth_keyring_configured": oauth_keyring_configured,
                "keyring_available": keyring_available,
                "auth_type": auth_type,
                "oauth_configured": oauth_configured,
                "oauth_scopes": oauth_metadata.as_ref().map(|o| &o.scopes),
                "oauth_expires_at": oauth_metadata.as_ref().and_then(|o| o.expires_at),
                "api_key_override": key_override,
                "profile_override": profile_override,
                "validated": validated,
                "warning": oauth_warning,
            }),
            output,
        )?;
        return Ok(());
    }

    println!(
        "Profile: {}",
        profile.clone().unwrap_or_else(|| "none".to_string())
    );
    println!("Config file: {}", if configured { "yes" } else { "no" });
    println!("Keyring: {}", if keyring_configured { "yes" } else { "no" });
    println!(
        "Keyring available: {}",
        if keyring_available { "yes" } else { "no" }
    );
    println!(
        "OAuth keyring: {}",
        if oauth_keyring_configured {
            "yes"
        } else {
            "no"
        }
    );
    println!(
        "API key override: {}",
        if key_override { "yes" } else { "no" }
    );
    if let Some(validated) = validated {
        println!("Validated: {}", if validated { "yes" } else { "no" });
    }
    println!("Auth type: {}", auth_type);
    if let Some(ref oauth) = oauth_metadata {
        println!("OAuth scopes: {:?}", oauth.scopes);
        if let Some(expires) = oauth.expires_at {
            let dt = chrono::DateTime::from_timestamp(expires, 0)
                .map(|d| d.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                .unwrap_or_else(|| "unknown".to_string());
            println!("OAuth expires: {}", dt);
        }
    }
    println!(
        "Profile override: {}",
        profile_override.unwrap_or_else(|| "none".to_string())
    );
    if let Some(warning) = oauth_warning {
        println!("Warning: {}", warning);
    }

    Ok(())
}

async fn validate_key(key: &str) -> Result<()> {
    let client = LinearClient::with_api_key(key.to_string())?;
    let query = r#"
        query {
            viewer {
                id
            }
        }
    "#;
    let result = client.query(query, None).await?;
    let viewer = &result["data"]["viewer"];
    if viewer.is_null() {
        anyhow::bail!("API key validation failed");
    }
    Ok(())
}

fn resolve_profile_for_write() -> Result<String> {
    if let Some(profile) = config::profile_override_name() {
        return Ok(profile);
    }
    let config_data = config::load_config()?;
    Ok(config_data.current.unwrap_or_else(|| "default".to_string()))
}

async fn oauth_login(
    client_id: Option<String>,
    scopes: String,
    admin: bool,
    port: u16,
    output: &OutputOptions,
) -> Result<()> {
    let client_id = client_id.unwrap_or_else(|| oauth::DEFAULT_CLIENT_ID.to_string());
    let redirect_uri = format!("http://localhost:{}/callback", port);
    let scopes = normalize_scopes(&scopes, admin);

    // Generate PKCE challenge and state
    let pkce = oauth::PkceChallenge::generate();
    let state = oauth::generate_state();

    // Build authorization URL
    let authorize_url =
        oauth::build_authorize_url(&client_id, &redirect_uri, &scopes, &state, &pkce)?;

    println!("Opening browser for Linear OAuth authentication...");
    println!("If the browser doesn't open, visit this URL:");
    println!("{}", authorize_url);
    println!();

    // Open browser
    if let Err(e) = open::that(&authorize_url) {
        eprintln!(
            "Failed to open browser: {}. Please open the URL above manually.",
            e
        );
    }

    // Wait for callback
    println!("Waiting for authorization callback on port {}...", port);
    let code = oauth::wait_for_callback(port, &state).await?;

    // Exchange code for tokens
    println!("Exchanging authorization code for tokens...");
    let tokens = oauth::exchange_code(&client_id, &redirect_uri, &code, &pkce.verifier).await?;

    // Validate the tokens by querying the viewer
    let client = LinearClient::with_api_key(format!("Bearer {}", tokens.access_token))?;
    let query = r#"query { viewer { id name email } }"#;
    let result = client.query(query, None).await?;
    let viewer = &result["data"]["viewer"];
    if viewer.is_null() {
        anyhow::bail!("OAuth token validation failed - could not fetch user info");
    }

    let user_name = viewer["name"].as_str().unwrap_or("Unknown");
    let user_email = viewer["email"].as_str().unwrap_or("Unknown");

    // Save tokens
    let profile = resolve_profile_for_write()?;
    let scopes_vec: Vec<String> = scopes.split(',').map(|s| s.trim().to_string()).collect();

    let oauth_config = config::OAuthConfig {
        client_id: client_id.clone(),
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        expires_at: tokens.expires_at,
        token_type: tokens.token_type,
        scopes: scopes_vec.clone(),
    };

    config::save_oauth_config(&profile, &oauth_config)?;

    if output.is_json() || output.has_template() {
        print_json_owned(
            json!({
                "profile": profile,
                "auth_type": "oauth",
                "user": user_name,
                "email": user_email,
                "scopes": scopes_vec,
                "storage": "keyring",
                "saved": true,
            }),
            output,
        )?;
        return Ok(());
    }

    println!();
    println!("OAuth authentication successful!");
    println!("  User: {} ({})", user_name, user_email);
    println!("  Scopes: {}", scopes);
    println!("  Tokens saved to keyring for profile '{}'", profile);

    Ok(())
}

fn normalize_scopes(scopes: &str, admin: bool) -> String {
    let mut normalized = Vec::new();

    for scope in scopes.split(',') {
        let scope = scope.trim();
        if !scope.is_empty() && !normalized.iter().any(|existing| existing == scope) {
            normalized.push(scope.to_string());
        }
    }

    if admin && !normalized.iter().any(|scope| scope == "admin") {
        normalized.push("admin".to_string());
    }

    normalized.join(",")
}

async fn revoke(force: bool, output: &OutputOptions) -> Result<()> {
    let profile = config::current_profile()?;

    let oauth_config = config::get_oauth_config(&profile)?;
    let oauth_config = match oauth_config {
        Some(c) => c,
        None => {
            anyhow::bail!(
                "No OAuth tokens found for profile '{}'. Use 'auth oauth' to authenticate.",
                profile
            );
        }
    };

    if !force && !crate::is_yes() {
        let confirmed = Confirm::new()
            .with_prompt(format!("Revoke OAuth tokens for profile '{}'?", profile))
            .default(false)
            .interact()?;
        if !confirmed {
            return Ok(());
        }
    }

    // Revoke the access token with Linear
    if let Err(e) = oauth::revoke_token(&oauth_config.access_token).await {
        eprintln!("Warning: Failed to revoke token with Linear: {}", e);
        eprintln!("Clearing local tokens anyway...");
    }

    // Clear local tokens
    config::clear_oauth_config(&profile)?;

    if output.is_json() || output.has_template() {
        print_json_owned(
            json!({
                "profile": profile,
                "revoked": true,
            }),
            output,
        )?;
        return Ok(());
    }

    println!("OAuth tokens revoked for profile '{}'", profile);
    Ok(())
}
