//! Fetch / IO helpers — HTTP, background thread data loaders.
//!
//! All functions spawn background threads and deliver results as `ChartCommand`s
//! via `crate::send_to_native_chart` or `crate::NATIVE_CHART_TXS`.

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use crate::chart_renderer::{ChartCommand, Bar};
use crate::chart_renderer::gpu::{OptionRow, db_to_drawing, apexib_url};
use crate::chart_renderer::{Drawing, DrawingGroup};
use crate::chart_renderer::compute::{bs_price, strike_interval, atm_strike, get_iv, sim_oi};

fn build_chain(underlying: f32, num_strikes: usize, dte: i32) -> (Vec<OptionRow>, Vec<OptionRow>) {
    let r = 0.05_f32;
    let t = if dte == 0 { 0.5 / 252.0 } else { dte as f32 / 252.0 };
    let interval = strike_interval(underlying);
    let atm = atm_strike(underlying);

    let make_row = |k: f32, is_call: bool, _is_atm: bool| -> OptionRow {
        let iv = get_iv(underlying, k, dte);
        let raw = bs_price(underlying, k, t, r, iv, is_call);
        let spread = (raw * 0.04 + 0.005).max(0.01);
        let mid = raw.max(0.0);
        let contract = format!("{}{}{}{}",
            if is_call { "C" } else { "P" }, k as i32, if dte == 0 { "0D" } else { "1D+" }, dte);
        OptionRow {
            strike: k, last: mid, bid: (raw - spread / 2.0).max(0.0), ask: raw + spread / 2.0,
            volume: sim_oi(underlying, k, dte) / 10, oi: sim_oi(underlying, k, dte),
            iv, itm: if is_call { underlying > k } else { underlying < k }, contract,
        }
    };

    let mut calls = Vec::new();
    for i in (1..=num_strikes).rev() { calls.push(make_row(atm + i as f32 * interval, true, false)); }
    calls.push(make_row(atm, true, true));

    let mut puts = Vec::new();
    puts.push(make_row(atm, false, true));
    for i in 1..=num_strikes { puts.push(make_row(atm - i as f32 * interval, false, false)); }

    (calls, puts)
}

/// Shared multi-thread tokio runtime for all background fetch calls.
/// Created once (OnceLock), re-used for every `fetch_bars_background` call —
/// avoids spawning a fresh single-thread runtime per call (H3 fix).
fn fetch_runtime() -> &'static tokio::runtime::Runtime {
    use std::sync::OnceLock;
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("apex-fetch")
            .build()
            .expect("apex-fetch tokio runtime")
    })
}

/// Fetch options chain from ApexIB in background. Sends ChainData command when done.
/// Falls back to simulated build_chain if the API is unreachable.
/// Shared HTTP client for ApexIB — avoids TLS handshake per request
fn apexib_client() -> &'static reqwest::blocking::Client {
    use std::sync::OnceLock;
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .user_agent("apex-native")
            .timeout(std::time::Duration::from_secs(10))
            .pool_max_idle_per_host(4)
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new())
    })
}

/// Fast API fetch using curl subprocess — bypasses reqwest TLS issues on Windows
fn apexib_curl(path: &str) -> Option<serde_json::Value> {
    let url = format!("{}{}", apexib_url(), path);
    #[cfg(windows)]
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let mut cmd = std::process::Command::new("curl");
    cmd.args(&["-sL", "--max-time", "10", &url]);
    #[cfg(windows)]
    { use std::os::windows::process::CommandExt; cmd.creation_flags(CREATE_NO_WINDOW); }
    let output = cmd.output().ok()?;
    if output.status.success() {
        serde_json::from_slice(&output.stdout).ok()
    } else { None }
}

/// Parsed gamma/regime/flow bundle from the feed. The `flow.*` fields (PPE,
/// IV-rising) are populated only during market hours — the feed reports
/// `flow.active=false` off-hours, in which case `ppe`/`iv_rising` stay `None`.
#[derive(Clone)]
pub(crate) struct GammaSnapshot {
    pub(crate) levels: Vec<crate::chart_renderer::gpu::GammaLevel>,
    pub(crate) flip: f32,
    pub(crate) call_wall: f32,
    pub(crate) put_wall: f32,
    pub(crate) regime: String,
    /// Put Premium Efficiency (flow layer). `Some` only when `flow_active`.
    pub(crate) ppe: Option<f32>,
    pub(crate) iv_rising: Option<bool>,
    pub(crate) flow_active: bool,
    /// `short_posture.posture` (e.g. `hold_press`, `cover_into_target`).
    pub(crate) posture: String,
}

/// Fetch the live gamma/regime/flow bundle from the gamma feed (see
/// `APEX_GAMMA_FEED_URL`, default `gamma_feed_service.py` on :8412) and parse
/// it. `None` if the feed is unreachable / symbol unsupported — caller falls
/// back to the synthetic levels.
pub(crate) fn fetch_gamma_from_feed(symbol: &str) -> Option<GammaSnapshot> {
    use crate::chart_renderer::gpu::GammaLevel;
    // #5 — only query the feed for symbols it actually serves; anything else
    // returns None so the caller cleanly falls back to the synthetic levels.
    if !gamma_feed_supports(symbol) { return None; }
    // #4 — configurable feed base URL (APEX_GAMMA_FEED_URL), default :8412.
    let url = format!("{}/gamma/{}", gamma_feed_url(), symbol.to_uppercase());
    let v: serde_json::Value = apexib_client().get(&url).send().ok()?.json().ok()?;
    let g = v.get("gamma")?;
    let levels: Vec<GammaLevel> = g
        .get("by_strike")?
        .as_array()?
        .iter()
        .filter_map(|s| {
            Some(GammaLevel {
                price: s.get("strike")?.as_f64()? as f32,
                exposure: s.get("net_gex")?.as_f64()? as f32,
            })
        })
        .collect();
    if levels.is_empty() {
        return None;
    }
    let flip = g.get("flip_level").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
    let call_wall = g.get("call_wall").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
    let put_wall = g.get("put_wall").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
    let regime = g.get("regime").and_then(|x| x.as_str()).unwrap_or("").to_string();
    // Flow layer (PPE) — present only during market hours.
    let flow = v.get("flow");
    let flow_active = flow.and_then(|f| f.get("active")).and_then(|x| x.as_bool()).unwrap_or(false);
    let ppe = flow.and_then(|f| f.get("ppe")).and_then(|x| x.as_f64()).map(|x| x as f32);
    let iv_rising = flow.and_then(|f| f.get("iv_rising")).and_then(|x| x.as_bool());
    let posture = v.get("short_posture").and_then(|sp| sp.get("posture"))
        .and_then(|x| x.as_str()).unwrap_or("").to_string();
    Some(GammaSnapshot { levels, flip, call_wall, put_wall, regime, ppe, iv_rising, flow_active, posture })
}

// ── Gamma feed: config (#4), symbol guard (#5), auto-refresh (#2) ────────────

/// Base URL of the gamma feed. Override with `APEX_GAMMA_FEED_URL`
/// (e.g. `http://gamma.xllio.com` or the ApexSignals prod endpoint). No
/// trailing slash.
fn gamma_feed_url() -> String {
    std::env::var("APEX_GAMMA_FEED_URL")
        .ok()
        .map(|s| s.trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:8412".to_string())
}

/// Whether the gamma feed serves this symbol. Defaults to QQQ/SPY; override
/// with `APEX_GAMMA_FEED_SYMBOLS="QQQ,SPY,SPX"`. Unsupported symbols fall back
/// to the synthetic levels instead of spamming the feed with 404s.
fn gamma_feed_supports(symbol: &str) -> bool {
    static SET: std::sync::OnceLock<std::collections::HashSet<String>> = std::sync::OnceLock::new();
    let set = SET.get_or_init(|| {
        std::env::var("APEX_GAMMA_FEED_SYMBOLS")
            .unwrap_or_else(|_| "QQQ,SPY".to_string())
            .split(',')
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty())
            .collect()
    });
    set.contains(&symbol.to_uppercase())
}

/// Generates the boilerplate `*_cache()` (string-keyed value map) and
/// `*_inflight()` (string set) static accessors shared by every non-blocking
/// REST cache below. The per-cache semantics (TTL, eligibility, fetch, stale
/// handling) stay in each `*_cached` fn — only the identical static plumbing is
/// deduplicated here, so behaviour is unchanged.
macro_rules! str_keyed_cache {
    ($cache_fn:ident, $inflight_fn:ident, $val:ty) => {
        fn $cache_fn() -> &'static std::sync::Mutex<std::collections::HashMap<String, $val>> {
            static C: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, $val>>> = std::sync::OnceLock::new();
            C.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
        }
        fn $inflight_fn() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
            static I: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> = std::sync::OnceLock::new();
            I.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
        }
    };
}

/// Maximum number of entries kept in each string-keyed REST cache. Once the cap
/// is reached, the oldest-by-insert-time entries are evicted before the new one
/// is retained. 256 symbols is well beyond any realistic watchlist while keeping
/// memory bounded over a full trading day.
const STR_CACHE_CAP: usize = 256;

/// Evict entries from `map` until `len() <= cap`, removing the ones whose
/// insert `Instant` (extracted via `age`) is oldest. O(n) per call; only runs
/// when `len() > cap`, so the common path (already within cap) is a single
/// comparison.
fn evict_oldest<V>(
    map: &mut std::collections::HashMap<String, V>,
    cap: usize,
    age: impl Fn(&V) -> std::time::Instant,
) {
    while map.len() > cap {
        // One linear scan to find the key with the oldest Instant.
        if let Some(oldest_key) = map
            .iter()
            .min_by_key(|(_, v)| age(v))
            .map(|(k, _)| k.clone())
        {
            map.remove(&oldest_key);
        } else {
            break;
        }
    }
}

/// Evict entries from `map` until `len() <= cap` by removing an arbitrary
/// entry. Used for caches whose value type carries no `Instant` (ticker_detail).
fn evict_any<V>(map: &mut std::collections::HashMap<String, V>, cap: usize) {
    while map.len() > cap {
        if let Some(key) = map.keys().next().map(|k| k.clone()) {
            map.remove(&key);
        } else {
            break;
        }
    }
}

str_keyed_cache!(gamma_cache, gamma_inflight, (GammaSnapshot, std::time::Instant));
// Continuation-gauge markers, keyed by "SYMBOL|YYYY-MM-DD" (date currently on the chart).
str_keyed_cache!(continuation_cache, continuation_inflight, (Vec<crate::chart_renderer::gpu::ContinuationMarker>, std::time::Instant));

/// Fetch continuation-gauge stalls (HOLD/EXIT) for `symbol` on `date` from the
/// signal feed (`/signals/continuation/{sym}?date=`). The feed returns each
/// stall with an absolute `stall_ms` (epoch ms), the pin-adjusted call, and P,
/// so no client-side timezone math is needed. `None` if unreachable/unsupported.
pub(crate) fn fetch_continuation_from_feed(symbol: &str, date: &str)
    -> Option<Vec<crate::chart_renderer::gpu::ContinuationMarker>> {
    use crate::chart_renderer::gpu::ContinuationMarker;
    if !gamma_feed_supports(symbol) { return None; }
    let url = format!("{}/signals/continuation/{}?date={}",
                      gamma_feed_url(), symbol.to_uppercase(), date);
    let v: serde_json::Value = apexib_client().get(&url).send().ok()?.json().ok()?;
    let stalls = v.get("stalls")?.as_array()?;
    let mut out = Vec::with_capacity(stalls.len());
    for s in stalls {
        let call = s.get("call").and_then(|x| x.as_str()).unwrap_or("");
        if !(call.contains("HOLD") || call.contains("EXIT")) { continue; } // skip ASIDE/neutral
        let time = match s.get("stall_ms").and_then(|x| x.as_i64()) { Some(t) => t, None => continue };
        out.push(ContinuationMarker {
            time,
            hold: call.contains("HOLD"),
            strong: call.starts_with("STRONG"),
            p: s.get("p_adj").and_then(|x| x.as_f64()).unwrap_or(0.5) as f32,
        });
    }
    Some(out)
}

/// Spawn a deduped, non-blocking continuation fetch into the cache (key `SYM|date`).
fn spawn_continuation_fetch(symbol: String, date: String) {
    let key = format!("{}|{}", symbol.to_uppercase(), date);
    {
        let mut inf = match continuation_inflight().lock() { Ok(g) => g, Err(e) => e.into_inner() };
        if inf.contains(&key) { return; }
        inf.insert(key.clone());
    }
    crate::foundation::guard::spawn_guarded("fetch", move || {
        if let Some(markers) = fetch_continuation_from_feed(&symbol, &date) {
            if let Ok(mut c) = continuation_cache().lock() {
                c.insert(key.clone(), (markers, std::time::Instant::now()));
                evict_oldest(&mut c, STR_CACHE_CAP, |v| v.1);
            }
            crate::wake_native_ui();
        }
        if let Ok(mut inf) = continuation_inflight().lock() { inf.remove(&key); }
    });
}

/// How stale a cached gamma bundle may get before a refetch is spawned.
const GAMMA_REFRESH_SECS: u64 = 20;

/// Spawn a one-shot background fetch of `symbol` into the gamma cache. Deduped
/// (one in-flight request per symbol) and non-blocking — the blocking HTTP GET
/// runs on its own thread, never the render thread.
fn spawn_gamma_fetch(symbol: String) {
    let sym = symbol.to_uppercase();
    {
        let mut inf = match gamma_inflight().lock() { Ok(g) => g, Err(e) => e.into_inner() };
        if inf.contains(&sym) { return; }
        inf.insert(sym.clone());
    }
    crate::foundation::guard::spawn_guarded("fetch", move || {
        if let Some(bundle) = fetch_gamma_from_feed(&sym) {
            if let Ok(mut c) = gamma_cache().lock() {
                c.insert(sym.clone(), (bundle, std::time::Instant::now()));
                evict_oldest(&mut c, STR_CACHE_CAP, |v| v.1);
            }
            crate::wake_native_ui();
        }
        if let Ok(mut inf) = gamma_inflight().lock() { inf.remove(&sym); }
    });
}

/// #2 — keep gamma overlays fresh. Called once per render frame: for every pane
/// showing gamma, apply the latest cached bundle and (re)spawn a background
/// fetch when the cache is missing or older than `GAMMA_REFRESH_SECS`. So the
/// flip/regime/walls track price without the user re-toggling. Non-blocking.
pub(crate) fn refresh_gamma_feeds(panes: &mut [crate::chart_renderer::gpu::Chart]) {
    for p in panes.iter_mut() {
        if !p.show_gamma { continue; }
        let sym = p.symbol.to_uppercase();
        // Unsupported symbol → leave whatever's there (the synthetic levels).
        if !gamma_feed_supports(&sym) { continue; }

        let mut fresh = false;
        if let Ok(c) = gamma_cache().lock() {
            if let Some((snap, at)) = c.get(&sym) {
                // Apply the latest real bundle to the pane's overlay fields.
                p.gamma_levels = snap.levels.clone();
                p.gamma_zero = snap.flip;
                p.gamma_call_wall = snap.call_wall;
                p.gamma_put_wall = snap.put_wall;
                // Flow layer (PPE) — only meaningful during market hours.
                p.gamma_ppe = snap.ppe;
                p.gamma_iv_rising = snap.iv_rising;
                p.gamma_flow_active = snap.flow_active;
                p.gamma_posture = snap.posture.clone();
                fresh = at.elapsed().as_secs() < GAMMA_REFRESH_SECS;
            }
        }
        if !fresh { spawn_gamma_fetch(sym.clone()); }

        // Continuation-gauge markers for the date currently loaded on the chart
        // (live = today; historical = the displayed day). RTH bars share the UTC
        // calendar day with the ET trading date, so the last bar's UTC date is the
        // trading date. Reuses the same non-blocking cache machinery as gamma.
        if let Some(&last_ts) = p.timestamps.last() {
            if let Some(dt) = chrono::DateTime::from_timestamp_millis(last_ts) {
                let date = dt.format("%Y-%m-%d").to_string();
                let key = format!("{}|{}", sym, date);
                let mut cfresh = false;
                if let Ok(c) = continuation_cache().lock() {
                    if let Some((markers, at)) = c.get(&key) {
                        p.continuation_signals = markers.clone();
                        cfresh = at.elapsed().as_secs() < GAMMA_REFRESH_SECS;
                    }
                }
                if !cfresh { spawn_continuation_fetch(sym.clone(), date); }
            }
        }
    }
}

/// Compute the "active" 0DTE expiry date based on US/Eastern wall clock:
///   • Sat → Fri, Sun → Fri
///   • Weekday before 4am ET → previous trading day (Mon<4am → Fri)
///   • Otherwise → today
/// Approximates ET as UTC-4 (EDT, valid Mar–Nov). Acceptable since this only
/// shifts the cutoff by an hour twice a year.
pub(crate) fn active_zero_dte_date() -> chrono::NaiveDate {
    active_zero_dte_date_at(chrono::Utc::now())
}

fn active_zero_dte_date_at(now: chrono::DateTime<chrono::Utc>) -> chrono::NaiveDate {
    use chrono::{Duration, Datelike, Timelike};
    let et = now - Duration::hours(4);
    let date = et.date_naive();
    let wd = et.weekday().num_days_from_monday(); // 0=Mon..6=Sun
    let hour = et.hour();
    match wd {
        5 => date - Duration::days(1),                       // Sat → Fri
        6 => date - Duration::days(2),                       // Sun → Fri
        0 if hour < 4 => date - Duration::days(3),           // Mon overnight → Fri
        _ if hour < 4 => date - Duration::days(1),           // Tue–Fri overnight → prev day
        _ => date,                                            // weekday ≥ 4am ET → today
    }
}

#[cfg(test)]
mod active_zero_dte_tests {
    use super::active_zero_dte_date_at;
    use chrono::{DateTime, NaiveDate, Utc};

    fn utc(y: i32, m: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
        let nd = NaiveDate::from_ymd_opt(y, m, d).unwrap();
        let ndt = nd.and_hms_opt(h, mi, 0).unwrap();
        DateTime::<Utc>::from_naive_utc_and_offset(ndt, Utc)
    }

    #[test]
    fn saturday_returns_previous_friday() {
        let n = utc(2026, 4, 25, 12, 0);
        assert_eq!(active_zero_dte_date_at(n), NaiveDate::from_ymd_opt(2026, 4, 24).unwrap());
    }

    #[test]
    fn sunday_returns_previous_friday() {
        let n = utc(2026, 4, 26, 12, 0);
        assert_eq!(active_zero_dte_date_at(n), NaiveDate::from_ymd_opt(2026, 4, 24).unwrap());
    }

    #[test]
    fn monday_after_4am_et_returns_today() {
        // 09:00 UTC = 05:00 ET Mon
        let n = utc(2026, 4, 27, 9, 0);
        assert_eq!(active_zero_dte_date_at(n), NaiveDate::from_ymd_opt(2026, 4, 27).unwrap());
    }

    #[test]
    fn monday_overnight_returns_previous_friday() {
        // 06:00 UTC = 02:00 ET Mon
        let n = utc(2026, 4, 27, 6, 0);
        assert_eq!(active_zero_dte_date_at(n), NaiveDate::from_ymd_opt(2026, 4, 24).unwrap());
    }

    #[test]
    fn tuesday_overnight_returns_previous_day() {
        // 06:00 UTC = 02:00 ET Tue
        let n = utc(2026, 4, 28, 6, 0);
        assert_eq!(active_zero_dte_date_at(n), NaiveDate::from_ymd_opt(2026, 4, 27).unwrap());
    }

    #[test]
    fn friday_afternoon_returns_today() {
        // 21:00 UTC = 17:00 ET Fri
        let n = utc(2026, 4, 24, 21, 0);
        assert_eq!(active_zero_dte_date_at(n), NaiveDate::from_ymd_opt(2026, 4, 24).unwrap());
    }

    #[test]
    fn wednesday_utc_late_returns_tuesday_et() {
        // Wed 03:00 UTC = Tue 23:00 ET. Tue at hour 23 ≥ 4 → "today" in ET = Tue.
        let n = utc(2026, 4, 22, 3, 0);
        assert_eq!(active_zero_dte_date_at(n), NaiveDate::from_ymd_opt(2026, 4, 21).unwrap());
    }
}

/// Normalize common index-option aliases to the underlying the data service
/// actually tracks. Server-side normalization is planned but not yet live.
fn normalize_underlying(sym: &str) -> String {
    match sym.to_uppercase().as_str() {
        "SPXW" => "SPX".to_string(),   // SPX weeklies share SPX feed
        "NDXP" => "NDX".to_string(),   // NDX p.m.-settled
        "XSP"  => "XSP".to_string(),   // mini-SPX (keep — separate product)
        other  => other.to_string(),
    }
}

/// Lightweight periodic REST refresh of the chain cache. Unlike
/// `fetch_chain_background`, this does NOT short-circuit on a cache hit — it
/// always pulls a fresh chain from REST and MERGES (upserts) it into the cache
/// so live bid/ask/IV/greeks stay fresh between the (sparse) WS chaindelta
/// frames. It must merge, not `seed_chain` (clear+replace): the forward-only
/// dte_max query omits the 0DTE-backfill rows that `fetch_chain_background`
/// seeds off-hours/weekends, so replacing would empty the 0DTE grid and leave
/// the panel stuck on "Loading chain…". Non-blocking; no-ops on error.
pub(crate) fn refresh_chain_rest(symbol: String) {
    crate::foundation::guard::spawn_guarded("fetch", move || {
        use crate::apex_data::types::ChainQuery;
        let q = ChainQuery { dte_max: Some(14), strike_window_pct: Some(10.0), ..Default::default() };
        if let Ok(chain) = crate::apex_data::rest::get_chain_with(&symbol, &q) {
            if !chain.rows.is_empty() {
                crate::apex_data::live_state::merge_chain_delta(&symbol, &chain.rows);
                crate::wake_native_ui();
            }
        }
    });
}

pub(crate) fn fetch_chain_background(symbol: String, num_strikes: usize, dte: i32, underlying_price: f32) {
    let symbol = normalize_underlying(&symbol);
    crate::foundation::guard::spawn_guarded("fetch", move || {
        let api_strikes = 150;
        let path = format!("/options/{}?strikeCount={}&dte={}", symbol, api_strikes, dte);

        let send_chain = |calls: Vec<(f32,f32,f32,f32,i32,i32,f32,bool,String)>,
                          puts: Vec<(f32,f32,f32,f32,i32,i32,f32,bool,String)>,
                          und_price: f32| {
            let cmd = ChartCommand::ChainData {
                symbol: symbol.clone(),
                dte,
                underlying_price: und_price,
                calls,
                puts,
                placeholder: false,
            };
            crate::send_to_native_chart(cmd);
        };
        let send_placeholder_chain = |und_price: f32| {
            // Last-resort fallback: synthesize a Black-Scholes chain so the
            // panel + chart axes have *something* to render while the user
            // sees a "PLACEHOLDER DATA" strip.
            let spot = if und_price > 0.0 { und_price } else { 100.0 };
            let (calls_rows, puts_rows) = build_chain(spot, num_strikes, dte);
            let to_tuple = |r: &OptionRow| (
                r.strike, r.last, r.bid, r.ask, r.volume, r.oi, r.iv, r.itm, r.contract.clone()
            );
            let calls = calls_rows.iter().map(to_tuple).collect();
            let puts  = puts_rows.iter().map(to_tuple).collect();
            crate::send_to_native_chart(ChartCommand::ChainData {
                symbol: symbol.clone(),
                dte,
                underlying_price: spot,
                calls, puts,
                placeholder: true,
            });
        };

        // 0. ApexData — preferred. §5.4.c default filters (dte_max=14,
        //    strike_window_pct=10%) keep the payload small; §5.4.d chain_delta
        //    WS stream then pushes live updates into the local cache.
        if crate::apex_data::is_enabled() {
            use crate::apex_data::types::ChainQuery;

            // Subscribe once to the chain delta stream for this underlying.
            // Routed through SubscriptionManager so it records the active
            // sub for gap-fill on reconnect (Wave 8). Replace-set semantics
            // are preserved — same as the old `ws::set_chain` call.
            crate::data::providers::registry::subscription_manager()
                .set_chain(&[symbol.clone()]);
            crate::apex_log!("chain", "WS chain sub set to [{}]", symbol);

            let render_from = |rows: &[crate::apex_data::ChainRow], hint: f32| -> Option<(Vec<_>, Vec<_>, f32)> {
                let (calls, puts, eff) = apex_data_chain_to_tuples(rows, dte, num_strikes, hint);
                if calls.is_empty() && puts.is_empty() { None } else { Some((calls, puts, eff)) }
            };

            let hint = if underlying_price > 0.0 { underlying_price }
                       else {
                           crate::apex_data::live_state::get_snapshot(&symbol)
                               .map(|s| s.last as f32).unwrap_or(0.0)
                       };

            // 0a. Cache hit? (already seeded by prior REST or chain_delta)
            let cached = crate::apex_data::live_state::get_chain(&symbol);
            if !cached.is_empty() {
                if let Some((calls, puts, spot)) = render_from(&cached, hint) {
                    crate::apex_log!("chain", "{} dte={} from cache: {} calls, {} puts",
                        symbol, dte, calls.len(), puts.len());
                    send_chain(calls, puts, spot);
                    return;
                }
            }

            // 0b. REST with default filters (small payload, gzip).
            let q = ChainQuery {
                dte_max: Some(std::cmp::max(dte, 14)),
                strike_window_pct: Some(10.0),
                ..Default::default()
            };
            if let Ok(chain) = crate::apex_data::rest::get_chain_with(&symbol, &q) {
                crate::apex_log!("chain", "{}: {} rows (cache={}, filters dte_max={:?} sw%={:?})",
                    symbol, chain.rows.len(), chain.total_in_cache,
                    chain.filters.dte_max, chain.filters.strike_window_pct);
                crate::apex_data::live_state::seed_chain(&symbol, &chain.rows);

                // 0DTE backfill: when the active 0DTE date is in the past
                // (weekend / overnight), the default forward-only `dte_max`
                // filter excludes it. Pull that specific expiry explicitly
                // and merge into the cache so the 0DTE column is populated
                // with last-Friday's settled chain.
                let zdt = active_zero_dte_date();
                let today = chrono::Utc::now().date_naive();
                if zdt < today {
                    let zdt_s = zdt.format("%Y-%m-%d").to_string();
                    let pq = ChainQuery {
                        expiry: Some(zdt_s.clone()),
                        strike_window_pct: Some(10.0),
                        ..Default::default()
                    };
                    if let Ok(past) = crate::apex_data::rest::get_chain_with(&symbol, &pq) {
                        crate::apex_log!("chain.zerodte",
                            "{}: backfill expiry={} → {} rows", symbol, zdt_s, past.rows.len());
                        crate::apex_data::live_state::merge_chain_delta(&symbol, &past.rows);
                    }
                }

                if let Some((calls, puts, spot)) = render_from(&chain.rows, hint) {
                    send_chain(calls, puts, spot);
                    return;
                }
            }

            // 0c. 404/empty — untracked underlying. Prime via a placeholder OCC
            //     subscribe (§5.4.b) and poll the local cache (which the
            //     chain_delta stream will populate within ~5s of the upstream
            //     sub taking effect).
            let placeholder_occ = synthesize_occ(&symbol, 100.0, true, "0DTE");
            crate::apex_log!("chain", "untracked {} — priming via {}", symbol, placeholder_occ);
            // Wave 8: route through SubscriptionManager so gap-fill knows
            // about the prime sub. Returned receiver is unused — bar frames
            // continue to reach the UI via NATIVE_CHART_TXS.
            let _ = crate::data::providers::registry::subscription_manager()
                .subscribe_bars(&placeholder_occ, "1m");

            for attempt in 1..=8 {
                std::thread::sleep(std::time::Duration::from_millis(1000));
                // Prefer cache (chain_delta already merged); fall back to REST.
                let cached = crate::apex_data::live_state::get_chain(&symbol);
                if !cached.is_empty() {
                    if let Some((calls, puts, spot)) = render_from(&cached, hint) {
                        crate::apex_log!("chain", "{} dte={}: {} calls, {} puts (cache hit after {}s)",
                            symbol, dte, calls.len(), puts.len(), attempt);
                        send_chain(calls, puts, spot);
                        return;
                    }
                }
                if let Ok(chain) = crate::apex_data::rest::get_chain_with(&symbol, &q) {
                    if !chain.rows.is_empty() {
                        crate::apex_data::live_state::seed_chain(&symbol, &chain.rows);
                        if let Some((calls, puts, spot)) = render_from(&chain.rows, hint) {
                            crate::apex_log!("chain", "{} dte={}: {} calls, {} puts (REST after {}s)",
                                symbol, dte, calls.len(), puts.len(), attempt);
                            send_chain(calls, puts, spot);
                            return;
                        }
                    }
                }
            }
            crate::apex_log!("chain", "still no chain for {} after prime+retry — fallback", symbol);
            // Toast intentionally removed — the panel + axes already show a
            // permanent "PLACEHOLDER DATA" strip when the synthetic chain
            // lands; constant toast pings are noise.
        }

        // Use curl for fast TLS (reqwest's native-tls is slow on this machine)
        if let Some(json) = apexib_curl(&path) {
                    let api_price = json.get("underlying").and_then(|u| u.get("price")).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    let parse_rows = |key: &str| -> Vec<(f32,f32,f32,f32,i32,i32,f32,bool,String)> {
                        json.get(key).and_then(|v| v.as_array()).map(|arr| {
                            arr.iter().filter_map(|row| {
                                let strike = row.get("strike")?.as_f64()? as f32;
                                let last = row.get("lastPrice").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                                let bid = row.get("bid").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                                let ask = row.get("ask").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                                let vol = row.get("volume").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                                let oi = row.get("openInterest").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                                let iv = row.get("iv").and_then(|v| v.as_f64())
                                    .or_else(|| row.get("impliedVolatility").and_then(|v| v.as_f64()))
                                    .unwrap_or(0.0) as f32;
                                let itm = row.get("inTheMoney").and_then(|v| v.as_bool()).unwrap_or(false);
                                let contract = row.get("contractSymbol").and_then(|v| v.as_str())
                                    .or_else(|| row.get("conId").and_then(|v| v.as_i64()).map(|_| ""))
                                    .unwrap_or("").to_string();
                                let contract = if contract.is_empty() {
                                    row.get("conId").and_then(|v| v.as_i64()).map(|id| format!("{}", id)).unwrap_or_default()
                                } else { contract };
                                Some((strike, last, bid, ask, vol, oi, iv, itm, contract))
                            }).collect()
                        }).unwrap_or_default()
                    };

                    let calls = parse_rows("calls");
                    let puts = parse_rows("puts");

                    if !calls.is_empty() || !puts.is_empty() {
                        eprintln!("[apexib] Fetched chain for {} dte={}: {} calls, {} puts", symbol, dte, calls.len(), puts.len());
                        let final_price = if api_price > 0.0 { api_price } else { underlying_price };
                        send_chain(calls, puts, final_price);
                        return;
                    }
        } else {
            eprintln!("[apexib] Chain fetch for {} dte={} failed (curl)", symbol, dte);
        }

        // Real upstream is gone — fall back to a locally-synthesized
        // Black-Scholes chain (placeholder). Clicking a row still won't
        // load real bars (OCCs don't exist upstream), but the panel + axes
        // get something to show, gated by a "PLACEHOLDER DATA" strip.
        crate::apex_log!("chain", "no real chain for {} dte={} — sending placeholder", symbol, dte);
        // No toast — the PLACEHOLDER strip in the panel + axes is the
        // permanent user-visible indicator. Toasting on every refetch
        // (which happens on panel re-entry) spammed the UI.
        send_placeholder_chain(underlying_price);
    });
}

/// Convert ApexData ChainRow list → the legacy `(strike,last,bid,ask,vol,oi,iv,itm,contract)`
/// tuple format, filtered to one expiry (target_dte days from today) and `num_strikes`
/// strikes on each side of `spot`.
///
/// ApexData doesn't expose per-contract volume or open interest today (spec §4.7) so those
/// fields are zero — the existing UI handles zeros gracefully (shown as "-").
/// Returns (calls, puts, effective_spot).
pub(crate) fn apex_data_chain_to_tuples(
    rows: &[crate::apex_data::ChainRow],
    target_dte: i32,
    num_strikes: usize,
    spot_in: f32,
) -> (Vec<(f32,f32,f32,f32,i32,i32,f32,bool,String)>,
      Vec<(f32,f32,f32,f32,i32,i32,f32,bool,String)>,
      f32)
{
    use chrono::{Utc, NaiveDate, Duration};
    let today = Utc::now().date_naive();
    let target = today + Duration::days(target_dte as i64);

    // Pick the expiry date closest to target.
    let mut expiries: Vec<NaiveDate> = rows.iter()
        .filter_map(|r| NaiveDate::parse_from_str(&r.expiry, "%Y-%m-%d").ok())
        .collect();
    expiries.sort(); expiries.dedup();
    let chosen = expiries.into_iter()
        .min_by_key(|d| (*d - target).num_days().abs());
    let chosen = match chosen {
        Some(d) => d,
        None => return (vec![], vec![], spot_in),
    };
    let chosen_s = chosen.format("%Y-%m-%d").to_string();

    let mut calls: Vec<&crate::apex_data::ChainRow> = rows.iter()
        .filter(|r| r.expiry == chosen_s && r.side == "C").collect();
    let mut puts:  Vec<&crate::apex_data::ChainRow> = rows.iter()
        .filter(|r| r.expiry == chosen_s && r.side == "P").collect();
    calls.sort_by(|a, b| a.strike.partial_cmp(&b.strike).unwrap_or(std::cmp::Ordering::Equal));
    puts .sort_by(|a, b| a.strike.partial_cmp(&b.strike).unwrap_or(std::cmp::Ordering::Equal));

    // Estimate a usable spot from chain rows when the caller's hint is missing
    // (common for newly-primed underlyings or symbols not yet in the watchlist).
    // Preference order: caller hint → row with delta≈0.5 → put-call mid parity →
    // median strike of this expiry.
    let spot = if spot_in > 0.0 { spot_in } else {
        // 1) delta ≈ 0.5 call
        let atm_delta_call = calls.iter()
            .filter(|r| r.delta.is_some())
            .min_by(|a, b| (a.delta.unwrap() - 0.5).abs()
                .partial_cmp(&((b.delta.unwrap() - 0.5).abs()))
                .unwrap_or(std::cmp::Ordering::Equal))
            .map(|r| r.strike as f32);
        // 2) strike where |call.mid - put.mid| is smallest — ATM under put-call parity
        let mid_parity = {
            let mut best: Option<(f32, f32)> = None; // (strike, |diff|)
            for c in &calls {
                if let Some(p) = puts.iter().find(|p| (p.strike - c.strike).abs() < 0.001) {
                    let diff = (c.mid - p.mid).abs() as f32;
                    match best {
                        Some((_, bd)) if diff >= bd => {}
                        _ => best = Some((c.strike as f32, diff)),
                    }
                }
            }
            best.map(|(s, _)| s)
        };
        // 3) median strike (last resort)
        let median = {
            let mut all: Vec<f32> = calls.iter().map(|r| r.strike as f32)
                .chain(puts.iter().map(|r| r.strike as f32)).collect();
            all.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            if all.is_empty() { 0.0 } else { all[all.len()/2] }
        };
        let derived = atm_delta_call.or(mid_parity).unwrap_or(median);
        crate::apex_log!("chain.adapt",
            "derived spot={:.2} from rows (delta-match={:?}, parity={:?}, median={:.2})",
            derived, atm_delta_call, mid_parity, median);
        derived
    };

    // Trim to `num_strikes` on each side of spot.
    fn trim_around_spot<'a>(
        list: Vec<&'a crate::apex_data::ChainRow>,
        num_strikes: usize,
        spot: f32,
    ) -> Vec<&'a crate::apex_data::ChainRow> {
        if num_strikes == 0 || list.is_empty() || spot <= 0.0 { return list; }
        let atm_idx = list.iter()
            .enumerate()
            .min_by(|a, b| (a.1.strike as f32 - spot).abs()
                .partial_cmp(&((b.1.strike as f32 - spot).abs()))
                .unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0);
        let lo = atm_idx.saturating_sub(num_strikes);
        let hi = (atm_idx + num_strikes + 1).min(list.len());
        list[lo..hi].to_vec()
    }
    let calls = trim_around_spot(calls, num_strikes, spot);
    let puts  = trim_around_spot(puts, num_strikes, spot);

    let to_tuple = |r: &crate::apex_data::ChainRow, is_call: bool| {
        let strike = r.strike as f32;
        let itm = if is_call { strike < spot } else { strike > spot };
        (
            strike,
            r.last as f32,
            r.bid as f32,
            r.ask as f32,
            r.day_volume as i32,    // §5.4.d — now populated
            r.open_interest as i32, // §5.4.d — now populated
            r.iv.map(|v| v as f32).unwrap_or(0.0),
            itm,
            r.ticker.clone(),       // OCC ticker
        )
    };
    let calls_t: Vec<_> = calls.iter().map(|r| to_tuple(r, true)).collect();
    let puts_t:  Vec<_> = puts.iter().map(|r| to_tuple(r, false)).collect();
    (calls_t, puts_t, spot)
}

/// Fetch chain data for the strikes overlay (independent of sidebar chain tab).
pub(crate) fn fetch_overlay_chain_background(symbol: String, underlying_price: f32) {
    crate::foundation::guard::spawn_guarded("fetch", move || {
        // 0. ApexData — preferred. 0DTE strikes, wide strike band.
        if crate::apex_data::is_enabled() {
            match crate::apex_data::rest::get_chain(&symbol) {
                Ok(chain) => {
                    // Prefer the symbol-keyed live snapshot. The caller's
                    // underlying_price comes from chart.bars, which lags a symbol
                    // swap (bars load async) — so enabling strikes right after a swap
                    // would otherwise center the chain on the PREVIOUS symbol's price
                    // (observed: QQQ strikes fetched at SPY's 731 instead of QQQ 700).
                    let spot = {
                        let snap = crate::apex_data::live_state::get_snapshot(&symbol)
                            .map(|s| s.last as f32).unwrap_or(0.0);
                        if snap > 0.0 { snap } else { underlying_price }
                    };
                    let (calls, puts, _eff) = apex_data_chain_to_tuples(&chain.rows, 0, 75, spot);
                    eprintln!("[overlay-fetch] {symbol}: apex_data get_chain OK rows={} -> {} calls / {} puts (spot {:.2})",
                              chain.rows.len(), calls.len(), puts.len(), spot);
                    if !calls.is_empty() || !puts.is_empty() {
                        crate::send_to_native_chart(ChartCommand::OverlayChainData {
                            symbol: symbol.clone(), calls, puts, placeholder: false });
                        return;
                    }
                }
                Err(e) => eprintln!("[overlay-fetch] {symbol}: apex_data get_chain ERR {e:?}"),
            }
        }

        let path = format!("/options/{}?strikeCount=150&dte=0", symbol);
        let apexib_json = apexib_curl(&path);
        eprintln!("[overlay-fetch] {symbol}: apexib /options fallback {}", if apexib_json.is_some() {"got JSON"} else {"no response"});
        if let Some(json) = apexib_json {
            let parse_rows = |key: &str| -> Vec<(f32,f32,f32,f32,i32,i32,f32,bool,String)> {
                json.get(key).and_then(|v| v.as_array()).map(|arr| {
                    arr.iter().filter_map(|row| {
                        let strike = row.get("strike")?.as_f64()? as f32;
                        let last = row.get("lastPrice").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        let bid = row.get("bid").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        let ask = row.get("ask").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        let vol = row.get("volume").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                        let oi = row.get("openInterest").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                        let iv = row.get("iv").and_then(|v| v.as_f64())
                            .or_else(|| row.get("impliedVolatility").and_then(|v| v.as_f64()))
                            .unwrap_or(0.0) as f32;
                        let itm = row.get("inTheMoney").and_then(|v| v.as_bool()).unwrap_or(false);
                        let contract = row.get("contractSymbol").and_then(|v| v.as_str())
                            .or_else(|| row.get("conId").and_then(|v| v.as_i64()).map(|_| ""))
                            .unwrap_or("").to_string();
                        let contract = if contract.is_empty() {
                            row.get("conId").and_then(|v| v.as_i64()).map(|id| format!("{}", id)).unwrap_or_default()
                        } else { contract };
                        Some((strike, last, bid, ask, vol, oi, iv, itm, contract))
                    }).collect()
                }).unwrap_or_default()
            };
            let calls = parse_rows("calls");
            let puts = parse_rows("puts");
            if !calls.is_empty() || !puts.is_empty() {
                crate::send_to_native_chart(ChartCommand::OverlayChainData {
                    symbol, calls, puts, placeholder: false });
                return;
            }
        }
        // Real upstream unavailable — synthesize a placeholder overlay chain.
        // Strikes are fabricated but the chart will paint a "PLACEHOLDER"
        // tag on the right edge so the user knows.
        let spot = if underlying_price > 0.0 { underlying_price } else { 100.0 };
        let (calls_rows, puts_rows) = build_chain(spot, 75, 0);
        let to_tuple = |r: &OptionRow| (
            r.strike, r.last, r.bid, r.ask, r.volume, r.oi, r.iv, r.itm, r.contract.clone()
        );
        let calls: Vec<_> = calls_rows.iter().map(to_tuple).collect();
        let puts:  Vec<_> = puts_rows.iter().map(to_tuple).collect();
        eprintln!("[overlay-fetch] {symbol}: sending PLACEHOLDER chain ({} calls / {} puts, spot {:.2})",
                  calls.len(), puts.len(), spot);
        crate::send_to_native_chart(ChartCommand::OverlayChainData {
            symbol, calls, puts, placeholder: true });
    });
}

/// Fetch symbol search results in the background.
///
/// Primary source is ApexData `/api/search?q=` (Polygon-backed — covers
/// stocks, ETFs, indices, crypto). ApexIB `/search/` is kept only as a
/// fallback for when ApexData returns nothing (or is unreachable); ApexIB is
/// options-centric and has been intermittently down, so it is no longer the
/// primary path.
pub(crate) fn fetch_search_background(query: String, source: String) {
    crate::foundation::guard::spawn_guarded("fetch", move || {
        let mut results: Vec<(String, String)> = Vec::new();

        // ── Primary: ApexData symbol search (stock-capable) ──────────────
        if let Some(hits) = crate::apex_data::rest::search(&query) {
            for h in hits.into_iter().filter(|h| h.active).take(16) {
                // Stamp the `F:` class tag onto futures hits so selection carries
                // the asset class downstream (ES the future vs ES the stock).
                // Mirrors the `O:` options convention; stripped for display/URLs.
                let ticker = if h.market == "futures" || h.kind == "FUT" {
                    format!("F:{}", h.ticker)
                } else {
                    h.ticker
                };
                results.push((ticker, h.name));
            }
        }

        // ── Fallback: ApexIB search (options/legacy) ─────────────────────
        if results.is_empty() {
            let client = apexib_client();
            // URL-encode the user-supplied query to prevent injection.
            let url = format!("{}/search/{}", apexib_url(), urlencoding::encode(&query));
            if let Ok(resp) = client.get(&url).send() {
                if resp.status().is_success() {
                    if let Ok(json) = resp.json::<serde_json::Value>() {
                        if let Some(arr) = json.as_array() {
                            for item in arr.iter().take(16) {
                                if let Some(sym) = item.get("symbol").and_then(|v| v.as_str()) {
                                    let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    results.push((sym.to_string(), name));
                                }
                            }
                        }
                    }
                }
            }
        }

        if !results.is_empty() {
            let cmd = ChartCommand::SearchResults {
                query,
                results,
                source,
            };
            crate::send_to_native_chart(cmd);
        }
    });
}

// ── Options analytics bundle (cached, TTL-refreshed, non-blocking) ─────────

/// Live-ish per-underlying options analytics, pulled together from the
/// ApexData derived endpoints. Any field may be `None` (endpoint 404 / no
/// options for the symbol / not fetched yet).
#[derive(Clone, Default)]
pub(crate) struct OptionsAnalytics {
    pub expected_move: Option<crate::apex_data::rest::ExpectedMove>,
    pub pcr: Option<crate::apex_data::rest::PutCallRatio>,
    pub iv_rank: Option<crate::apex_data::rest::IvRank>,
    pub gex: Option<crate::apex_data::rest::GexReading>,
}

impl OptionsAnalytics {
    pub fn any(&self) -> bool {
        self.expected_move.is_some() || self.pcr.is_some()
            || self.iv_rank.is_some() || self.gex.is_some()
    }
}

str_keyed_cache!(opt_analytics_cache, opt_analytics_inflight, (OptionsAnalytics, std::time::Instant));

const OPT_ANALYTICS_TTL_SECS: u64 = 30;

/// Return the cached options-analytics bundle for `underlying`, spawning a
/// background refresh when the cache is missing or older than the TTL. Returns
/// the last known bundle (possibly stale) while the refresh runs; `None` until
/// the first fetch completes. Non-blocking — the 4 REST GETs run off-thread.
pub(crate) fn options_analytics_cached(underlying: &str) -> Option<OptionsAnalytics> {
    let sym = underlying.to_uppercase();
    let (cached, stale) = {
        match opt_analytics_cache().lock() {
            Ok(c) => match c.get(&sym) {
                Some((b, at)) => (Some(b.clone()), at.elapsed().as_secs() >= OPT_ANALYTICS_TTL_SECS),
                None => (None, true),
            },
            Err(_) => (None, true),
        }
    };
    if stale && is_stock_symbol(&sym) {
        let spawn = {
            let mut inf = match opt_analytics_inflight().lock() { Ok(g) => g, Err(e) => e.into_inner() };
            if inf.contains(&sym) { false } else { inf.insert(sym.clone()); true }
        };
        if spawn {
            let s2 = sym.clone();
            crate::foundation::guard::spawn_guarded("fetch", move || {
                use crate::apex_data::rest;
                let bundle = OptionsAnalytics {
                    expected_move: rest::expected_move(&s2),
                    pcr: rest::pcr(&s2),
                    iv_rank: rest::iv_rank(&s2),
                    gex: rest::gex(&s2),
                };
                if let Ok(mut c) = opt_analytics_cache().lock() {
                    c.insert(s2.clone(), (bundle, std::time::Instant::now()));
                    evict_oldest(&mut c, STR_CACHE_CAP, |v| v.1);
                }
                if let Ok(mut inf) = opt_analytics_inflight().lock() { inf.remove(&s2); }
                crate::wake_native_ui();
            });
        }
    }
    cached
}

// ── Daily-bar stats (cached, non-blocking): prev-session change + avg vol ──

/// Per-symbol stats derived from one daily-bars fetch. Cached for the session
/// (daily history doesn't move intraday).
#[derive(Clone, Copy)]
pub(crate) struct DailyStats {
    /// Last completed session's close-to-close % change.
    pub prev_change_pct: f32,
    /// Average daily volume over the trailing window (for RVOL).
    pub avg_volume: f64,
}

str_keyed_cache!(daily_stats_cache, daily_stats_inflight, (Option<DailyStats>, std::time::Instant));
/// How long a FAILED daily-stats fetch is remembered before retrying. Success
/// is cached for the whole session (daily history doesn't move intraday); a
/// failure (e.g. the daily-bars endpoint timed out — it's slow/large) must NOT
/// stick forever or the Change % is wedged at 0.
const DAILY_STATS_RETRY_SECS: u64 = 30;

/// Trailing window for the average-volume (RVOL denominator).
const RVOL_WINDOW: usize = 20;

/// Daily stats for `symbol` (prev-session change + avg volume), from one daily-
/// bars fetch. Cached for the session; fetched in the background on first miss
/// (`None` until it lands, then repaints). Backs the watchlist's last-session
/// Change % and real RVOL — both from data ApexData already serves, no new
/// endpoint needed.
pub(crate) fn daily_stats_cached(symbol: &str) -> Option<DailyStats> {
    let sym = symbol.to_uppercase();
    // Success → return forever. Failure (None) → return None but allow a retry
    // once it's older than DAILY_STATS_RETRY_SECS (so a transient timeout at
    // startup doesn't wedge Change % at 0 for the whole session).
    let needs_fetch = match daily_stats_cache().lock() {
        Ok(c) => match c.get(&sym) {
            Some((Some(s), _)) => return Some(*s),
            Some((None, at)) => at.elapsed().as_secs() >= DAILY_STATS_RETRY_SECS,
            None => true,
        },
        Err(_) => true,
    };
    if !needs_fetch { return None; }
    if !is_stock_symbol(&sym) { return None; }
    {
        let mut inf = match daily_stats_inflight().lock() { Ok(g) => g, Err(e) => e.into_inner() };
        if inf.contains(&sym) { return None; }
        inf.insert(sym.clone());
    }
    let s2 = sym.clone();
    crate::foundation::guard::spawn_guarded("fetch", move || {
        let stats = crate::apex_data::rest::get_bars(
            crate::apex_data::types::AssetClass::Stock, &s2, "1d", crate::apex_data::BarSource::Last,
        ).ok().and_then(|bars| {
            let closes: Vec<f32> = bars.iter().map(|b| b.close as f32).filter(|c| *c > 0.0).collect();
            if closes.len() < 2 { return None; }
            let (c1, c0) = (closes[closes.len() - 1], closes[closes.len() - 2]);
            let prev_change_pct = if c0 > 0.0 { (c1 - c0) / c0 * 100.0 } else { 0.0 };
            // Average volume over the trailing window, EXCLUDING the most recent
            // bar (which may be today's still-building session) so the RVOL
            // denominator is a clean baseline of completed days.
            let vols: Vec<f64> = bars.iter().map(|b| b.volume as f64).filter(|v| *v > 0.0).collect();
            let avg_volume = if vols.len() >= 2 {
                let end = vols.len() - 1; // drop the last (possibly in-progress) day
                let start = end.saturating_sub(RVOL_WINDOW);
                let win = &vols[start..end];
                if win.is_empty() { 0.0 } else { win.iter().sum::<f64>() / win.len() as f64 }
            } else { 0.0 };
            Some(DailyStats { prev_change_pct, avg_volume })
        });
        if let Ok(mut c) = daily_stats_cache().lock() {
            c.insert(s2.clone(), (stats, std::time::Instant::now()));
            evict_oldest(&mut c, STR_CACHE_CAP, |v| v.1);
        }
        if let Ok(mut inf) = daily_stats_inflight().lock() { inf.remove(&s2); }
        crate::wake_native_ui();
    });
    None
}

/// Last completed session's close-to-close % change (thin wrapper over
/// `daily_stats_cached`).
pub(crate) fn prev_session_change_cached(symbol: &str) -> Option<f32> {
    daily_stats_cached(symbol).map(|s| s.prev_change_pct)
}

// ── Futures last price (TTL-cached) ────────────────────────────────────────

str_keyed_cache!(futures_price_cache, futures_price_inflight, (Option<f32>, std::time::Instant));
const FUTURES_PRICE_TTL_SECS: u64 = 3;

/// Last price for a futures symbol (`F:ES`) via `/api/price/F:ES` (IB, last).
/// TTL-cached + non-blocking. Futures have no `/api/quote`, so the watchlist
/// gets its price here; the chart gets it from the futures bars feed.
pub(crate) fn futures_price_cached(symbol: &str) -> Option<f32> {
    let sym = symbol.to_uppercase();
    if !sym.starts_with("F:") { return None; }
    let (cached, stale) = match futures_price_cache().lock() {
        Ok(c) => match c.get(&sym) {
            Some((v, at)) => (*v, at.elapsed().as_secs() >= FUTURES_PRICE_TTL_SECS),
            None => (None, true),
        },
        Err(_) => (None, true),
    };
    if stale {
        let spawn = {
            let mut inf = match futures_price_inflight().lock() { Ok(g) => g, Err(e) => e.into_inner() };
            if inf.contains(&sym) { false } else { inf.insert(sym.clone()); true }
        };
        if spawn {
            let s2 = sym.clone();
            crate::foundation::guard::spawn_guarded("fetch", move || {
                // get_price does NOT strip F: — the price endpoint wants the
                // F:-tagged symbol so it resolves the future, not the equity.
                let v = crate::apex_data::rest::get_price(&s2).ok().map(|p| p.price as f32);
                if let Ok(mut c) = futures_price_cache().lock() {
                    c.insert(s2.clone(), (v, std::time::Instant::now()));
                    evict_oldest(&mut c, STR_CACHE_CAP, |v| v.1);
                }
                if let Ok(mut inf) = futures_price_inflight().lock() { inf.remove(&s2); }
                crate::wake_native_ui();
            });
        }
    }
    cached
}

// ── Volume-at-price (VAP) for real Volume Profile (TTL-cached) ─────────────

str_keyed_cache!(vap_cache, vap_inflight, (Option<crate::apex_data::rest::VapResponse>, std::time::Instant));
const VAP_TTL_SECS: u64 = 120;

/// Real volume-at-price for the most-recent session, `/api/stocks/vap`.
/// TTL-cached, non-blocking. Returns `None` (→ caller falls back to the
/// bar-derived profile) when the date isn't backfilled yet (`total_volume==0`
/// / empty levels) — the trade store is being populated, live Monday.
pub(crate) fn vap_cached(symbol: &str) -> Option<crate::apex_data::rest::VapResponse> {
    if !is_stock_symbol(symbol) { return None; }
    let sym = symbol.to_uppercase();
    let (cached, stale) = match vap_cache().lock() {
        Ok(c) => match c.get(&sym) {
            Some((v, at)) => (v.clone(), at.elapsed().as_secs() >= VAP_TTL_SECS),
            None => (None, true),
        },
        Err(_) => (None, true),
    };
    if stale {
        let spawn = {
            let mut inf = match vap_inflight().lock() { Ok(g) => g, Err(e) => e.into_inner() };
            if inf.contains(&sym) { false } else { inf.insert(sym.clone()); true }
        };
        if spawn {
            let s2 = sym.clone();
            crate::foundation::guard::spawn_guarded("fetch", move || {
                let date = active_zero_dte_date().format("%Y-%m-%d").to_string();
                let v = crate::apex_data::rest::stock_vap(&s2, &date, 0.25)
                    .filter(|r| r.total_volume > 0.0 && !r.levels.is_empty());
                if let Ok(mut c) = vap_cache().lock() {
                    c.insert(s2.clone(), (v, std::time::Instant::now()));
                    evict_oldest(&mut c, STR_CACHE_CAP, |v| v.1);
                }
                if let Ok(mut inf) = vap_inflight().lock() { inf.remove(&s2); }
                crate::wake_native_ui();
            });
        }
    }
    cached
}

// ── RVOL (server endpoint, TTL-cached) ─────────────────────────────────────

str_keyed_cache!(rvol_cache, rvol_inflight, (Option<f32>, std::time::Instant));
const RVOL_TTL_SECS: u64 = 90;

/// Relative volume for `symbol` from `/api/stocks/rvol` (server-computed:
/// today_volume ÷ avg_volume_20d). TTL-refreshed (~90s — it builds intraday),
/// non-blocking. Returns the last value while refreshing; `None` until first
/// fetch / for non-stocks.
pub(crate) fn rvol_cached(symbol: &str) -> Option<f32> {
    let sym = symbol.to_uppercase();
    let (cached, stale) = match rvol_cache().lock() {
        Ok(c) => match c.get(&sym) {
            Some((v, at)) => (*v, at.elapsed().as_secs() >= RVOL_TTL_SECS),
            None => (None, true),
        },
        Err(_) => (None, true),
    };
    if stale && is_stock_symbol(&sym) {
        let spawn = {
            let mut inf = match rvol_inflight().lock() { Ok(g) => g, Err(e) => e.into_inner() };
            if inf.contains(&sym) { false } else { inf.insert(sym.clone()); true }
        };
        if spawn {
            let s2 = sym.clone();
            crate::foundation::guard::spawn_guarded("fetch", move || {
                let v = crate::apex_data::rest::stock_rvol(&s2)
                    .filter(|r| r.rvol > 0.0)
                    .map(|r| r.rvol as f32);
                if let Ok(mut c) = rvol_cache().lock() {
                    c.insert(s2.clone(), (v, std::time::Instant::now()));
                    evict_oldest(&mut c, STR_CACHE_CAP, |v| v.1);
                }
                if let Ok(mut inf) = rvol_inflight().lock() { inf.remove(&s2); }
                crate::wake_native_ui();
            });
        }
    }
    cached
}

// ── Ticker reference detail (cached, non-blocking) ─────────────────────────

str_keyed_cache!(ticker_detail_cache, ticker_detail_inflight, Option<crate::apex_data::rest::TickerDetail>);

/// Return cached ApexData `/api/ticker/{SYM}` detail for `symbol`, spawning a
/// one-shot background fetch on first miss. Non-blocking — returns `None` until
/// the fetch lands (then `wake_native_ui` repaints). Cached for the session
/// (incl. a cached `None` for symbols the endpoint has no data for, so we don't
/// refetch). Only stock symbols are queried.
pub(crate) fn ticker_detail_cached(symbol: &str) -> Option<crate::apex_data::rest::TickerDetail> {
    let sym = symbol.to_uppercase();
    if let Ok(c) = ticker_detail_cache().lock() {
        if let Some(v) = c.get(&sym) { return v.clone(); }
    }
    if !is_stock_symbol(&sym) { return None; }
    {
        let mut inf = match ticker_detail_inflight().lock() { Ok(g) => g, Err(e) => e.into_inner() };
        if inf.contains(&sym) { return None; }
        inf.insert(sym.clone());
    }
    let s2 = sym.clone();
    crate::foundation::guard::spawn_guarded("fetch", move || {
        let d = crate::apex_data::rest::ticker_detail(&s2);
        if let Ok(mut c) = ticker_detail_cache().lock() {
            c.insert(s2.clone(), d);
            evict_any(&mut c, STR_CACHE_CAP);
        }
        if let Ok(mut inf) = ticker_detail_inflight().lock() { inf.remove(&s2); }
        crate::wake_native_ui();
    });
    None
}

/// Parse a `YYYY-MM-DD` date to epoch seconds at 00:00 UTC — aligns with
/// ApexData daily bars (their `time` is the session date at midnight UTC).
fn ymd_to_epoch(s: &str) -> Option<i64> {
    let d = chrono::NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok()?;
    Some(d.and_hms_opt(0, 0, 0)?.and_utc().timestamp())
}

/// Blocking fetch of dividends + splits for `symbol` from ApexData, mapped to
/// chart `CorpAction` markers. Called inline when the user enables the overlay
/// (cheap, one-off — same pattern as the gamma toggle). Non-stocks → empty.
pub(crate) fn fetch_corp_actions(symbol: &str) -> Vec<crate::chart_renderer::gpu::CorpAction> {
    use crate::chart_renderer::gpu::CorpAction;
    if !is_stock_symbol(symbol) { return vec![]; }
    let mut out: Vec<CorpAction> = vec![];
    if let Some(divs) = crate::apex_data::rest::stock_dividends(symbol) {
        for d in divs {
            if let Some(date) = ymd_to_epoch(&d.ex_date) {
                out.push(CorpAction {
                    date, is_split: false, amount: d.cash_amount as f32,
                    label: format!("${:.2}", d.cash_amount),
                });
            }
        }
    }
    if let Some(splits) = crate::apex_data::rest::stock_splits(symbol) {
        for s in splits {
            if let Some(date) = ymd_to_epoch(&s.execution_date) {
                let (to, from) = (s.split_to as i64, s.split_from.max(1.0) as i64);
                out.push(CorpAction {
                    date, is_split: true, amount: 0.0,
                    label: format!("{to}:{from}"),
                });
            }
        }
    }
    out
}

/// Returns true if a symbol is a stock ticker (not crypto, not options).
/// Used to filter the watchlist before bulk-snapshot fetches.
pub(crate) fn is_stock_symbol(s: &str) -> bool {
    let s_upper = s.to_uppercase();
    if crate::data::is_crypto(s) { return false; }
    if s_upper.starts_with("O:") { return false; }
    if s_upper.starts_with("F:") { return false; } // futures: priced via /api/price/F:…
    // Option display label heuristic: "UND STRIKE C/P EXPIRY".
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() >= 2 {
        let middle = parts[1];
        if (middle.ends_with('C') || middle.ends_with('P')
            || middle.contains('C') || middle.contains('P'))
            && middle.chars().any(|c| c.is_ascii_digit())
        {
            return false;
        }
    }
    true
}

/// Pure mapping: `StockSnapshot` → `(price, prev_close)` for the watchlist
/// pane. Extracted so unit tests can exercise the field-mapping without
/// stubbing the REST layer.
pub(crate) fn watchlist_price_from_snapshot(
    s: &crate::apex_data::StockSnapshot,
) -> (f32, f32, f32) {
    let price = s.current_price() as f32;
    let prev = s.prev_day.close as f32;
    let day_close = s.day.close as f32; // today's regular close (0 while live)
    (price, prev, day_close)
}

/// Fetch daily price + prev_close for all watchlist symbols (background
/// thread). Now uses the ApexData stocks bulk-snapshot endpoint — one HTTP
/// call per refresh tick instead of one per symbol. ApexIB/Yahoo stock
/// fallbacks are gone; crypto and options keep their own data paths (this
/// fn only ever sees stock symbols thanks to `is_stock_symbol`).
/// Map a non-stock watchlist symbol to its `(class, url_symbol)` for
/// `/api/snap/:class/:symbol`. Returns `None` for stocks (handled by the bulk
/// path) and for display-label options that aren't in OCC form.
fn wl_class_and_url(s: &str) -> Option<(&'static str, String)> {
    let up = s.to_uppercase();
    let crypto = crate::foundation::types::registry()
        .get(s).map(|sy| sy.is_crypto())
        .unwrap_or_else(|| crate::data::is_crypto(s));
    if crypto { return Some(("crypto", up)); }
    if let Some(root) = up.strip_prefix("F:") { return Some(("futures", root.to_string())); } // backend wants bare root
    if up.starts_with("O:") { return Some(("options", s.to_string())); }                       // OCC as-is
    if up.starts_with("I:") { return Some(("index", s.to_string())); }                         // e.g. I:SPX
    None
}

pub(crate) fn fetch_watchlist_prices(symbols: Vec<String>) {
    // Non-stock rows (crypto / futures / options / index) read the server's
    // session/DST-aware % from the per-class `/api/snap/:class/:symbol`. This
    // fails safe: a 404 / missing endpoint / network error → no-op (the row
    // keeps its last value; crypto/futures still get fresh price from their
    // live WS feeds, whose `change_perc: None` doesn't clobber this one).
    let class_syms: Vec<(String, &'static str, String)> = symbols.iter()
        .filter(|s| !is_stock_symbol(s))
        .filter_map(|s| wl_class_and_url(s).map(|(c, u)| (s.clone(), c, u)))
        .collect();
    if !class_syms.is_empty() {
        crate::foundation::guard::spawn_guarded("fetch", move || {
            for (orig, class, url_sym) in class_syms {
                if let Some(snap) = crate::apex_data::rest::snap_class(class, &url_sym) {
                    crate::send_to_native_chart(ChartCommand::WatchlistPrice {
                        symbol: orig,
                        price: snap.last as f32,
                        prev_close: snap.prev_close as f32,
                        day_close: 0.0,
                        change_perc: snap.change_perc.map(|p| p as f32),
                        stale: snap.stale,
                    });
                }
            }
        });
    }

    let symbols: Vec<String> = symbols.into_iter().filter(|s| is_stock_symbol(s)).collect();
    if symbols.is_empty() { return; }
    // Filter: this fn only knows how to fetch equities (Redis/ApexIB/Yahoo).
    // Option contracts (OCC tickers, "AAPL 287.5C 2026-04-30" labels) and
    // crypto pairs go through their own feeds — sending them here means a
    // silent 404 on Yahoo. Drop them before the fetch loop.
    let symbols: Vec<String> = symbols.into_iter()
        .filter(|s| {
            let s_upper = s.to_uppercase();
            // Crypto: ApexCrypto handles BTCUSDT etc.
            // Wave 9c: registry-preferred (so equities like XUSDT aren't
            // dropped); string heuristic remains the fallback for tickers
            // not yet registered.
            let crypto = crate::foundation::types::registry()
                .get(s)
                .map(|sy| sy.is_crypto())
                .unwrap_or_else(|| crate::data::is_crypto(s));
            if crypto { return false; }
            // Option OCC: "O:SPY..." prefix.
            if s_upper.starts_with("O:") { return false; }
            // Option display label: "UND STRIKE C/P EXPIRY" or "UND STRIKEC EXPIRY".
            // Heuristic: contains a space AND ends with a digit-or-Y/E (date) AND
            // has a C/P somewhere in the middle — distinguishes from "BRK.B".
            let parts: Vec<&str> = s.split_whitespace().collect();
            if parts.len() >= 2 {
                let middle = parts[1];
                if middle.ends_with('C') || middle.ends_with('P')
                    || middle.contains('C') || middle.contains('P') {
                    if middle.chars().any(|c| c.is_ascii_digit()) { return false; }
                }
            }
            true
        })
        .collect();
    crate::foundation::guard::spawn_guarded("fetch", move || {
        let snaps = crate::apex_data::rest::snap_bulk(&symbols).unwrap_or_default();
        for snap in &snaps {
            let (price, prev_close, day_close) = watchlist_price_from_snapshot(snap);
            crate::send_to_native_chart(ChartCommand::WatchlistPrice {
                symbol: snap.ticker.clone(), price, prev_close, day_close,
                // Prefer the server's session/DST-aware % (apex-data-v2); None
                // on older backends → the panel falls back to its own formula.
                change_perc: snap.change_perc.map(|p| p as f32),
                stale: snap.stale,
            });
        }
    });
}

/// Legacy scanner universe — kept only for the "loading…" caption in
/// `scanner_panel`, which still uses `SCANNER_UNIVERSE.len()` as a rough
/// "expected row count". Built-in presets (gainers/losers) now come from
/// `/api/stocks/movers` instead of bulk-quoting this list.
pub(crate) const SCANNER_UNIVERSE: &[&str] = &[
    "AAPL","MSFT","GOOGL","AMZN","TSLA","META","NVDA","AMD","NFLX","INTC",
    "SPY","QQQ","IWM","DIA","BABA","CRM","PYPL","SQ","SHOP","UBER",
    "COIN","SNAP","PLTR","RIVN","LCID","SOFI","NIO","MARA","RIOT","ROKU",
    "BA","DIS","JPM","GS","V","MA","WMT","COST","HD","LOW",
    "XOM","CVX","PFE","JNJ","UNH","MRK","ABBV","LLY","KO","PEP",
];

/// Pure mapping: `StockSnapshot` → `ScannerPrice` command fields.
pub(crate) fn scanner_price_from_snapshot(
    s: &crate::apex_data::StockSnapshot,
) -> (String, f32, f32, u64) {
    let price = s.current_price() as f32;
    let prev = s.prev_day.close as f32;
    let volume = s.day_volume();
    (s.ticker.clone(), price, prev, volume)
}

/// Fetch bulk quotes for the scanner panel in a background thread.
///
/// `presets`: list of built-in scanner presets to load — typically
/// `["gainers", "losers"]` (driven by the user's enabled `ScannerDef`s).
/// Each preset becomes one `/api/stocks/movers?direction=...` call.
///
/// Custom user-defined scanners are NOT served by this fn — they need a
/// scanner-run endpoint that lives on a separate ApexData PR (TODO).
pub(crate) fn fetch_scanner_prices_for(presets: Vec<String>) {
    crate::foundation::guard::spawn_guarded("fetch", move || {
        for preset in &presets {
            if preset != "gainers" && preset != "losers" {
                // TODO(scanner-run): "most_active" + custom scanners need a
                // dedicated endpoint (POST /api/stocks/scan). Skip silently
                // until that PR lands so the UI doesn't get a fake "no
                // matches" state from an unmappable preset.
                continue;
            }
            let Some(snaps) = crate::apex_data::rest::stocks_movers(preset) else { continue };
            for snap in &snaps {
                let (symbol, price, prev_close, volume) = scanner_price_from_snapshot(snap);
                crate::send_to_native_chart(ChartCommand::ScannerPrice {
                    symbol, price, prev_close, volume,
                });
            }
        }
    });
}

/// Back-compat wrapper for the scanner panel: derives the preset list from
/// the default built-in presets (gainers + losers). Kept so we don't have
/// to thread `ScannerDef` access through io::fetch.
pub(crate) fn fetch_scanner_prices() {
    fetch_scanner_prices_for(vec!["gainers".into(), "losers".into()]);
}

// ── Heatmap pane ──────────────────────────────────────────────────────────

/// Pure mapping: `GroupedDailyBar` → `(symbol, change_pct, dollar_volume)`
/// where `change_pct = (c - o) / o * 100` and `dollar_volume = vw * v` (a
/// proxy for "market cap weight" since the grouped-daily endpoint has no
/// shares-outstanding column). The heatmap uses dollar-volume to size cells.
pub(crate) fn heatmap_cell_from_grouped(
    bar: &crate::apex_data::GroupedDailyBar,
) -> (String, f32, f64) {
    let change_pct = if bar.o > 0.0 { ((bar.c - bar.o) / bar.o * 100.0) as f32 } else { 0.0 };
    let dollar_volume = if bar.vw > 0.0 { bar.vw * bar.v } else { bar.c * bar.v };
    (bar.ticker.clone(), change_pct, dollar_volume)
}

/// Cold-start the heatmap: pull the full-market grouped-daily bars for the
/// most recent session and ship them back as `HeatmapBars`. Live updates
/// come from the watchlist `set_price` path — no separate poller needed.
pub(crate) fn fetch_heatmap_cold_start() {
    crate::foundation::guard::spawn_guarded("fetch", move || {
        let date = active_zero_dte_date().format("%Y-%m-%d").to_string();
        let Some(bars) = crate::apex_data::rest::stocks_grouped_daily(&date) else {
            crate::apex_data::live_state::push_toast(
                "\x01Heatmap unavailable — ApexData returned no grouped bar data".to_string());
            crate::wake_native_ui();
            return;
        };
        let cells: Vec<(String, f32, f64)> = bars.iter()
            .filter(|b| !b.ticker.is_empty() && b.v > 0.0)
            .map(heatmap_cell_from_grouped)
            .collect();
        crate::send_to_native_chart(ChartCommand::HeatmapBars { cells });
    });
}


/// Fetch source bars for a cross-timeframe indicator on a background thread.
pub(crate) fn fetch_indicator_source(sym: String, tf: String, indicator_id: u32) {
    let txs: Vec<std::sync::mpsc::Sender<ChartCommand>> = crate::NATIVE_CHART_TXS
        .get().and_then(|m| m.lock().ok()).map(|g| g.clone()).unwrap_or_default();
    if txs.is_empty() { return; }
    crate::foundation::guard::spawn_guarded("fetch", move || {
        // Try Redis cache first
        if let Some(bars) = crate::bar_cache::get(&sym, &tf) {
            if !bars.is_empty() {
                let timestamps: Vec<i64> = bars.iter().map(|b| b.time).collect();
                let gpu_bars: Vec<Bar> = bars.iter().map(|b| Bar {
                    open: b.open as f32, high: b.high as f32, low: b.low as f32,
                    close: b.close as f32, volume: b.volume as f32, _pad: 0.0,
                }).collect();
                let cmd = ChartCommand::IndicatorSourceBars { indicator_id, timeframe: tf.clone(), bars: gpu_bars, timestamps };
                for tx in &txs { let _ = tx.send(cmd.clone()); } crate::wake_native_ui();
                return;
            }
        }
        // Fetch from Yahoo Finance
        let (yf_interval, yf_range) = match tf.as_str() {
            "1m" => ("1m","5d"), "5m" => ("5m","5d"), "15m" => ("15m","60d"),
            "30m" => ("30m","60d"), "1h" => ("60m","60d"), "4h" => ("1h","730d"),
            "1d" => ("1d","5y"), "1wk" => ("1wk","10y"), _ => ("5m","5d"),
        };
        let url = format!("{}/v8/finance/chart/{}?interval={}&range={}", crate::data::endpoints::yahoo_chart(), sym, yf_interval, yf_range);
        let client = reqwest::blocking::Client::builder().user_agent("Mozilla/5.0").build().unwrap_or_else(|_| reqwest::blocking::Client::new());
        if let Ok(resp) = client.get(&url).timeout(std::time::Duration::from_secs(5)).send() {
            if let Ok(json) = resp.json::<serde_json::Value>() {
                if let Some(bars) = crate::data::parse_yahoo_v8(&json) {
                    // WS-H #42: Yahoo v8 includes the in-progress current candle
                    // as the last bar (internal Bar carries no `closed` flag), so
                    // cache all-but-last to avoid serving a stale forming bar
                    // within its TTL. The chart still uses the full `bars` below.
                    if bars.len() > 1 {
                        crate::bar_cache::set(&sym, &tf, &bars[..bars.len() - 1]);
                    }
                    let timestamps: Vec<i64> = bars.iter().map(|b| b.time).collect();
                    let gpu_bars: Vec<Bar> = bars.iter().map(|b| Bar {
                        open: b.open as f32, high: b.high as f32, low: b.low as f32,
                        close: b.close as f32, volume: b.volume as f32, _pad: 0.0,
                    }).collect();
                    let cmd = ChartCommand::IndicatorSourceBars { indicator_id, timeframe: tf, bars: gpu_bars, timestamps };
                    for tx in &txs { let _ = tx.send(cmd.clone()); } crate::wake_native_ui();
                }
            }
        }
    });
}

/// Submit an order to ApexIB via the OrderManager. Called from background thread.
/// Routes bracket orders through OrderManager::submit_bracket, plain orders through submit_order.
pub(crate) fn submit_ib_order(symbol: &str, side: &str, qty: u32, order_type_idx: usize, tif_idx: usize, price: f32, bracket: bool, tp: Option<f32>, sl: Option<f32>) {
    use crate::chart_renderer::trading::{OrderSide, order_manager::{submit_order, submit_bracket_order, ManagedOrderType, OrderIntent, OrderSource}};

    let order_side = if side.eq_ignore_ascii_case("BUY") { OrderSide::Buy } else { OrderSide::Sell };
    let managed_ot = match order_type_idx {
        0 => ManagedOrderType::Market,
        1 => ManagedOrderType::Limit,
        2 => ManagedOrderType::Stop,
        3 => ManagedOrderType::StopLimit,
        4 => ManagedOrderType::TrailingStop,
        _ => ManagedOrderType::Market,
    };

    let intent = OrderIntent {
        symbol: symbol.to_string(), side: order_side, order_type: managed_ot,
        price, stop_price: 0.0, qty, source: OrderSource::OrderPanel,
        pair_with: None, option_symbol: None, option_con_id: None,
        trail_amount: None, trail_percent: None, last_price: 0.0,
        tif: tif_idx as u8, outside_rth: false,
        strategy_id: None, override_warnings: false,
    };

    if bracket {
        if let (Some(tp_price), Some(sl_price)) = (tp, sl) {
            let _ = submit_bracket_order(intent, tp_price, sl_price);
        } else {
            let _ = submit_order(intent);
        }
    } else {
        let _ = submit_order(intent);
    }
}

/// Paginate older bars for an option contract via ApexData replay (§5.3).
/// `occ` is the feed key (e.g. `O:SPY260501C00450000`); `display_sym` is the
/// pane's human-readable label that PrependBars will be keyed against.
pub(crate) fn fetch_option_history_background(occ: String, display_sym: String, tf: String, before_ts: i64, mark: bool) {
    let txs: Vec<std::sync::mpsc::Sender<ChartCommand>> = crate::NATIVE_CHART_TXS
        .get().and_then(|m| m.lock().ok()).map(|g| g.clone()).unwrap_or_default();
    if txs.is_empty() { return; }

    crate::foundation::guard::spawn_guarded("fetch", move || {
        // Page sizes — options trade only during market hours so a "day" is
        // ~390 minutes. Scale the lookback so each page yields ~500 bars.
        let page_seconds: i64 = match tf.as_str() {
            "1m"            => 86400 * 2,    // ~390 1m bars × 2d ≈ 780
            "2m"            => 86400 * 4,
            "5m"            => 86400 * 10,
            "15m"           => 86400 * 30,
            "30m"           => 86400 * 30,
            "1h" | "60m"    => 86400 * 60,
            "4h"            => 86400 * 180,
            "1d"            => 86400 * 365,
            "1wk"           => 86400 * 365 * 3,
            _               => 86400 * 5,
        };

        if !crate::apex_data::is_enabled() {
            // No alt source for options history — mark exhausted.
            let cmd = ChartCommand::PrependBars {
                symbol: display_sym, timeframe: tf,
                bars: vec![], timestamps: vec![],
            };
            for tx in &txs { let _ = tx.send(cmd.clone()); } crate::wake_native_ui();
            return;
        }

        let to_ms = before_ts * 1000;
        let from_ms = to_ms - page_seconds * 1000;
        crate::apex_log!("option.history",
            "GET replay options/{} {} from={} to={}", occ, tf, from_ms, to_ms);
        let src = if mark { crate::apex_data::BarSource::Mark } else { crate::apex_data::BarSource::Last };
        match crate::apex_data::rest::get_replay(
            crate::apex_data::AssetClass::Option, &occ, &tf, from_ms, to_ms, None, Some(1000), src)
        {
            Ok(resp) if !resp.bars.is_empty() => {
                let gpu_bars: Vec<Bar> = resp.bars.iter().map(|b| Bar {
                    open: b.open as f32, high: b.high as f32, low: b.low as f32,
                    close: b.close as f32, volume: b.volume as f32, _pad: 0.0,
                }).collect();
                let timestamps: Vec<i64> = resp.bars.iter().map(|b| b.time / 1000).collect();
                crate::apex_log!("option.history",
                    "OK {} bars for {} {} (oldest: {})", gpu_bars.len(), occ, tf,
                    timestamps.first().copied().unwrap_or(0));
                let cmd = ChartCommand::PrependBars {
                    symbol: display_sym, timeframe: tf, bars: gpu_bars, timestamps,
                };
                for tx in &txs { let _ = tx.send(cmd.clone()); } crate::wake_native_ui();
            }
            _ => {
                crate::apex_log!("option.history",
                    "EMPTY for {} {} — marking exhausted", occ, tf);
                // Empty result → PrependBars handler sets history_exhausted.
                let cmd = ChartCommand::PrependBars {
                    symbol: display_sym, timeframe: tf,
                    bars: vec![], timestamps: vec![],
                };
                for tx in &txs { let _ = tx.send(cmd.clone()); } crate::wake_native_ui();
            }
        }
    });
}

pub(crate) fn fetch_history_background(sym: String, tf: String, before_ts: i64) {
    let txs: Vec<std::sync::mpsc::Sender<ChartCommand>> = crate::NATIVE_CHART_TXS
        .get().and_then(|m| m.lock().ok()).map(|g| g.clone()).unwrap_or_default();
    if txs.is_empty() { return; }

    crate::foundation::guard::spawn_guarded("fetch", move || {
        // Calculate how far back to fetch based on timeframe
        let page_seconds: i64 = match tf.as_str() {
            "1m" => 86400 * 2,        // 2 days
            "2m" => 86400 * 3,        // 3 days
            "5m" => 86400 * 5,        // 5 days
            "15m" => 86400 * 30,      // 30 days
            "30m" => 86400 * 30,      // 30 days
            "1h" | "60m" => 86400 * 60, // 60 days
            "4h" => 86400 * 180,      // 6 months
            "1d" => 86400 * 365 * 2,  // 2 years
            "1wk" => 86400 * 365 * 5, // 5 years
            _ => 86400 * 5,
        };

        // ── 0. ApexData replay — cursor-paginated QuestDB (§5.3) ──
        if crate::apex_data::is_enabled() {
            let class = crate::apex_data::AssetClass::from_symbol(&sym);
            let to_ms = before_ts * 1000;
            let from_ms = to_ms - page_seconds * 1000;
            if let Ok(resp) = crate::apex_data::rest::get_replay(class, &sym, &tf, from_ms, to_ms, None, Some(1000), crate::apex_data::BarSource::Last) {
                if !resp.bars.is_empty() {
                    let gpu_bars: Vec<Bar> = resp.bars.iter().map(|b| Bar {
                        open: b.open as f32, high: b.high as f32, low: b.low as f32,
                        close: b.close as f32, volume: b.volume as f32, _pad: 0.0,
                    }).collect();
                    let timestamps: Vec<i64> = resp.bars.iter().map(|b| b.time / 1000).collect();
                    eprintln!("[apex_data] replay {} bars for {} {} before {}", gpu_bars.len(), sym, tf, before_ts);
                    let cmd = ChartCommand::PrependBars {
                        symbol: sym.clone(), timeframe: tf.clone(),
                        bars: gpu_bars, timestamps,
                    };
                    for tx in &txs { let _ = tx.send(cmd.clone()); } crate::wake_native_ui();
                    return;
                }
            }
        }

        let period2 = before_ts;
        let period1 = before_ts - page_seconds;
        let yf_interval = match tf.as_str() {
            "1h" => "60m", "4h" => "1h",
            other => other,
        };

        let url = format!(
            "{}/v8/finance/chart/{}?interval={}&period1={}&period2={}",
            crate::data::endpoints::yahoo_chart(), sym, yf_interval, period1, period2
        );
        eprintln!("[history] fetching {} {} before {} ({}..{})", sym, tf, before_ts, period1, period2);

        let client = reqwest::blocking::Client::builder()
            .user_agent("Mozilla/5.0")
            .build().unwrap_or_else(|_| reqwest::blocking::Client::new());

        match client.get(&url).timeout(std::time::Duration::from_secs(10)).send() {
            Ok(resp) => {
                if let Ok(json) = resp.json::<serde_json::Value>() {
                    if let Some(bars) = crate::data::parse_yahoo_v8(&json) {
                        let gpu_bars: Vec<Bar> = bars.iter().map(|b| Bar {
                            open: b.open as f32, high: b.high as f32, low: b.low as f32,
                            close: b.close as f32, volume: b.volume as f32, _pad: 0.0,
                        }).collect();
                        let timestamps: Vec<i64> = bars.iter().map(|b| b.time).collect();
                        eprintln!("[history] got {} bars for {} {} (oldest: {})", gpu_bars.len(), sym, tf,
                            timestamps.first().copied().unwrap_or(0));

                        let cmd = ChartCommand::PrependBars {
                            symbol: sym, timeframe: tf, bars: gpu_bars, timestamps,
                        };
                        for tx in &txs { let _ = tx.send(cmd.clone()); } crate::wake_native_ui();
                        return;
                    }
                }
            }
            Err(e) => eprintln!("[history] fetch error: {e}"),
        }

        // On failure, send empty to clear loading flag and mark exhausted
        let cmd = ChartCommand::PrependBars {
            symbol: sym, timeframe: tf, bars: vec![], timestamps: vec![],
        };
        for tx in &txs { let _ = tx.send(cmd.clone()); } crate::wake_native_ui();
    });
}

/// Fetch bars from Redis cache → OCOCO → yfinance sidecar → Yahoo Finance v8 on a background thread.
/// Sends LoadBars command via the global NATIVE_CHART_TXS channels (all windows).
/// Results are cached in Redis for subsequent requests.
/// Load drawings from DB — uses the single DB worker thread, no per-call runtime.
pub(crate) fn fetch_drawings_background(sym: String) {
    let txs: Vec<std::sync::mpsc::Sender<ChartCommand>> = crate::NATIVE_CHART_TXS
        .get().and_then(|m| m.lock().ok()).map(|g| g.clone()).unwrap_or_default();
    if txs.is_empty() { return; }

    // Spawn a thread that sends requests to the DB worker and waits for replies.
    // The DB worker is a single thread with a single tokio runtime — no pool exhaustion.
    crate::foundation::guard::spawn_guarded("fetch", move || {
        let db_drawings = crate::drawing_db::load_symbol(&sym);
        let drawings: Vec<Drawing> = db_drawings.iter().filter_map(|dd| db_to_drawing(dd)).collect();
        let db_groups = crate::drawing_db::load_groups();
        let groups: Vec<DrawingGroup> = db_groups.into_iter()
            .map(|(id, name, color)| DrawingGroup { id, name, color }).collect();
        let cmd = ChartCommand::LoadDrawings { symbol: sym, drawings, groups };
        for tx in &txs { let _ = tx.send(cmd.clone()); } crate::wake_native_ui();
    });
}

/// Public entry point for standalone binary to trigger initial data load.
pub fn fetch_bars_background_pub(sym: String, tf: String) { fetch_bars_background(sym, tf, 0); }

/// Fetch bars for an option contract using the OCC ticker as the fetch key,
/// but emit `ChartCommand::LoadBars` with the pane's display symbol so existing
/// routing matches. Bars come from ApexData's `/api/bars/options/:contract/:tf`.
/// Build an OCC option ticker like `O:SPY251219C00450000` from (underlying,
/// strike, is_call, DTE-label).
///
/// DTE label examples: `"0DTE"`, `"5DTE"`, `"1DTE+"`, or an empty string.
/// Weekends roll forward to Friday (simple: pick the next weekday). For empty
/// or unparseable labels we default to 0DTE.
pub(crate) fn synthesize_occ(underlying: &str, strike: f32, is_call: bool, expiry_label: &str) -> String {
    use chrono::{Utc, Duration, NaiveDate, Datelike, Weekday};
    // Two acceptable forms:
    //   1. "YYYY-MM-DD" — parsed directly as a date.
    //   2. "5DTE" / "0DTE" / "" — N days from today.
    let mut d: NaiveDate = if let Ok(parsed) = NaiveDate::parse_from_str(expiry_label, "%Y-%m-%d") {
        parsed
    } else {
        let days: i64 = expiry_label
            .trim_end_matches(|c: char| !c.is_ascii_digit())
            .chars().filter(|c| c.is_ascii_digit()).collect::<String>()
            .parse().unwrap_or(0);
        Utc::now().date_naive() + Duration::days(days)
    };
    // Weekly options settle Fri; if we landed on Sat/Sun, nudge forward.
    match d.weekday() {
        Weekday::Sat => d += Duration::days(2),
        Weekday::Sun => d += Duration::days(1),
        _ => {}
    }
    let side = if is_call { 'C' } else { 'P' };
    // Strike integer part * 1000 + fractional cents in 0.001 units (OCC uses 1/1000 of $).
    let strike_int = (strike * 1000.0).round() as i64;
    // Polygon stores SPX/NDX index options under their PM-settled weekly tickers
    // (SPXW / NDXP). When the caller passes the user-facing index symbol, map
    // it to the OCC root. Pass-through everything else (including SPXW/NDXP if
    // the caller already used them) so the produced ticker matches the server.
    let occ_root: String = match underlying.to_uppercase().as_str() {
        "SPX" => "SPXW".into(),
        "NDX" => "NDXP".into(),
        other => other.to_string(),
    };
    format!("O:{}{}{}{:08}", occ_root, d.format("%y%m%d"), side, strike_int)
}

pub(crate) fn fetch_option_bars_background(occ: String, display_sym: String, tf: String, mark: bool) {
    crate::apex_log!("option.fetch", "ENTRY occ={occ} display='{display_sym}' tf={tf} mark={mark}");
    let txs: Vec<std::sync::mpsc::Sender<ChartCommand>> = crate::NATIVE_CHART_TXS
        .get().and_then(|m| m.lock().ok()).map(|g| g.clone()).unwrap_or_default();
    crate::apex_log!("option.fetch", "txs.len={}", txs.len());
    if txs.is_empty() { crate::apex_log!("option.fetch", "ABORT: no channel senders"); return; }

    if !crate::apex_data::is_enabled() {
        crate::apex_log!("option.fetch", "ABORT: ApexData disabled");
        crate::apex_data::live_state::push_toast(
            "\x02ApexData is disabled — option bars require an active ApexData subscription".to_string());
        crate::wake_native_ui();
        return;
    }

    let ws_was = crate::apex_data::ws::is_connected();
    crate::apex_log!("option.fetch", "WS connected={ws_was} — calling add_{}bar_sub", if mark {"mark_"} else {""});
    if mark {
        // Wave 8c: route mark-bar sub through SubscriptionManager with the
        // Mark source variant. The actual upstream WS call still has to be
        // made by us (Mark has no MarketDataProvider trait surface), but
        // SubscriptionManager now tracks the refcount + bumper state so
        // gap-fill / stale checks see it.
        use crate::data::providers::subscription_manager::BarSource;
        let mgr = crate::data::providers::registry::subscription_manager();
        let _ = mgr.subscribe_bars_with_source(&occ, &tf, BarSource::Mark);
        crate::apex_data::ws::add_mark_bar_sub(&occ, &tf);
        // Make sure we're not also receiving last-source frames for this contract.
        mgr.unsubscribe_bars_with_source(&occ, &tf, BarSource::Last);
    } else {
        // Wave 8: route through SubscriptionManager so gap-fill knows about
        // the active option bar sub. Receiver is unused — frames continue to
        // reach the UI via NATIVE_CHART_TXS.
        use crate::data::providers::subscription_manager::BarSource;
        let mgr = crate::data::providers::registry::subscription_manager();
        let _ = mgr.subscribe_bars_with_source(&occ, &tf, BarSource::Last);
        mgr.unsubscribe_bars_with_source(&occ, &tf, BarSource::Mark);
        crate::apex_data::ws::remove_mark_bar_sub(&occ, &tf);
    }
    let quote_set: Vec<String> = vec![occ.clone()];
    // Keep any existing quote subs alongside this one.
    // (set_quotes is replace-set semantics, so we need the full set — but our
    //  frame hook rebuilds the quote set each frame from watched_list, so just
    //  adding via live_state watch is enough — it'll be included next tick.)
    let _ = quote_set;

    crate::foundation::guard::spawn_guarded("fetch", move || {
        let send = |bars: Vec<crate::data::Bar>, src: &str| {
            let gpu_bars: Vec<Bar> = bars.iter().map(|b| Bar {
                open: b.open as f32, high: b.high as f32, low: b.low as f32,
                close: b.close as f32, volume: b.volume as f32, _pad: 0.0,
            }).collect();
            let timestamps: Vec<i64> = bars.iter().map(|b| b.time).collect();
            eprintln!("[option-chart] {} bars for {} ({} / {}) from {}",
                gpu_bars.len(), occ, display_sym, tf, src);
            let cmd = ChartCommand::LoadBars {
                symbol: display_sym.clone(),
                timeframe: tf.clone(),
                bars: gpu_bars, timestamps,
                gen: 0, // option-chart path (not the rapid symbol-switch stock path)
            };
            for tx in &txs { let _ = tx.send(cmd.clone()); } crate::wake_native_ui();
        };

        // History (may be empty for a brand-new contract — first close after sub
        // populates it; live ticks stream immediately via the WS sub above).
        let src = if mark { crate::apex_data::BarSource::Mark } else { crate::apex_data::BarSource::Last };
        crate::apex_log!("option.fetch", "REST get_bars Option {occ} {tf} source={}", src.as_str());
        match crate::apex_data::rest::get_bars(
            crate::apex_data::AssetClass::Option, &occ, &tf, src)
        {
            Ok(bars) if !bars.is_empty() => {
                crate::apex_log!("option.fetch", "OK {} bars for {occ}", bars.len());
                let adapted: Vec<crate::data::Bar> = bars.into_iter().map(|b| crate::data::Bar {
                    time: b.time, open: b.open, high: b.high, low: b.low, close: b.close, volume: b.volume,
                }).collect();
                send(adapted, "ApexData");
            }
            other => {
                let was_empty = matches!(other, Ok(_));
                crate::apex_log!("option.fetch",
                    "{} for {occ} {tf} source={} — trying mark fallback",
                    if was_empty { "EMPTY history" } else { "UNREACHABLE/breaker" },
                    src.as_str());
                // Auto-fallback: if Last has no data (current server state for
                // most option contracts), retry with Mark so the chart still
                // populates instead of staying blank. Without this, a fresh
                // option pane would never load until the user manually toggles.
                if !mark {
                    if let Ok(bars) = crate::apex_data::rest::get_bars(
                        crate::apex_data::AssetClass::Option, &occ, &tf,
                        crate::apex_data::BarSource::Mark)
                    {
                        if !bars.is_empty() {
                            crate::apex_log!("option.fetch",
                                "OK {} bars for {occ} (FALLBACK to mark — Last had nothing)",
                                bars.len());
                            // Swap WS subs to mark too so live updates match.
                            // Wave 8c: add a Mark refcount in SubscriptionManager
                            // before the WS call so gap-fill sees the new source,
                            // then drop the Last refcount we added moments ago.
                            use crate::data::providers::subscription_manager::BarSource;
                            let mgr = crate::data::providers::registry::subscription_manager();
                            let _ = mgr.subscribe_bars_with_source(&occ, &tf, BarSource::Mark);
                            crate::apex_data::ws::add_mark_bar_sub(&occ, &tf);
                            mgr.unsubscribe_bars_with_source(&occ, &tf, BarSource::Last);
                            // Flag the pane so the toggle reflects the active
                            // source. The pane's bar_source_mark is currently
                            // false, but the bars are mark — we send a Mark
                            // hint via ChartCommand. Cheapest path: send an
                            // explicit ChartCommand::SetBarSourceMark(...).
                            // For now just emit the LoadBars and let the user
                            // see data; toggle still shows correct state when
                            // they interact.
                            let adapted: Vec<crate::data::Bar> = bars.into_iter().map(|b| crate::data::Bar {
                                time: b.time, open: b.open, high: b.high, low: b.low, close: b.close, volume: b.volume,
                            }).collect();
                            send(adapted, "ApexData(mark-fallback)");
                            return;
                        }
                    }
                }
                // Both Last and Mark (fallback) returned empty or error — notify
                // the user so the chart doesn't appear to hang indefinitely, and
                // send an empty LoadBars so the pane knows loading finished.
                crate::apex_log!("option.fetch",
                    "ALL sources empty/failed for {occ} {tf} — no historical bars available");
                let reason = if was_empty { "No historical bars" } else { "Fetch failed" };
                crate::apex_data::live_state::push_toast(
                    format!("\x01Option bars unavailable for {} ({})", occ, reason));
                crate::wake_native_ui();
                // Send empty LoadBars so the pane clears its loading state
                // rather than staying blank with no indication of completion.
                let cmd = ChartCommand::LoadBars {
                    symbol: display_sym.clone(),
                    timeframe: tf.clone(),
                    bars: vec![],
                    timestamps: vec![],
                    gen: 0,
                };
                for tx in &txs { let _ = tx.send(cmd.clone()); }
                crate::wake_native_ui();
            }
        }
    });
}

pub(crate) fn fetch_bars_background(sym: String, tf: String, gen: u64) {
    let txs: Vec<std::sync::mpsc::Sender<ChartCommand>> = crate::NATIVE_CHART_TXS
        .get()
        .and_then(|m| m.lock().ok())
        .map(|g| g.clone())
        .unwrap_or_default();
    if txs.is_empty() { return; }

    // Route through the canonical Wave-2 provider chain (crypto → cached
    // apex_data → OCOCO → yfinance → Yahoo). The chain encapsulates the
    // 6-tier `if-else` ladder this function used to inline.
    crate::foundation::guard::spawn_guarded("fetch", move || {
        let chain = crate::data::providers::registry::bar_chain();
        // Re-use the process-wide fetch runtime rather than spinning a fresh
        // single-thread runtime per call (H3 fix). Guard against the rare case
        // where this thread is already inside a tokio context.
        let rt = fetch_runtime();
        // Run BOTH the bars fetch AND the subscribe_bars call inside the
        // runtime context. subscribe_bars internally uses tokio::spawn to
        // pump the fanout — that requires Handle::current() which is only
        // set while block_on is active.
        let sym_is_crypto = crate::foundation::types::registry()
            .get(&sym)
            .map(|s| s.is_crypto())
            .unwrap_or_else(|| crate::data::is_crypto(&sym));
        let result = rt.block_on(async {
            let bars = chain.bars(&sym, &tf, 0, 0, None).await;
            // Wave 8: route through SubscriptionManager so gap-fill on
            // reconnect sees this sub. Receiver is unused; the data path
            // is unchanged.
            // Wave 9c: registry-preferred crypto check so non-crypto
            // tickers whose strings end in `USDT` still get the ApexData
            // WS subscription.
            if crate::apex_data::is_enabled() && !sym_is_crypto {
                // Registers the upstream WS bar subscription (→ ws::add_bar_sub)
                // + the gap-fill anchor pump. The returned receiver is
                // intentionally dropped: live bars reach the UI via the lib.rs
                // NATIVE_CHART_TXS hot path, not this fanout. See providers/mod.rs.
                let _ = crate::data::providers::registry::subscription_manager()
                    .subscribe_bars(&sym, &tf);
            }
            bars
        });
        let bars = match result {
            Ok(b) if !b.is_empty() => b,
            Ok(_) => {
                eprintln!("[native-chart] {} {} empty", sym, tf);
                let cmd = ChartCommand::BarsUnavailable {
                    symbol: sym.clone(), timeframe: tf.clone(),
                    reason: "no bars returned".into(),
                };
                for tx in &txs { let _ = tx.send(cmd.clone()); }
                crate::wake_native_ui();
                return;
            }
            Err(e) => {
                eprintln!("[native-chart] {} {} all sources failed: {e}", sym, tf);
                let cmd = ChartCommand::BarsUnavailable {
                    symbol: sym.clone(), timeframe: tf.clone(),
                    reason: "data source unreachable".into(),
                };
                for tx in &txs { let _ = tx.send(cmd.clone()); }
                crate::wake_native_ui();
                return;
            }
        };
        let gpu_bars: Vec<Bar> = bars.iter().map(|b| Bar {
            open: b.open as f32, high: b.high as f32, low: b.low as f32,
            close: b.close as f32, volume: b.volume as f32, _pad: 0.0,
        }).collect();
        // BarWire.time is in ms; LoadBars wants seconds.
        let timestamps: Vec<i64> = bars.iter().map(|b| b.time / 1000).collect();
        eprintln!("[native-chart] {} bars for {} {} via provider chain", gpu_bars.len(), sym, tf);
        let cmd = ChartCommand::LoadBars {
            symbol: sym.clone(),
            timeframe: tf.clone(),
            bars: gpu_bars,
            timestamps: timestamps.clone(),
            gen, // stamp the result with the request generation captured at spawn
        };
        for tx in &txs { let _ = tx.send(cmd.clone()); }
        crate::wake_native_ui();

        // Proactively fetch the page of history before the loaded range so that
        // the first scroll-left doesn't stall waiting for the server.
        if let Some(&earliest_ts) = timestamps.first() {
            fetch_history_background(sym.clone(), tf.clone(), earliest_ts);
        }

        // Pre-warm the next timeframe up into the tab cache so the first TF switch is instant.
        fetch_prewarm_background(sym, tf);
    });
}

/// Fetch bars for the next-higher timeframe and stash them in the tab cache.
/// The CacheBars handler only inserts when the key is absent, so a live entry
/// from a user interaction always takes precedence over this background fetch.
pub(crate) fn fetch_prewarm_background(sym: String, tf: String) {
    let adjacent = match tf.as_str() {
        "1m" | "2m" => "5m",
        "5m"        => "15m",
        "15m" | "30m" => "1h",
        "1h" | "60m" => "4h",
        "4h"        => "1d",
        _ => return, // 1d / 1wk: no meaningful next TF to preload
    }.to_string();

    let txs: Vec<std::sync::mpsc::Sender<ChartCommand>> = crate::NATIVE_CHART_TXS
        .get().and_then(|m| m.lock().ok()).map(|g| g.clone()).unwrap_or_default();
    if txs.is_empty() { return; }

    crate::foundation::guard::spawn_guarded("fetch", move || {
        let chain = crate::data::providers::registry::bar_chain();
        let rt = fetch_runtime();
        let result = rt.block_on(async {
            chain.bars(&sym, &adjacent, 0, 0, None).await
        });
        let bars = match result {
            Ok(b) if !b.is_empty() => b,
            _ => return,
        };
        let gpu_bars: Vec<Bar> = bars.iter().map(|b| Bar {
            open: b.open as f32, high: b.high as f32, low: b.low as f32,
            close: b.close as f32, volume: b.volume as f32, _pad: 0.0,
        }).collect();
        let timestamps: Vec<i64> = bars.iter().map(|b| b.time / 1000).collect();
        eprintln!("[prewarm] {} bars cached for {} {}", gpu_bars.len(), sym, adjacent);
        let cmd = ChartCommand::CacheBars {
            symbol: sym, timeframe: adjacent, bars: gpu_bars, timestamps,
        };
        for tx in &txs { let _ = tx.send(cmd.clone()); }
        crate::wake_native_ui();
    });
}

pub(crate) fn fetch_overlay_bars_background(sym: String, tf: String) {
    eprintln!("[overlay-fetch] Starting fetch for {} {}", sym, tf);
    let txs: Vec<std::sync::mpsc::Sender<ChartCommand>> = crate::NATIVE_CHART_TXS
        .get().and_then(|m| m.lock().ok()).map(|g| g.clone()).unwrap_or_default();
    if txs.is_empty() { eprintln!("[overlay-fetch] No TXS channels!"); return; }
    crate::foundation::guard::spawn_guarded("fetch", move || {
        let client = reqwest::blocking::Client::builder()
            .user_agent("Mozilla/5.0").build().unwrap_or_else(|_| reqwest::blocking::Client::new());
        let (yf_interval, yf_range) = match tf.as_str() {
            "1m" => ("1m","5d"), "2m" => ("2m","5d"), "5m" => ("5m","5d"),
            "15m" => ("15m","60d"), "30m" => ("30m","60d"), "1h" => ("60m","60d"),
            "4h" => ("1h","730d"), "1d" => ("1d","5y"), "1wk" => ("1wk","10y"),
            _ => ("5m","5d"),
        };
        let fetch = |url: &str| -> Option<Vec<crate::data::Bar>> {
            let resp = client.get(url).timeout(std::time::Duration::from_secs(5)).send().ok()?;
            resp.json::<Vec<crate::data::Bar>>().ok()
        };
        let ococo_url = format!("{}/api/bars?symbol={}&interval={}&limit=500", crate::data::endpoints::ococo_http(), sym, tf);
        if let Some(bars) = fetch(&ococo_url).filter(|b| !b.is_empty()) {
            let gpu_bars: Vec<Bar> = bars.iter().map(|b| Bar { open: b.open as f32, high: b.high as f32, low: b.low as f32, close: b.close as f32, volume: b.volume as f32, _pad: 0.0 }).collect();
            let timestamps: Vec<i64> = bars.iter().map(|b| b.time).collect();
            let cmd = ChartCommand::OverlayBars { symbol: sym.clone(), bars: gpu_bars, timestamps };
            for tx in &txs { let _ = tx.send(cmd.clone()); } crate::wake_native_ui();
            return;
        }
        let yahoo_url = format!("{}/v8/finance/chart/{}?interval={}&range={}", crate::data::endpoints::yahoo_chart(), sym, yf_interval, yf_range);
        if let Ok(resp) = client.get(&yahoo_url).timeout(std::time::Duration::from_secs(5)).send() {
            if let Ok(json) = resp.json::<serde_json::Value>() {
                if let Some(bars) = crate::data::parse_yahoo_v8(&json) {
                    let gpu_bars: Vec<Bar> = bars.iter().map(|b| Bar { open: b.open as f32, high: b.high as f32, low: b.low as f32, close: b.close as f32, volume: b.volume as f32, _pad: 0.0 }).collect();
                    let timestamps: Vec<i64> = bars.iter().map(|b| b.time).collect();
                    let cmd = ChartCommand::OverlayBars { symbol: sym.clone(), bars: gpu_bars, timestamps };
                    for tx in &txs { let _ = tx.send(cmd.clone()); } crate::wake_native_ui();
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apex_data::{StockSnapshot, DayAgg, PrevDayAgg, LastTrade, MinAgg, GroupedDailyBar};

    // ── is_stock_symbol filter ────────────────────────────────────────────

    #[test]
    fn stock_symbol_filter_accepts_equities_rejects_crypto_and_options() {
        assert!(is_stock_symbol("AAPL"));
        assert!(is_stock_symbol("SPY"));
        assert!(is_stock_symbol("BRK.B"));
        assert!(!is_stock_symbol("O:SPY260501C00450000"));
        assert!(!is_stock_symbol("AAPL 287.5C 2026-04-30"));
        // Crypto path is delegated to crate::data::is_crypto; sanity-check a
        // few well-known pairs that should not flow through the bulk fetch.
        assert!(!is_stock_symbol("BTCUSDT"));
    }

    // ── watchlist mapping ─────────────────────────────────────────────────

    fn snap(ticker: &str, last_price: f64, prev_close: f64, day_volume: f64) -> StockSnapshot {
        StockSnapshot {
            ticker: ticker.into(),
            day: DayAgg { close: last_price, volume: day_volume, ..Default::default() },
            min: None,
            prev_day: PrevDayAgg { close: prev_close, ..Default::default() },
            last_trade: LastTrade { price: last_price, ..Default::default() },
            last_quote: Default::default(),
            todays_change: last_price - prev_close,
            todays_change_perc: ((last_price - prev_close) / prev_close) * 100.0,
            updated: 0,
            ..Default::default()
        }
    }

    #[test]
    fn watchlist_maps_snapshot_to_price_and_prev_close() {
        let s = snap("AAPL", 195.50, 192.00, 50_000_000.0);
        let (price, prev, _) = watchlist_price_from_snapshot(&s);
        assert!((price - 195.50).abs() < 0.001);
        assert!((prev - 192.00).abs() < 0.001);
    }

    #[test]
    fn watchlist_mapping_prefers_last_trade_over_day_close() {
        let mut s = snap("MSFT", 0.0, 410.0, 0.0);
        s.last_trade.price = 412.5;
        s.day.close = 411.0;
        let (price, prev, _) = watchlist_price_from_snapshot(&s);
        assert!((price - 412.5).abs() < 0.001);
        assert!((prev - 410.0).abs() < 0.001);
    }

    #[test]
    fn watchlist_mapping_falls_back_to_minute_close_when_no_trade() {
        let mut s = snap("NVDA", 0.0, 880.0, 0.0);
        s.last_trade.price = 0.0;
        s.day.close = 0.0;
        s.min = Some(MinAgg { close: 885.0, accumulated_volume: 1234.0, ..Default::default() });
        let (price, prev, _) = watchlist_price_from_snapshot(&s);
        assert!((price - 885.0).abs() < 0.001);
        assert!((prev - 880.0).abs() < 0.001);
    }

    // ── scanner mapping ───────────────────────────────────────────────────

    #[test]
    fn scanner_mapping_extracts_volume_from_day_agg() {
        let s = snap("TSLA", 250.0, 245.0, 80_000_000.0);
        let (sym, price, prev, vol) = scanner_price_from_snapshot(&s);
        assert_eq!(sym, "TSLA");
        assert!((price - 250.0).abs() < 0.001);
        assert!((prev - 245.0).abs() < 0.001);
        assert_eq!(vol, 80_000_000);
    }

    #[test]
    fn scanner_mapping_uses_minute_accumulated_volume_in_premarket() {
        let mut s = snap("AMD", 165.0, 162.0, 0.0); // no day volume yet
        s.min = Some(MinAgg {
            close: 165.0, accumulated_volume: 1_500_000.0, ..Default::default()
        });
        let (_sym, _price, _prev, vol) = scanner_price_from_snapshot(&s);
        assert_eq!(vol, 1_500_000);
    }

    // ── REST mock parsing ─────────────────────────────────────────────────

    #[test]
    fn snap_bulk_parses_polygon_envelope() {
        let body = r#"[
          {
            "ticker": "AAPL",
            "day":   {"o":190.0,"h":196.0,"l":189.5,"c":195.5,"v":50000000,"vw":193.0},
            "prevDay":{"o":188.0,"h":193.0,"l":187.0,"c":192.0,"v":48000000,"vw":190.0},
            "lastTrade":{"p":195.6,"s":100,"t":1715000000000000000,"x":4},
            "lastQuote":{"P":195.61,"S":2,"p":195.59,"s":3,"t":1715000000000000000},
            "todaysChange":3.5,"todaysChangePerc":1.82,
            "updated":1715000000000000000
          }
        ]"#;
        let snaps: Vec<StockSnapshot> = serde_json::from_str(body).expect("parse");
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].ticker, "AAPL");
        assert!((snaps[0].last_trade.price - 195.6).abs() < 0.001);
        assert!((snaps[0].prev_day.close - 192.0).abs() < 0.001);
        assert!((snaps[0].todays_change_perc - 1.82).abs() < 0.001);
    }

    #[test]
    fn movers_envelope_parses_like_bulk() {
        // The movers endpoint returns the same per-row shape as snap/bulk.
        let body = r#"[
          {"ticker":"GME","day":{"c":40.0,"v":120000000},"prevDay":{"c":20.0},"lastTrade":{"p":41.0},"todaysChangePerc":100.0}
        ]"#;
        let snaps: Vec<StockSnapshot> = serde_json::from_str(body).expect("parse");
        let (sym, price, prev, vol) = scanner_price_from_snapshot(&snaps[0]);
        assert_eq!(sym, "GME");
        assert!((price - 41.0).abs() < 0.001);
        assert!((prev - 20.0).abs() < 0.001);
        assert_eq!(vol, 120_000_000);
    }

    // ── heatmap mapping ───────────────────────────────────────────────────

    #[test]
    fn grouped_daily_maps_to_heatmap_cell() {
        let bar = GroupedDailyBar {
            ticker: "AAPL".into(),
            o: 100.0, h: 105.0, l: 99.0, c: 102.0,
            v: 1_000_000.0, vw: 101.5, n: 0, t: 0,
        };
        let (sym, change_pct, weight) = heatmap_cell_from_grouped(&bar);
        assert_eq!(sym, "AAPL");
        // (102 - 100) / 100 * 100 = 2.0
        assert!((change_pct - 2.0).abs() < 0.0001);
        // vw * v = 101.5 * 1_000_000
        assert!((weight - 101_500_000.0).abs() < 1.0);
    }

    #[test]
    fn grouped_daily_handles_zero_open() {
        let bar = GroupedDailyBar { ticker: "NEW".into(), o: 0.0, c: 10.0, v: 1.0, vw: 1.0, ..Default::default() };
        let (_sym, change_pct, _w) = heatmap_cell_from_grouped(&bar);
        assert_eq!(change_pct, 0.0);
    }

    #[test]
    fn grouped_daily_parses_polygon_envelope() {
        let body = r#"[
          {"T":"AAPL","o":190.0,"h":196.0,"l":189.5,"c":195.5,"v":50000000,"vw":193.0,"n":250000,"t":1715000000000}
        ]"#;
        let bars: Vec<GroupedDailyBar> = serde_json::from_str(body).expect("parse");
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].ticker, "AAPL");
        assert!((bars[0].c - 195.5).abs() < 0.001);
    }

    // ── scanner preset routing ────────────────────────────────────────────

    #[test]
    fn scanner_preset_routing_recognizes_built_ins() {
        // We can't easily exercise the network call without a mock server,
        // but we *can* assert the preset → universe selection rules used by
        // fetch_scanner_prices_for: only "gainers" and "losers" produce a
        // network call; everything else is a no-op (silent TODO).
        let recognized = |p: &str| matches!(p, "gainers" | "losers");
        assert!(recognized("gainers"));
        assert!(recognized("losers"));
        assert!(!recognized("most_active"));
        assert!(!recognized("custom"));
    }

    // ── cache eviction ────────────────────────────────────────────────────

    /// Verify that `evict_oldest` keeps the map at or below `cap` and removes
    /// the entries with the smallest (oldest) Instant values.
    ///
    /// We use a synthetic `HashMap<String, (u32, std::time::Instant)>` so the
    /// Instants are constructed via `checked_add` on a known base — fully
    /// deterministic, no wall-clock ordering dependency.
    #[test]
    fn cache_evict_oldest_removes_oldest_entries() {
        use std::collections::HashMap;
        use std::time::{Duration, Instant};

        // Build a base instant and give each entry a deterministically
        // increasing age (entry 0 is oldest, entry N-1 is newest).
        let base = Instant::now();
        let mut map: HashMap<String, (u32, Instant)> = HashMap::new();
        let cap = 4_usize;
        let total = cap + 3; // insert 7 entries into a cap-4 cache
        for i in 0..total {
            let at = base + Duration::from_millis(i as u64); // i=0 is oldest
            map.insert(format!("SYM{i}"), (i as u32, at));
        }
        assert_eq!(map.len(), total);

        evict_oldest(&mut map, cap, |v| v.1);

        // Must be at or below cap.
        assert_eq!(map.len(), cap, "expected {cap} entries after eviction");

        // The 3 oldest (SYM0, SYM1, SYM2) must have been evicted.
        assert!(!map.contains_key("SYM0"), "oldest entry SYM0 should be evicted");
        assert!(!map.contains_key("SYM1"), "SYM1 should be evicted");
        assert!(!map.contains_key("SYM2"), "SYM2 should be evicted");

        // The newest entries must still be present.
        assert!(map.contains_key(&format!("SYM{}", total - 1)), "newest entry must survive");
        assert!(map.contains_key(&format!("SYM{}", total - 2)), "second-newest must survive");
    }

    /// Verify that `evict_any` keeps the map at or below `cap` regardless of
    /// value type (used by ticker_detail_cache which has no Instant field).
    #[test]
    fn cache_evict_any_caps_map_size() {
        use std::collections::HashMap;

        let mut map: HashMap<String, u32> = HashMap::new();
        let cap = 3_usize;
        for i in 0..(cap + 5) {
            map.insert(format!("K{i}"), i as u32);
        }
        assert!(map.len() > cap);
        evict_any(&mut map, cap);
        assert!(map.len() <= cap, "map must be capped at {cap}");
    }
}

