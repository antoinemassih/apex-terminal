//! Discord integration for Apex Terminal
//! OAuth2 for user identity + Bot API for channel/message access.
//!
//! discord.env format:
//!   DISCORD_CLIENT_ID=...
//!   DISCORD_CLIENT_SECRET=...
//!   DISCORD_BOT_TOKEN=...     (enables channels, messages, sending)

use std::sync::{Mutex, OnceLock};

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
        eprintln!("[discord] Saved token expired");
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
            eprintln!("[discord] Config loaded (bot: {})", has_bot);
        }
        // Restore saved auth token from disk
        if let Some(auth) = load_auth_from_disk() {
            eprintln!("[discord] Restored session for {} ({})", auth.username, auth.user_id);
            let _ = DISCORD_TOKEN.get_or_init(|| Mutex::new(None));
            if let Some(m) = DISCORD_TOKEN.get() {
                *m.lock().unwrap() = Some(auth);
            }
        }
    } else {
        eprintln!("[discord] No discord.env found — Discord integration disabled");
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

// ── OAuth2 flow ─────────────────────────────────────────────────────────────

pub fn start_oauth2() {
    let config = match DISCORD_CONFIG.get() {
        Some(c) => c,
        None => { eprintln!("[discord] Not configured"); return; }
    };

    let auth_url = format!(
        "https://discord.com/oauth2/authorize?client_id={}&redirect_uri={}&response_type=code&scope={}",
        config.client_id,
        urlencoding::encode(REDIRECT_URI),
        urlencoding::encode(SCOPES),
    );

    eprintln!("[discord] Opening browser for OAuth2...");
    let _ = open::that(&auth_url);

    std::thread::spawn(move || { start_callback_server(); });
}

fn start_callback_server() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = match TcpListener::bind(format!("127.0.0.1:{}", REDIRECT_PORT)) {
        Ok(l) => l,
        Err(e) => { eprintln!("[discord] Failed to bind: {}", e); return; }
    };
    listener.set_nonblocking(false).ok();
    eprintln!("[discord] Callback server listening on port {}", REDIRECT_PORT);

    if let Ok((mut stream, _)) = listener.accept() {
        let mut buf = [0u8; 4096];
        let n = stream.read(&mut buf).unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..n]);

        if let Some(code) = extract_code(&request) {
            eprintln!("[discord] Got auth code: {}...", &code[..code.len().min(10)]);
            match exchange_code(&code) {
                Ok(auth) => {
                    eprintln!("[discord] Authenticated as {} ({})", auth.username, auth.user_id);
                    save_auth_to_disk(&auth);
                    let _ = DISCORD_TOKEN.get_or_init(|| Mutex::new(None));
                    if let Some(m) = DISCORD_TOKEN.get() {
                        *m.lock().unwrap() = Some(auth);
                    }
                    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html><body style='background:#1a1a2e;color:#eee;font-family:monospace;text-align:center;padding:60px'><h1>Connected to Apex Terminal</h1><p>You can close this tab.</p><script>setTimeout(()=>window.close(),2000)</script></body></html>";
                    let _ = stream.write_all(response.as_bytes());
                }
                Err(e) => {
                    eprintln!("[discord] Token exchange failed: {}", e);
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

fn extract_code(request: &str) -> Option<String> {
    let first_line = request.lines().next()?;
    let url_part = first_line.split_whitespace().nth(1)?;
    let query = url_part.split('?').nth(1)?;
    for param in query.split('&') {
        if let Some(code) = param.strip_prefix("code=") {
            return Some(code.to_string());
        }
    }
    None
}

fn exchange_code(code: &str) -> Result<DiscordAuth, String> {
    let config = DISCORD_CONFIG.get().ok_or("Not configured")?;
    let client = http();

    let resp = client.post(format!("{}/oauth2/token", DISCORD_API))
        .form(&[
            ("client_id", config.client_id.as_str()),
            ("client_secret", config.client_secret.as_str()),
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", REDIRECT_URI),
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
            eprintln!("[discord] Channel fetch {}: {}", status, body);
            vec![]
        }
        Err(e) => { eprintln!("[discord] Channel fetch error: {}", e); vec![] }
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
            eprintln!("[discord] Message fetch {}: {}", status, &body[..body.len().min(200)]);
            vec![]
        }
        Err(e) => { eprintln!("[discord] Message fetch error: {}", e); vec![] }
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
        eprintln!("[discord] Fetched {} guilds", guilds.len());
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
        eprintln!("[discord] Fetched {} channels for {}", channels.len(), guild_id);
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
    eprintln!("[discord] Disconnected");
}
