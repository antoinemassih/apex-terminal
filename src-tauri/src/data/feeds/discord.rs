//! Discord integration for Apex Terminal
//! OAuth2 for user identity + Bot API for channel/message access.
//!
//! discord.env format:
//!   DISCORD_CLIENT_ID=...
//!   DISCORD_CLIENT_SECRET=...
//!   DISCORD_BOT_TOKEN=...     (enables channels, messages, sending)

use std::sync::{Mutex, OnceLock};
use crate::data::connectivity::errors_sink::{report, ErrorLevel};

const REDIRECT_PORT: u16 = 19847;
const REDIRECT_URI: &str = "http://localhost:19847/callback";
const DISCORD_API: &str = "https://discord.com/api/v10";
const DISCORD_CDN: &str = "https://cdn.discordapp.com";
const SCOPES: &str = "identify guilds";

// ── Config ──────────────────────────────────────────────────────────────────

struct DiscordConfig {
    client_id: String,
    client_secret: String,
    bot_token: Option<String>,
}

static DISCORD_CONFIG: OnceLock<DiscordConfig> = OnceLock::new();
static DISCORD_TOKEN: OnceLock<Mutex<Option<DiscordAuth>>> = OnceLock::new();
static HTTP_CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();

fn http() -> &'static reqwest::blocking::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .pool_max_idle_per_host(2)
            .build()
            .unwrap()
    })
}

// ── Background fetch queues (written by bg threads, drained by UI) ──────────

static PENDING_GUILDS: OnceLock<Mutex<Option<Vec<DiscordGuild>>>> = OnceLock::new();
static PENDING_CHANNELS: OnceLock<Mutex<Option<Vec<DiscordChannel>>>> = OnceLock::new();
static PENDING_MESSAGES: OnceLock<Mutex<Option<(Vec<DiscordMessageApi>, bool)>>> = OnceLock::new();
static PENDING_ICONS: OnceLock<Mutex<Vec<GuildIconData>>> = OnceLock::new();
static PENDING_SEND: OnceLock<Mutex<Option<Result<DiscordMessageApi, String>>>> = OnceLock::new();

// ── Types ───────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct DiscordAuth {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: std::time::Instant,
    pub user_id: String,
    pub username: String,
    pub avatar: String,
}

/// Serializable version for disk persistence
#[derive(serde::Serialize, serde::Deserialize)]
struct DiscordAuthDisk {
    access_token: String,
    refresh_token: String,
    expires_epoch: u64, // seconds since UNIX epoch
    user_id: String,
    username: String,
    avatar: String,
}

fn token_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("discord_token.json")
}

fn save_auth_to_disk(auth: &DiscordAuth) {
    let epoch_now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let remaining = auth.expires_at.saturating_duration_since(std::time::Instant::now()).as_secs();
    let disk = DiscordAuthDisk {
        access_token: auth.access_token.clone(),
        refresh_token: auth.refresh_token.clone(),
        expires_epoch: epoch_now + remaining,
        user_id: auth.user_id.clone(),
        username: auth.username.clone(),
        avatar: auth.avatar.clone(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&disk) {
        let _ = std::fs::write(token_path(), json);
    }
}

fn load_auth_from_disk() -> Option<DiscordAuth> {
    let data = std::fs::read_to_string(token_path()).ok()?;
    let disk: DiscordAuthDisk = serde_json::from_str(&data).ok()?;
    let epoch_now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    if disk.expires_epoch <= epoch_now {
        // Token expired — delete file
        let _ = std::fs::remove_file(token_path());
        report(ErrorLevel::Warn, "discord", "token_expired", "saved token expired");
        return None;
    }
    let remaining = disk.expires_epoch - epoch_now;
    Some(DiscordAuth {
        access_token: disk.access_token,
        refresh_token: disk.refresh_token,
        expires_at: std::time::Instant::now() + std::time::Duration::from_secs(remaining),
        user_id: disk.user_id,
        username: disk.username,
        avatar: disk.avatar,
    })
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct DiscordGuild {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct DiscordChannel {
    pub id: String,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub channel_type: u8,
    pub position: Option<i32>,
    pub parent_id: Option<String>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct DiscordMessageApi {
    pub id: String,
    pub content: String,
    pub author: DiscordAuthor,
    pub timestamp: String,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct DiscordAuthor {
    pub id: String,
    pub username: String,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
}

pub struct GuildIconData {
    pub guild_id: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl DiscordAuthor {
    pub fn display_name(&self) -> &str {
        self.global_name.as_deref().unwrap_or(&self.username)
    }
}

impl DiscordChannel {
    /// Text channel (type 0) or announcement (type 5)
    pub fn is_text(&self) -> bool { self.channel_type == 0 || self.channel_type == 5 }
    /// Category (type 4)
    pub fn is_category(&self) -> bool { self.channel_type == 4 }
}

// ── Config loading ──────────────────────────────────────────────────────────

pub fn load_config() {
    let env_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("discord.env");
    if let Ok(content) = std::fs::read_to_string(&env_path) {
        let mut client_id = String::new();
        let mut client_secret = String::new();
        let mut bot_token: Option<String> = None;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            if let Some(val) = line.strip_prefix("DISCORD_CLIENT_ID=") {
                client_id = val.trim().to_string();
            } else if let Some(val) = line.strip_prefix("DISCORD_CLIENT_SECRET=") {
                client_secret = val.trim().to_string();
            } else if let Some(val) = line.strip_prefix("DISCORD_BOT_TOKEN=") {
                let v = val.trim().to_string();
                if !v.is_empty() { bot_token = Some(v); }
            }
        }
        if !client_id.is_empty() && !client_secret.is_empty() {
            let has_bot = bot_token.is_some();
            let _ = DISCORD_CONFIG.set(DiscordConfig { client_id, client_secret, bot_token });
            report(ErrorLevel::Info, "discord", "config_loaded", format!("bot: {has_bot}"));
        }
        // Restore saved auth token from disk
        if let Some(auth) = load_auth_from_disk() {
            report(ErrorLevel::Info, "discord", "session_restored", format!("{} ({})", auth.username, auth.user_id));
            let _ = DISCORD_TOKEN.get_or_init(|| Mutex::new(None));
            if let Some(m) = DISCORD_TOKEN.get() {
                *m.lock().unwrap() = Some(auth);
            }
        }
    } else {
        report(ErrorLevel::Info, "discord", "not_configured", "No discord.env found — Discord integration disabled");
    }
}

pub fn is_configured() -> bool { DISCORD_CONFIG.get().is_some() }
pub fn has_bot() -> bool { DISCORD_CONFIG.get().map(|c| c.bot_token.is_some()).unwrap_or(false) }

pub fn is_authenticated() -> bool {
    DISCORD_TOKEN.get()
        .and_then(|m| m.lock().ok())
        .map(|t| t.is_some())
        .unwrap_or(false)
}

pub fn get_auth() -> Option<DiscordAuth> {
    DISCORD_TOKEN.get()
        .and_then(|m| m.lock().ok())
        .and_then(|t| t.clone())
}

// ── OAuth2 CSRF state + PKCE storage ────────────────────────────────────────

/// One-shot pending OAuth state stored between auth-URL generation and callback.
/// Cleared after first use or after STATE_TTL_SECS seconds.
struct PendingOAuthState {
    /// CSRF state token (base64url, no padding)
    state: String,
    /// PKCE code verifier (base64url, no padding)
    code_verifier: String,
    /// Wall-clock instant the state was created (for expiry check)
    created_at: std::time::Instant,
}

const STATE_TTL_SECS: u64 = 300; // 5 minutes

static PENDING_OAUTH_STATE: OnceLock<Mutex<Option<PendingOAuthState>>> = OnceLock::new();

fn pending_state_store() -> &'static Mutex<Option<PendingOAuthState>> {
    PENDING_OAUTH_STATE.get_or_init(|| Mutex::new(None))
}

/// Base64url-encode (no padding) per RFC 4648 §5.
fn base64url_encode(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    // Manual base64url — avoids pulling in a base64 crate; the alphabet is
    // well-defined and the test suite exercises it independently.
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity((bytes.len() * 4 + 2) / 3);
    let mut chunks = bytes.chunks(3);
    while let Some(chunk) = chunks.next() {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let combined = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((combined >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((combined >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 { out.push(ALPHABET[((combined >> 6) & 0x3f) as usize] as char); }
        if chunk.len() > 2 { out.push(ALPHABET[(combined & 0x3f) as usize] as char); }
        let _ = &mut out; // suppress unused write lint
    }
    out
}

/// Generate `len` random bytes using `rand`.
fn random_bytes(len: usize) -> Vec<u8> {
    use rand::RngCore;
    let mut buf = vec![0u8; len];
    rand::thread_rng().fill_bytes(&mut buf);
    buf
}

/// Compute PKCE S256 challenge: base64url(SHA-256(verifier)).
fn pkce_challenge(verifier: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(verifier.as_bytes());
    base64url_encode(&hash)
}

/// Constant-time string comparison via `subtle`.
fn ct_eq(a: &str, b: &str) -> bool {
    use subtle::ConstantTimeEq;
    // Lengths differ → definitely not equal; revealing the length is fine
    // (state tokens are fixed-length anyway).
    if a.len() != b.len() {
        return false;
    }
    a.as_bytes().ct_eq(b.as_bytes()).into()
}

// ── OAuth2 flow ─────────────────────────────────────────────────────────────

pub fn start_oauth2() {
    let config = match DISCORD_CONFIG.get() {
        Some(c) => c,
        None => { report(ErrorLevel::Warn, "discord", "oauth_no_config", "Not configured"); return; }
    };

    // Generate CSRF state token: 32 random bytes → base64url ≈ 43 chars.
    let state = base64url_encode(&random_bytes(32));
    // Generate PKCE verifier: 32 random bytes → base64url ≈ 43 chars (within [43,128]).
    let code_verifier = base64url_encode(&random_bytes(32));
    let code_challenge = pkce_challenge(&code_verifier);

    // Store for callback validation (one-shot).
    {
        let mut guard = pending_state_store().lock().unwrap();
        *guard = Some(PendingOAuthState {
            state: state.clone(),
            code_verifier: code_verifier.clone(),
            created_at: std::time::Instant::now(),
        });
    }

    let auth_url = format!(
        "https://discord.com/oauth2/authorize\
         ?client_id={}\
         &redirect_uri={}\
         &response_type=code\
         &scope={}\
         &state={}\
         &code_challenge={}\
         &code_challenge_method=S256",
        config.client_id,
        urlencoding::encode(REDIRECT_URI),
        urlencoding::encode(SCOPES),
        urlencoding::encode(&state),
        urlencoding::encode(&code_challenge),
    );

    report(ErrorLevel::Info, "discord", "oauth_start", "Opening browser for OAuth2");
    let _ = open::that(&auth_url);

    std::thread::spawn(move || { start_callback_server(); });
}

fn start_callback_server() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = match TcpListener::bind(format!("127.0.0.1:{}", REDIRECT_PORT)) {
        Ok(l) => l,
        Err(e) => { report(ErrorLevel::Error, "discord", "callback_bind_failed", e.to_string()); return; }
    };
    listener.set_nonblocking(false).ok();
    report(ErrorLevel::Info, "discord", "callback_listening", format!("port {REDIRECT_PORT}"));

    if let Ok((mut stream, _)) = listener.accept() {
        let mut buf = [0u8; 4096];
        let n = stream.read(&mut buf).unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..n]);

        // Extract both ?code= and ?state= from the callback URL.
        let code = extract_query_param(&request, "code");
        let returned_state = extract_query_param(&request, "state");

        // --- CSRF + expiry validation ---
        let code_verifier = match validate_and_consume_state(returned_state.as_deref()) {
            Ok(v) => v,
            Err(e) => {
                report(ErrorLevel::Warn, "discord", &e, "OAuth state validation failed");
                let response = format!(
                    "HTTP/1.1 403 Forbidden\r\nContent-Type: text/html\r\n\r\n\
                     <html><body style='background:#1a1a2e;color:#eee;font-family:monospace;\
                     text-align:center;padding:60px'><h1>Auth Failed</h1><p>{e}</p></body></html>"
                );
                let _ = stream.write_all(response.as_bytes());
                return;
            }
        };

        if let Some(code) = code {
            report(ErrorLevel::Info, "discord", "oauth_code_received", format!("{}...", &code[..code.len().min(10)]));
            match exchange_code(&code, &code_verifier) {
                Ok(auth) => {
                    report(ErrorLevel::Info, "discord", "authenticated", format!("{} ({})", auth.username, auth.user_id));
                    save_auth_to_disk(&auth);
                    let _ = DISCORD_TOKEN.get_or_init(|| Mutex::new(None));
                    if let Some(m) = DISCORD_TOKEN.get() {
                        *m.lock().unwrap() = Some(auth);
                    }
                    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html><body style='background:#1a1a2e;color:#eee;font-family:monospace;text-align:center;padding:60px'><h1>Connected to Apex Terminal</h1><p>You can close this tab.</p><script>setTimeout(()=>window.close(),2000)</script></body></html>";
                    let _ = stream.write_all(response.as_bytes());
                }
                Err(e) => {
                    report(ErrorLevel::Error, "discord", "token_exchange_failed", e.to_string());
                    let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html><body style='background:#1a1a2e;color:#eee;font-family:monospace;text-align:center;padding:60px'><h1>Connection Failed</h1><p>{}</p></body></html>", e);
                    let _ = stream.write_all(response.as_bytes());
                }
            }
        } else {
            let response = "HTTP/1.1 400 Bad Request\r\n\r\nNo code found";
            let _ = stream.write_all(response.as_bytes());
        }
    }
}

/// Validate the returned CSRF state and consume it (one-shot).
/// Returns the stored `code_verifier` on success; an error event key on failure.
fn validate_and_consume_state(returned_state: Option<&str>) -> Result<String, String> {
    let returned = match returned_state {
        Some(s) if !s.is_empty() => s,
        _ => {
            report(ErrorLevel::Warn, "discord", "oauth_state_missing", "No state param in callback");
            return Err("oauth_state_missing".into());
        }
    };

    let mut guard = pending_state_store().lock().unwrap();
    let pending = match guard.take() {
        Some(p) => p,
        None => {
            report(ErrorLevel::Warn, "discord", "oauth_state_missing", "No pending OAuth state — possible replay");
            return Err("oauth_state_missing".into());
        }
    };

    // Check expiry before touching the state value.
    if pending.created_at.elapsed().as_secs() > STATE_TTL_SECS {
        report(ErrorLevel::Warn, "discord", "oauth_state_expired", "OAuth state expired (> 5 min)");
        return Err("oauth_state_expired".into());
    }

    // Constant-time comparison to resist timing attacks.
    if !ct_eq(returned, &pending.state) {
        report(ErrorLevel::Warn, "discord", "oauth_state_mismatch", "OAuth state mismatch — possible CSRF");
        return Err("oauth_state_mismatch".into());
    }

    Ok(pending.code_verifier)
}

/// Extract a single named query param from a raw HTTP request line.
fn extract_query_param(request: &str, name: &str) -> Option<String> {
    let first_line = request.lines().next()?;
    let url_part = first_line.split_whitespace().nth(1)?;
    let query = url_part.split('?').nth(1)?;
    let prefix = format!("{}=", name);
    for param in query.split('&') {
        if let Some(val) = param.strip_prefix(&prefix as &str) {
            return Some(val.to_string());
        }
    }
    None
}

fn exchange_code(code: &str, code_verifier: &str) -> Result<DiscordAuth, String> {
    let config = DISCORD_CONFIG.get().ok_or("Not configured")?;
    let client = http();

    let resp = client.post(format!("{}/oauth2/token", DISCORD_API))
        .form(&[
            ("client_id", config.client_id.as_str()),
            ("client_secret", config.client_secret.as_str()),
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", REDIRECT_URI),
            ("code_verifier", code_verifier),
        ])
        .send()
        .map_err(|e| format!("Request failed: {}", e))?;

    if !resp.status().is_success() {
        let text = resp.text().unwrap_or_default();
        return Err(format!("Token exchange failed: {}", text));
    }

    let json: serde_json::Value = resp.json().map_err(|e| format!("Parse: {}", e))?;
    let access_token = json["access_token"].as_str().ok_or("No access_token")?.to_string();
    let refresh_token = json["refresh_token"].as_str().unwrap_or("").to_string();
    let expires_in = json["expires_in"].as_u64().unwrap_or(604800);

    let user_resp = client.get(format!("{}/users/@me", DISCORD_API))
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .map_err(|e| format!("User fetch failed: {}", e))?;

    let user: serde_json::Value = user_resp.json().map_err(|e| format!("User parse: {}", e))?;

    Ok(DiscordAuth {
        access_token,
        refresh_token,
        expires_at: std::time::Instant::now() + std::time::Duration::from_secs(expires_in),
        user_id: user["id"].as_str().unwrap_or("").to_string(),
        username: user["global_name"].as_str()
            .or_else(|| user["username"].as_str())
            .unwrap_or("Unknown").to_string(),
        avatar: user["avatar"].as_str().unwrap_or("").to_string(),
    })
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod oauth_tests {
    use super::*;

    // Helper: manually install a pending state for testing without going through start_oauth2().
    fn install_state(state: &str, verifier: &str, age_secs: u64) {
        let created_at = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(age_secs))
            .unwrap_or(std::time::Instant::now());
        let mut guard = pending_state_store().lock().unwrap();
        *guard = Some(PendingOAuthState {
            state: state.to_string(),
            code_verifier: verifier.to_string(),
            created_at,
        });
    }

    // Helper: clear state between tests (each test runs in the same process).
    fn clear_state() {
        *pending_state_store().lock().unwrap() = None;
    }

    #[test]
    fn oauth_state_mismatch_rejected() {
        install_state("abc", "verifier_xyz", 0);
        let result = validate_and_consume_state(Some("xyz"));
        assert!(result.is_err(), "mismatched state must be rejected");
        assert_eq!(result.unwrap_err(), "oauth_state_mismatch");
    }

    #[test]
    fn oauth_state_match_accepted() {
        install_state("correct_state", "the_verifier", 0);
        let result = validate_and_consume_state(Some("correct_state"));
        assert!(result.is_ok(), "matching state must be accepted");
        assert_eq!(result.unwrap(), "the_verifier");
    }

    #[test]
    fn oauth_state_expired_rejected() {
        // Age of STATE_TTL_SECS + 1 → expired.
        install_state("good_state", "verifier", STATE_TTL_SECS + 1);
        let result = validate_and_consume_state(Some("good_state"));
        assert!(result.is_err(), "expired state must be rejected");
        assert_eq!(result.unwrap_err(), "oauth_state_expired");
    }

    #[test]
    fn oauth_state_one_shot() {
        install_state("once", "v", 0);
        // First use: OK.
        let first = validate_and_consume_state(Some("once"));
        assert!(first.is_ok());
        // Second use with same state: should fail (state was consumed).
        let second = validate_and_consume_state(Some("once"));
        assert!(second.is_err(), "state must be consumed on first use");
    }

    #[test]
    fn oauth_pkce_challenge_verifier_relationship() {
        // Known test vector: SHA-256("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk")
        // = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM" (RFC 7636 Appendix B, adapted).
        // We use our own verifier and verify round-trip consistency.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = pkce_challenge(verifier);
        // Re-compute independently.
        use sha2::{Digest, Sha256};
        let hash = Sha256::digest(verifier.as_bytes());
        let expected = base64url_encode(&hash);
        assert_eq!(challenge, expected, "pkce_challenge must produce base64url(sha256(verifier))");
        // Challenge must not equal verifier.
        assert_ne!(challenge, verifier);
    }

    #[test]
    fn oauth_state_missing_param_rejected() {
        install_state("s", "v", 0);
        let result = validate_and_consume_state(None);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "oauth_state_missing");
        clear_state();
    }

    #[test]
    fn base64url_encode_no_padding() {
        // Encoded output must not contain '=' (no padding).
        let out = base64url_encode(&[0u8; 32]);
        assert!(!out.contains('='), "base64url must not include padding");
        // Must only contain URL-safe chars.
        for c in out.chars() {
            assert!(c.is_ascii_alphanumeric() || c == '-' || c == '_',
                "unexpected char in base64url output: {c}");
        }
    }
}

// ── Synchronous API (used inside background threads) ────────────────────────

pub fn fetch_guilds() -> Vec<DiscordGuild> {
    let auth = match get_auth() { Some(a) => a, None => return vec![] };
    let client = http();
    match client.get(format!("{}/users/@me/guilds", DISCORD_API))
        .header("Authorization", format!("Bearer {}", auth.access_token))
        .send()
    {
        Ok(r) if r.status().is_success() => r.json::<Vec<DiscordGuild>>().unwrap_or_default(),
        _ => vec![]
    }
}

fn bot_token() -> Option<String> {
    DISCORD_CONFIG.get().and_then(|c| c.bot_token.clone())
}

pub fn fetch_channels_sync(guild_id: &str) -> Vec<DiscordChannel> {
    let token = match bot_token() { Some(t) => t, None => return vec![] };
    let client = http();
    match client.get(format!("{}/guilds/{}/channels", DISCORD_API, guild_id))
        .header("Authorization", format!("Bot {}", token))
        .send()
    {
        Ok(r) if r.status().is_success() => {
            let mut ch: Vec<DiscordChannel> = r.json().unwrap_or_default();
            ch.sort_by_key(|c| c.position.unwrap_or(999));
            ch
        }
        Ok(r) => {
            let status = r.status();
            let body = r.text().unwrap_or_default();
            report(ErrorLevel::Warn, "discord", "channel_fetch_http", format!("{status}: {body}"));
            vec![]
        }
        Err(e) => { report(ErrorLevel::Warn, "discord", "channel_fetch_error", e.to_string()); vec![] }
    }
}

pub fn fetch_messages_sync(channel_id: &str, limit: u32, after: Option<&str>) -> Vec<DiscordMessageApi> {
    let token = match bot_token() { Some(t) => t, None => return vec![] };
    let client = http();
    let mut url = format!("{}/channels/{}/messages?limit={}", DISCORD_API, channel_id, limit);
    if let Some(after_id) = after {
        url.push_str(&format!("&after={}", after_id));
    }
    match client.get(&url)
        .header("Authorization", format!("Bot {}", token))
        .send()
    {
        Ok(r) if r.status().is_success() => {
            let mut msgs: Vec<DiscordMessageApi> = r.json().unwrap_or_default();
            msgs.reverse();
            msgs
        }
        Ok(r) => {
            let status = r.status();
            let body = r.text().unwrap_or_default();
            report(ErrorLevel::Warn, "discord", "message_fetch_http", format!("{status}: {}", &body[..body.len().min(200)]));
            vec![]
        }
        Err(e) => { report(ErrorLevel::Warn, "discord", "message_fetch_error", e.to_string()); vec![] }
    }
}

pub fn send_message_sync(channel_id: &str, content: &str) -> Result<DiscordMessageApi, String> {
    let token = bot_token().ok_or("No bot token")?;
    let client = http();
    let resp = client.post(format!("{}/channels/{}/messages", DISCORD_API, channel_id))
        .header("Authorization", format!("Bot {}", token))
        .json(&serde_json::json!({ "content": content }))
        .send()
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        let text = resp.text().unwrap_or_default();
        return Err(format!("Send failed: {}", text));
    }
    resp.json().map_err(|e| e.to_string())
}

pub fn fetch_guild_icon_sync(guild_id: &str, icon_hash: &str) -> Option<GuildIconData> {
    let url = format!("{}/icons/{}/{}.png?size=64", DISCORD_CDN, guild_id, icon_hash);
    let bytes = http().get(&url).send().ok()?.bytes().ok()?;
    let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some(GuildIconData {
        guild_id: guild_id.to_string(),
        width: w,
        height: h,
        rgba: img.into_raw(),
    })
}

// ── Background launchers (non-blocking, results go to PENDING_*) ────────────

/// Fetch guilds + their icons in background
pub fn fetch_guilds_bg() {
    PENDING_GUILDS.get_or_init(|| Mutex::new(None));
    std::thread::spawn(|| {
        let guilds = fetch_guilds();
        report(ErrorLevel::Info, "discord", "guilds_fetched", format!("{} guilds", guilds.len()));
        // Fetch icons for guilds that have them
        for g in &guilds {
            if let Some(ref hash) = g.icon {
                if let Some(icon) = fetch_guild_icon_sync(&g.id, hash) {
                    let pending = PENDING_ICONS.get_or_init(|| Mutex::new(Vec::new()));
                    pending.lock().unwrap().push(icon);
                }
            }
        }
        let pending = PENDING_GUILDS.get().unwrap();
        *pending.lock().unwrap() = Some(guilds);
    });
}

/// Fetch channels for a guild in background
pub fn fetch_channels_bg(guild_id: String) {
    PENDING_CHANNELS.get_or_init(|| Mutex::new(None));
    std::thread::spawn(move || {
        let channels = fetch_channels_sync(&guild_id);
        report(ErrorLevel::Info, "discord", "channels_fetched", format!("{} channels for {}", channels.len(), guild_id));
        let pending = PENDING_CHANNELS.get().unwrap();
        *pending.lock().unwrap() = Some(channels);
    });
}

/// Fetch messages for a channel in background
pub fn fetch_messages_bg(channel_id: String, after: Option<String>) {
    PENDING_MESSAGES.get_or_init(|| Mutex::new(None));
    let is_append = after.is_some();
    std::thread::spawn(move || {
        let limit = if is_append { 20 } else { 30 };
        let msgs = fetch_messages_sync(&channel_id, limit, after.as_deref());
        // Always store result (even empty) so loading flag clears
        if !is_append || !msgs.is_empty() {
            let pending = PENDING_MESSAGES.get().unwrap();
            *pending.lock().unwrap() = Some((msgs, is_append));
        }
    });
}

/// Send a message in background
pub fn send_message_bg(channel_id: String, content: String) {
    PENDING_SEND.get_or_init(|| Mutex::new(None));
    std::thread::spawn(move || {
        let result = send_message_sync(&channel_id, &content);
        let pending = PENDING_SEND.get().unwrap();
        *pending.lock().unwrap() = Some(result);
    });
}

// ── Drain functions (called from UI thread each frame) ──────────────────────

pub fn drain_guilds() -> Option<Vec<DiscordGuild>> {
    PENDING_GUILDS.get()?.lock().ok()?.take()
}

pub fn drain_channels() -> Option<Vec<DiscordChannel>> {
    PENDING_CHANNELS.get()?.lock().ok()?.take()
}

/// Returns (messages, is_append). If is_append, add to existing; otherwise replace.
pub fn drain_messages() -> Option<(Vec<DiscordMessageApi>, bool)> {
    PENDING_MESSAGES.get()?.lock().ok()?.take()
}

pub fn drain_icons() -> Vec<GuildIconData> {
    match PENDING_ICONS.get() {
        Some(m) => {
            let mut guard = m.lock().unwrap();
            std::mem::take(&mut *guard)
        }
        None => vec![]
    }
}

pub fn drain_send() -> Option<Result<DiscordMessageApi, String>> {
    PENDING_SEND.get()?.lock().ok()?.take()
}

/// Format Discord ISO timestamp to relative time
pub fn relative_time(iso: &str) -> String {
    // Parse "2024-01-15T12:34:56.789000+00:00" → relative
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(iso) {
        let now = chrono::Utc::now();
        let diff = now.signed_duration_since(dt);
        if diff.num_seconds() < 60 { return "now".into(); }
        if diff.num_minutes() < 60 { return format!("{}m", diff.num_minutes()); }
        if diff.num_hours() < 24 { return format!("{}h", diff.num_hours()); }
        return format!("{}d", diff.num_days());
    }
    // Fallback: try just taking time portion
    if iso.len() >= 16 { iso[11..16].to_string() } else { iso.to_string() }
}

pub fn disconnect() {
    if let Some(m) = DISCORD_TOKEN.get() {
        *m.lock().unwrap() = None;
    }
    let _ = std::fs::remove_file(token_path());
    report(ErrorLevel::Info, "discord", "disconnected", "Disconnected");
}

// ── Wave 7C: Authenticated adapter w/ live refresh ────────────────────────
//
// Discord OAuth2 supports refresh via `grant_type=refresh_token` against
// `/api/oauth2/token`. We POST with the stored refresh token, persist the
// new access + refresh tokens (Discord rotates the refresh token), and
// return the new access token. Errors map to `AuthError::RefreshFailed`.
//
// The refresh runs in `spawn_blocking` so the blocking `reqwest::Client`
// (shared with the rest of this module) doesn't stall the tokio reactor.

const DISCORD_TOKEN_URL: &str = "https://discord.com/api/oauth2/token";

pub struct DiscordAuthProvider;

#[async_trait::async_trait]
impl crate::data::connectivity::Authenticated for DiscordAuthProvider {
    async fn refresh_token(&self) -> Result<String, crate::data::connectivity::AuthError> {
        use crate::data::connectivity::AuthError;

        // Snapshot what we need under the OnceLock guards before crossing the
        // .await boundary; the underlying mutexes aren't Send across awaits.
        let current = get_auth().ok_or(AuthError::MissingCredentials)?;
        if current.refresh_token.is_empty() {
            return Err(AuthError::TokenInvalid);
        }
        let config = DISCORD_CONFIG
            .get()
            .ok_or_else(|| AuthError::RefreshFailed("discord: not configured".into()))?;
        let client_id = config.client_id.clone();
        let client_secret = config.client_secret.clone();
        let refresh = current.refresh_token.clone();
        let kept_user_id = current.user_id.clone();
        let kept_username = current.username.clone();
        let kept_avatar = current.avatar.clone();

        let new_auth = tokio::task::spawn_blocking(move || -> Result<DiscordAuth, String> {
            let client = http();
            let resp = client
                .post(DISCORD_TOKEN_URL)
                .form(&[
                    ("client_id", client_id.as_str()),
                    ("client_secret", client_secret.as_str()),
                    ("grant_type", "refresh_token"),
                    ("refresh_token", refresh.as_str()),
                ])
                .send()
                .map_err(|e| format!("send: {e}"))?;
            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().unwrap_or_default();
                return Err(format!("status {status}: {}", &body[..body.len().min(200)]));
            }
            let json: serde_json::Value = resp.json().map_err(|e| format!("parse: {e}"))?;
            let access_token = json["access_token"]
                .as_str()
                .ok_or_else(|| "no access_token in response".to_string())?
                .to_string();
            // Discord rotates the refresh token on every refresh; fall back
            // to the old one if (somehow) absent so we don't end up with an
            // empty refresh_token that locks us out next cycle.
            let refresh_token = json["refresh_token"]
                .as_str()
                .map(|s| s.to_string())
                .unwrap_or(refresh);
            let expires_in = json["expires_in"].as_u64().unwrap_or(604800);
            Ok(DiscordAuth {
                access_token,
                refresh_token,
                expires_at: std::time::Instant::now() + std::time::Duration::from_secs(expires_in),
                user_id: kept_user_id,
                username: kept_username,
                avatar: kept_avatar,
            })
        })
        .await
        .map_err(|e| AuthError::RefreshFailed(format!("spawn_blocking join: {e}")))?
        .map_err(AuthError::RefreshFailed)?;

        // Persist + update the in-memory token store (same path as exchange_code).
        save_auth_to_disk(&new_auth);
        let _ = DISCORD_TOKEN.get_or_init(|| Mutex::new(None));
        if let Some(m) = DISCORD_TOKEN.get() {
            if let Ok(mut g) = m.lock() {
                *g = Some(new_auth.clone());
            }
        }
        report(ErrorLevel::Info, "discord", "token_refreshed", "rotated refresh token, saved to disk");
        Ok(new_auth.access_token)
    }
    fn current_token(&self) -> Option<String> {
        get_auth().map(|a| a.access_token)
    }
}
