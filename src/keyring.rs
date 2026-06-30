//! Secure API key storage using OS keyring.
//!
//! This module provides cross-platform credential storage:
//! - macOS: Keychain
//! - Windows: Credential Manager
//! - Linux: Secret Service (requires D-Bus and a keyring daemon)

use anyhow::{Context, Result};
use keyring::{Entry, Error as KeyringError};

const SERVICE_NAME: &str = "linear-cli";

fn entry(service_name: &str, profile: &str) -> Result<Entry> {
    // keyring 4.x's default `v1` mode selects the platform-native store and
    // uses Secret Service on Linux, preserving this fork's keyring-only
    // credential behavior without falling back to plaintext config.
    Entry::new(service_name, profile).context("Failed to create keyring entry")
}

fn verify_stored_secret(
    service_name: &str,
    profile: &str,
    expected: &str,
    secret_kind: &str,
) -> Result<()> {
    let verify_entry =
        entry(service_name, profile).context("Failed to create keyring entry for verification")?;

    match verify_entry.get_password() {
        Ok(actual) if actual == expected => Ok(()),
        Ok(_) => anyhow::bail!(
            "{} was written to the keyring but could not be read back unchanged",
            secret_kind
        ),
        Err(KeyringError::NoEntry) => anyhow::bail!(
            "{} could not be read back from the keyring after writing it",
            secret_kind
        ),
        Err(e) => Err(e).context(format!(
            "Failed to verify {} after writing it to the keyring",
            secret_kind
        )),
    }
}

/// Get an API key from the keyring for a profile.
/// Returns Ok(None) if no key is stored, Ok(Some(key)) if found.
pub fn get_key(profile: &str) -> Result<Option<String>> {
    let entry = entry(SERVICE_NAME, profile)?;

    match entry.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(e) => Err(e).context("Failed to read API key from keyring"),
    }
}

/// Store an API key in the keyring for a profile.
pub fn set_key(profile: &str, api_key: &str) -> Result<()> {
    let entry = entry(SERVICE_NAME, profile)?;

    entry
        .set_password(api_key)
        .context("Failed to store API key in keyring")?;

    verify_stored_secret(SERVICE_NAME, profile, api_key, "API key")?;

    Ok(())
}

/// Delete an API key from the keyring for a profile.
/// Returns Ok(()) even if no key was stored.
pub fn delete_key(profile: &str) -> Result<()> {
    let entry = entry(SERVICE_NAME, profile)?;

    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(KeyringError::NoEntry) => Ok(()), // Already gone, that's fine
        Err(e) => Err(e).context("Failed to delete API key from keyring"),
    }
}

/// Check if keyring is available on this system.
pub fn is_available() -> bool {
    let probe_account = format!("__linear_cli_probe_{}__", std::process::id());
    let probe_secret = format!("probe-{}", std::process::id());

    match entry(SERVICE_NAME, &probe_account) {
        Ok(probe_entry) => {
            if probe_entry.set_password(&probe_secret).is_err() {
                return false;
            }

            let verified = entry(SERVICE_NAME, &probe_account)
                .ok()
                .and_then(|verify_entry| verify_entry.get_password().ok())
                .map(|actual| actual == probe_secret)
                .unwrap_or(false);
            let _ = probe_entry.delete_credential();
            verified
        }
        Err(_) => false,
    }
}

const OAUTH_SERVICE_NAME: &str = "linear-cli-oauth";

/// Get OAuth tokens JSON from keyring for a profile
pub fn get_oauth_tokens(profile: &str) -> Result<Option<String>> {
    let entry = entry(OAUTH_SERVICE_NAME, profile)?;

    match entry.get_password() {
        Ok(json) => Ok(Some(json)),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(e) => Err(e).context("Failed to read OAuth tokens from keyring"),
    }
}

/// Store OAuth tokens JSON in keyring for a profile
pub fn set_oauth_tokens(profile: &str, json: &str) -> Result<()> {
    let entry = entry(OAUTH_SERVICE_NAME, profile)?;
    entry
        .set_password(json)
        .context("Failed to store OAuth tokens in keyring")?;

    verify_stored_secret(OAUTH_SERVICE_NAME, profile, json, "OAuth tokens")?;

    Ok(())
}

/// Delete OAuth tokens from keyring for a profile
pub fn delete_oauth_tokens(profile: &str) -> Result<()> {
    let entry = entry(OAUTH_SERVICE_NAME, profile)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(KeyringError::NoEntry) => Ok(()),
        Err(e) => Err(e).context("Failed to delete OAuth tokens from keyring"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    const TEST_PROFILE: &str = "linear-cli-test-profile";
    const TEST_KEY: &str = "lin_api_test_key_12345";
    const TEST_OAUTH_JSON: &str = r#"{"client_id":"client","access_token":"access","refresh_token":"refresh","expires_at":1234567890,"token_type":"Bearer","scopes":["read","write"]}"#;

    fn keyring_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn lock_keyring_test() -> MutexGuard<'static, ()> {
        keyring_test_lock()
            .lock()
            .expect("keyring test lock should not be poisoned")
    }

    #[test]
    fn test_is_available() {
        let _guard = lock_keyring_test();

        // Just check it doesn't panic - availability depends on system
        let available = is_available();
        println!("Keyring available: {}", available);
    }

    #[test]
    fn test_set_get_delete_key() {
        let _guard = lock_keyring_test();

        if !is_available() {
            eprintln!("Skipping keyring test - keyring not available");
            return;
        }

        // Clean up any leftover from previous test runs
        let _ = delete_key(TEST_PROFILE);

        // Set a key - check for errors
        if let Err(e) = set_key(TEST_PROFILE, TEST_KEY) {
            eprintln!("Skipping test - set_key failed: {}", e);
            return;
        }

        // Get the key back
        match get_key(TEST_PROFILE) {
            Ok(Some(key)) => {
                assert_eq!(key, TEST_KEY, "Key should match");
            }
            Ok(None) => {
                // Some systems (like CI) may have keyring available but not persistent
                eprintln!("Warning: Key not found after set - keyring may not be persistent in this environment");
            }
            Err(e) => {
                eprintln!("Warning: get_key failed: {}", e);
            }
        }

        // Clean up
        let _ = delete_key(TEST_PROFILE);
    }

    #[test]
    fn test_delete_nonexistent_key() {
        let _guard = lock_keyring_test();

        if !is_available() {
            eprintln!("Skipping keyring test - keyring not available");
            return;
        }

        // Deleting a key that doesn't exist should not error
        let result = delete_key("nonexistent-profile-xyz");
        assert!(result.is_ok(), "Deleting nonexistent key should succeed");
    }

    #[test]
    fn test_overwrite_key() {
        let _guard = lock_keyring_test();

        if !is_available() {
            eprintln!("Skipping keyring test - keyring not available");
            return;
        }

        let profile = "linear-cli-test-overwrite";
        let _ = delete_key(profile); // Clean up

        // Set initial key - check for errors
        if let Err(e) = set_key(profile, "key1") {
            eprintln!("Skipping test - set_key failed: {}", e);
            return;
        }

        // Verify or skip if not persistent
        match get_key(profile) {
            Ok(Some(key)) if key == "key1" => {
                // Overwrite with new key
                set_key(profile, "key2").expect("Failed to set key2");
                if let Ok(Some(key2)) = get_key(profile) {
                    assert_eq!(key2, "key2", "Overwritten key should match");
                }
            }
            _ => {
                eprintln!("Warning: Keyring not persistent in this environment");
            }
        }

        // Clean up
        let _ = delete_key(profile);
    }

    #[test]
    fn test_set_get_delete_oauth_tokens() {
        let _guard = lock_keyring_test();

        if !is_available() {
            eprintln!("Skipping keyring test - keyring not available");
            return;
        }

        let profile = "linear-cli-test-oauth-profile";
        let _ = delete_oauth_tokens(profile);

        if let Err(e) = set_oauth_tokens(profile, TEST_OAUTH_JSON) {
            eprintln!("Skipping test - set_oauth_tokens failed: {}", e);
            return;
        }

        match get_oauth_tokens(profile) {
            Ok(Some(json)) => assert_eq!(json, TEST_OAUTH_JSON, "OAuth JSON should match"),
            Ok(None) => {
                eprintln!("Warning: OAuth tokens not found after set - keyring may not be persistent in this environment");
            }
            Err(e) => {
                eprintln!("Warning: get_oauth_tokens failed: {}", e);
            }
        }

        let _ = delete_oauth_tokens(profile);
    }
}
