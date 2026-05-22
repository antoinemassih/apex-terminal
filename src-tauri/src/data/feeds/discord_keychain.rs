//! OS keychain storage for Discord OAuth tokens.
//!
//! Tokens (access_token, refresh_token, expires_epoch, user_id, username, avatar)
//! are encoded as a single JSON blob stored under one keychain entry so that
//! reads and writes are atomic at the OS level.
//!
//! Service name : "apex-terminal"
//! Account name : "discord-oauth"
//!
//! # Fail-safe policy
//!
//! If the keychain is unavailable (headless CI, Linux without Secret Service,
//! etc.) the functions return `Err` — callers must treat a missing keychain
//! entry as "user must re-authenticate." We intentionally do NOT fall back to
//! plaintext storage.
//!
//! # Sensitive data
//!
//! Token strings are NEVER included in error messages, log lines, or returned
//! `Err` strings.

use serde::{Deserialize, Serialize};

const SERVICE: &str = "apex-terminal";
const ACCOUNT: &str = "discord-oauth";

/// The persisted shape — deliberately separate from `DiscordAuth` so that the
/// memory representation (uses `std::time::Instant`) stays decoupled from the
/// on-disk/keychain representation (uses a plain `u64` epoch).
#[derive(Serialize, Deserialize)]
pub struct DiscordTokens {
    pub access_token: String,
    pub refresh_token: String,
    /// Seconds since UNIX epoch at which the access token expires.
    pub expires_epoch: u64,
    pub user_id: String,
    pub username: String,
    pub avatar: String,
}

/// Store tokens in the OS keychain.
///
/// Both tokens and all metadata are serialised as one JSON blob so the write
/// is atomic from the keychain's perspective.
///
/// # Errors
/// Returns `Err` if the keychain is unavailable or rejects the write.
/// Token content is never included in the error message.
pub fn store_tokens(tokens: &DiscordTokens) -> Result<(), String> {
    let blob = serde_json::to_string(tokens)
        .map_err(|e| format!("keychain serialize error: {e}"))?;

    let entry = keyring::Entry::new(SERVICE, ACCOUNT)
        .map_err(|e| format!("keychain entry create error: {e}"))?;

    entry
        .set_password(&blob)
        .map_err(|e| format!("keychain write error: {e}"))?;

    Ok(())
}

/// Load tokens from the OS keychain.
///
/// Returns `Ok(None)` when no entry exists yet (first run / after disconnect).
/// Returns `Err` when the keychain itself is unavailable.
///
/// # Errors
/// Token content is never included in the error message.
pub fn load_tokens() -> Result<Option<DiscordTokens>, String> {
    let entry = keyring::Entry::new(SERVICE, ACCOUNT)
        .map_err(|e| format!("keychain entry create error: {e}"))?;

    match entry.get_password() {
        Ok(blob) => {
            let tokens: DiscordTokens = serde_json::from_str(&blob)
                .map_err(|e| format!("keychain deserialize error: {e}"))?;
            Ok(Some(tokens))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("keychain read error: {e}")),
    }
}

/// Delete the keychain entry (used on disconnect or after a failed migration).
///
/// Returns `Ok(())` even if no entry existed.
pub fn delete_tokens() -> Result<(), String> {
    let entry = keyring::Entry::new(SERVICE, ACCOUNT)
        .map_err(|e| format!("keychain entry create error: {e}"))?;

    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("keychain delete error: {e}")),
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trip test using the keyring mock backend (no real OS keychain).
    ///
    /// The `keyring` crate exposes `keyring::set_default_credential_builder`
    /// to swap in an in-memory mock backend, which is exactly what we use here
    /// so tests are hermetic and don't require a real keychain service.
    // `set_default_credential_builder` mutates a PROCESS-GLOBAL backend, so the
    // keychain tests are not safe to run in parallel with each other. Ignored
    // by default — run with: cargo test -- --ignored --test-threads=1
    #[test]
    #[ignore = "mutates the process-global keyring backend; not parallel-safe"]
    fn round_trip_mock_keychain() {
        // Install the in-memory mock so this test never touches the OS keychain.
        keyring::set_default_credential_builder(keyring::mock::default_credential_builder());

        let original = DiscordTokens {
            access_token: "test_access_abc123".to_string(),
            refresh_token: "test_refresh_xyz789".to_string(),
            expires_epoch: 9_999_999_999,
            user_id: "123456789".to_string(),
            username: "TestUser".to_string(),
            avatar: "avatar_hash".to_string(),
        };

        // Store
        store_tokens(&original).expect("store should succeed with mock backend");

        // Load
        let loaded = load_tokens()
            .expect("load should succeed with mock backend")
            .expect("entry should be present after store");

        assert_eq!(loaded.access_token, original.access_token);
        assert_eq!(loaded.refresh_token, original.refresh_token);
        assert_eq!(loaded.expires_epoch, original.expires_epoch);
        assert_eq!(loaded.user_id, original.user_id);
        assert_eq!(loaded.username, original.username);
        assert_eq!(loaded.avatar, original.avatar);

        // Delete
        delete_tokens().expect("delete should succeed");

        // After delete: load returns None
        let after_delete = load_tokens()
            .expect("load after delete should not error")
            .is_none();
        assert!(after_delete, "no entry should exist after delete");
    }

    #[test]
    #[ignore = "mutates the process-global keyring backend; not parallel-safe"]
    fn load_missing_returns_none() {
        keyring::set_default_credential_builder(keyring::mock::default_credential_builder());

        // No store — should get Ok(None)
        let result = load_tokens().expect("load with no entry should not error");
        assert!(result.is_none());
    }
}
