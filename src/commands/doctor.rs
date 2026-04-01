use anyhow::Result;
use dialoguer::Password;
use serde_json::json;

use crate::api::LinearClient;
use crate::cache::{self, Cache};
use crate::config;
use crate::output::{print_json_owned, OutputOptions};

pub async fn run(output: &OutputOptions, check_api: bool, fix: bool) -> Result<()> {
    let config_path = config::config_file_path()?;
    let config_data = config::load_config()?;
    let profile = config::current_profile().ok();
    let api_key_override = config::api_key_override_present();
    let profile_override = config::profile_override_name();
    let cache_dir = cache::cache_dir_path()?;

    let config_file_configured = profile
        .as_ref()
        .and_then(|p| config_data.workspaces.get(p))
        .is_some();

    let keyring_available = crate::keyring::is_available();

    let oauth_keyring_configured = profile
        .as_ref()
        .and_then(|p| crate::keyring::get_oauth_tokens(p).ok())
        .flatten()
        .is_some();

    let api_key_keyring_configured = profile
        .as_ref()
        .and_then(|p| crate::keyring::get_key(p).ok())
        .flatten()
        .is_some();

    let oauth_configured = profile
        .as_ref()
        .and_then(|p| config::get_oauth_metadata(p).ok())
        .flatten()
        .is_some();

    let configured = config_file_configured
        || api_key_keyring_configured
        || oauth_configured
        || oauth_keyring_configured;

    let mut api_ok = None;
    let mut api_error = None;
    if check_api || fix {
        match validate_api().await {
            Ok(()) => api_ok = Some(true),
            Err(err) => {
                api_ok = Some(false);
                api_error = Some(err.to_string());
            }
        }
    }

    // --fix mode: auto-remediate common issues
    if fix {
        let mut fixed = Vec::new();

        // Fix 1: Missing config file — create default
        if !config_path.exists() {
            let default_config = config::Config::default();
            config::save_config(&default_config)?;
            fixed.push("Created default config file");
        }

        // Fix 2: No profile configured — create a "default" workspace entry
        if !configured && profile.is_none() {
            // Only prompt if not in quiet/json mode
            if !output.is_json() && !crate::output::is_quiet() {
                println!("No API key configured.");
                let key = Password::new()
                    .with_prompt("Enter your Linear API key")
                    .interact()?;

                if !key.is_empty() {
                    config::set_api_key(&key)?;
                    fixed.push("Saved API key");
                }
            }
        }

        // Fix 3: Stale/corrupt cache — clear it
        if let Ok(cache) = Cache::new() {
            if let Ok(()) = cache.clear_all() {
                fixed.push("Cleared cache");
            }
        }

        // Fix 4: API key invalid — prompt for new one
        if api_ok == Some(false) && !output.is_json() && !crate::output::is_quiet() {
            println!("API key is invalid or expired.");
            let key = Password::new()
                .with_prompt("Enter a new Linear API key (or leave blank to skip)")
                .allow_empty_password(true)
                .interact()?;

            if !key.is_empty() {
                config::set_api_key(&key)?;
                fixed.push("Updated API key");

                // Re-validate
                match validate_api().await {
                    Ok(()) => {
                        api_ok = Some(true);
                        api_error = None;
                        fixed.push("API key validated successfully");
                    }
                    Err(err) => {
                        api_error = Some(err.to_string());
                    }
                }
            }
        }

        if output.is_json() || output.has_template() {
            print_json_owned(
                json!({
                    "fix": true,
                    "fixed": fixed,
                    "config_path": config_path.to_string_lossy(),
                    "api_ok": api_ok,
                    "api_error": api_error,
                }),
                output,
            )?;
            return Ok(());
        }

        if fixed.is_empty() {
            println!("No issues found to fix.");
        } else {
            println!("Fixed:");
            for item in &fixed {
                println!("  + {}", item);
            }
        }

        if let Some(false) = api_ok {
            if let Some(err) = &api_error {
                println!("API still failing: {}", err);
            }
        }

        return Ok(());
    }

    if output.is_json() || output.has_template() {
        print_json_owned(
            json!({
                "config_path": config_path.to_string_lossy(),
                "profile": profile,
                "configured": configured,
                "keyring_available": keyring_available,
                "oauth_configured": oauth_configured,
                "oauth_keyring_configured": oauth_keyring_configured,
                "api_key_override": api_key_override,
                "profile_override": profile_override,
                "cache_dir": cache_dir.to_string_lossy(),
                "cache_ttl_seconds": output.cache.effective_ttl_seconds(),
                "api_ok": api_ok,
                "api_error": api_error,
            }),
            output,
        )?;
        return Ok(());
    }

    println!("Config path: {}", config_path.display());
    println!("Profile: {}", profile.unwrap_or_else(|| "none".to_string()));
    println!("Configured: {}", if configured { "yes" } else { "no" });
    println!(
        "Keyring available: {}",
        if keyring_available { "yes" } else { "no" }
    );
    println!(
        "OAuth configured: {}",
        if oauth_configured { "yes" } else { "no" }
    );
    println!(
        "OAuth keyring configured: {}",
        if oauth_keyring_configured {
            "yes"
        } else {
            "no"
        }
    );
    println!(
        "API key override: {}",
        if api_key_override { "yes" } else { "no" }
    );
    println!(
        "Profile override: {}",
        profile_override.unwrap_or_else(|| "none".to_string())
    );
    println!("Cache dir: {}", cache_dir.display());
    println!("Cache TTL: {}s", output.cache.effective_ttl_seconds());
    if let Some(api_ok) = api_ok {
        println!("API check: {}", if api_ok { "ok" } else { "failed" });
        if let Some(err) = api_error {
            println!("API error: {}", err);
        }
    }

    Ok(())
}

async fn validate_api() -> Result<()> {
    let client = LinearClient::new()?;
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
        anyhow::bail!("Viewer query failed");
    }
    Ok(())
}
