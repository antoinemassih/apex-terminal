//! Fetch / IO helpers — HTTP, background thread data loaders.
//!
//! All functions spawn background threads and deliver results as `ChartCommand`s
//! via `crate::send_to_native_chart` or `crate::NATIVE_CHART_TXS`.

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use crate::chart_renderer::{ChartCommand, Bar};
use crate::chart_renderer::gpu::{OptionRow, ScanResult, APEXIB_URL, db_to_drawing};
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
    let url = format!("{}{}", APEXIB_URL, path);
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

pub(crate) fn fetch_chain_background(symbol: String, num_strikes: usize, dte: i32, underlying_price: f32) {
    let symbol = normalize_underlying(&symbol);
    std::thread::spawn(move || {
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
            };
            crate::send_to_native_chart(cmd);
        };

        // 0. ApexData — preferred. §5.4.c default filters (dte_max=14,
        //    strike_window_pct=10%) keep the payload small; §5.4.d chain_delta
        //    WS stream then pushes live updates into the local cache.
        if crate::apex_data::is_enabled() {
            use crate::apex_data::types::ChainQuery;

            // Subscribe once to the chain delta stream for this underlying.
            // Idempotent — dedup happens server-side by refcount.
            crate::apex_data::ws::set_chain(&[symbol.clone()]);
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
            if let Some(chain) = crate::apex_data::rest::get_chain_with(&symbol, &q) {
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
                    if let Some(past) = crate::apex_data::rest::get_chain_with(&symbol, &pq) {
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
            crate::apex_data::ws::add_bar_sub(&placeholder_occ, "1m");

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
                if let Some(chain) = crate::apex_data::rest::get_chain_with(&symbol, &q) {
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
            crate::apex_data::live_state::push_toast(format!(
                "No chain data for {} — not in server's tracked underlyings",
                symbol));
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

        // No simulated chain — fake strikes produce OCCs that don't exist upstream, which
        // means every click on a sim row silently fails to load bars. Send an empty chain
        // and a toast so the user knows the real feed is unavailable.
        crate::apex_log!("chain", "no real chain available for {} dte={} — sending empty", symbol, dte);
        crate::apex_data::live_state::push_toast(format!(
            "No chain data for {} — real feed unavailable", symbol));
        send_chain(vec![], vec![], underlying_price);
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
    std::thread::spawn(move || {
        // 0. ApexData — preferred. 0DTE strikes, wide strike band.
        if crate::apex_data::is_enabled() {
            if let Some(chain) = crate::apex_data::rest::get_chain(&symbol) {
                let spot = if underlying_price > 0.0 { underlying_price }
                           else {
                               crate::apex_data::live_state::get_snapshot(&symbol)
                                   .map(|s| s.last as f32).unwrap_or(0.0)
                           };
                let (calls, puts, _eff) = apex_data_chain_to_tuples(&chain.rows, 0, 75, spot);
                if !calls.is_empty() || !puts.is_empty() {
                    crate::send_to_native_chart(ChartCommand::OverlayChainData { symbol: symbol.clone(), calls, puts });
                    return;
                }
            }
        }

        let path = format!("/options/{}?strikeCount=150&dte=0", symbol);
        if let Some(json) = apexib_curl(&path) {
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
                crate::send_to_native_chart(ChartCommand::OverlayChainData { symbol, calls, puts });
                return;
            }
        }
        // No simulated fallback — send an empty overlay so the chart reflects reality
        // rather than drawing fabricated strikes over a real symbol.
        crate::send_to_native_chart(ChartCommand::OverlayChainData { symbol, calls: vec![], puts: vec![] });
    });
}

/// Fetch symbol search results from ApexIB in background.
pub(crate) fn fetch_search_background(query: String, source: String) {
    std::thread::spawn(move || {
        let client = apexib_client();

        let url = format!("{}/search/{}", APEXIB_URL, query);
        let mut results: Vec<(String, String)> = Vec::new();

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

/// Fetch daily previous close for all watchlist symbols (background thread).
/// Tries ApexIB first (bars endpoint), falls back to Yahoo Finance.
pub(crate) fn fetch_watchlist_prices(symbols: Vec<String>) {
    // Filter: this fn only knows how to fetch equities (Redis/ApexIB/Yahoo).
    // Option contracts (OCC tickers, "AAPL 287.5C 2026-04-30" labels) and
    // crypto pairs go through their own feeds — sending them here means a
    // silent 404 on Yahoo. Drop them before the fetch loop.
    let symbols: Vec<String> = symbols.into_iter()
        .filter(|s| {
            let s_upper = s.to_uppercase();
            // Crypto: ApexCrypto handles BTCUSDT etc.
            if crate::data::is_crypto(s) { return false; }
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
    std::thread::spawn(move || {
        let ib_client = apexib_client();
        let client = reqwest::blocking::Client::builder()
            .user_agent("Mozilla/5.0")
            .timeout(std::time::Duration::from_secs(5))
            .build().unwrap_or_else(|_| reqwest::blocking::Client::new());
        for sym in &symbols {
            // Try Redis cache first
            if let Some(bars) = crate::bar_cache::get(sym, "1d") {
                if bars.len() >= 2 {
                    let price = bars.last().map(|b| b.close as f32).unwrap_or(0.0);
                    let prev = bars[bars.len()-2].close as f32;
                    let cmd = ChartCommand::WatchlistPrice { symbol: sym.clone(), price, prev_close: prev };
                    crate::send_to_native_chart(cmd);
                    continue;
                }
            }

            // Try ApexIB bars endpoint
            let apexib_ok = (|| -> Option<()> {
                let url = format!("{}/bars/{}?interval=1d&limit=2", APEXIB_URL, sym);
                let resp = client.get(&url).send().ok()?;
                if !resp.status().is_success() { return None; }
                let json = resp.json::<serde_json::Value>().ok()?;
                let bars = json.as_array()?;
                if bars.len() < 2 { return None; }
                let prev = bars[0].get("close").and_then(|v| v.as_f64())? as f32;
                let price = bars[1].get("close").and_then(|v| v.as_f64())? as f32;
                let cmd = ChartCommand::WatchlistPrice { symbol: sym.clone(), price, prev_close: prev };
                crate::send_to_native_chart(cmd);
                Some(())
            })();

            if apexib_ok.is_some() { continue; }

            // Fallback: Yahoo Finance
            let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=1d&range=5d", sym);
            if let Ok(resp) = client.get(&url).send() {
                if let Ok(json) = resp.json::<serde_json::Value>() {
                    if let Some(bars) = crate::data::parse_yahoo_v8(&json) {
                        crate::bar_cache::set(sym, "1d", &bars);
                        if bars.len() >= 2 {
                            let price = bars.last().map(|b| b.close as f32).unwrap_or(0.0);
                            let prev = bars[bars.len()-2].close as f32;
                            let cmd = ChartCommand::WatchlistPrice { symbol: sym.clone(), price, prev_close: prev };
                            crate::send_to_native_chart(cmd);
                        }
                    }
                }
            }
        }
    });
}

/// Scanner universe — ~50 popular symbols to bulk-quote for scanner panels.
pub(crate) const SCANNER_UNIVERSE: &[&str] = &[
    "AAPL","MSFT","GOOGL","AMZN","TSLA","META","NVDA","AMD","NFLX","INTC",
    "SPY","QQQ","IWM","DIA","BABA","CRM","PYPL","SQ","SHOP","UBER",
    "COIN","SNAP","PLTR","RIVN","LCID","SOFI","NIO","MARA","RIOT","ROKU",
    "BA","DIS","JPM","GS","V","MA","WMT","COST","HD","LOW",
    "XOM","CVX","PFE","JNJ","UNH","MRK","ABBV","LLY","KO","PEP",
];

/// Fetch bulk quotes for the scanner universe in a background thread.
/// Sends ScannerPrice commands back to the chart event loop.
pub(crate) fn fetch_scanner_prices() {
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::builder()
            .user_agent("Mozilla/5.0")
            .timeout(std::time::Duration::from_secs(8))
            .build().unwrap_or_else(|_| reqwest::blocking::Client::new());

        for sym in SCANNER_UNIVERSE {
            // 1. Try Redis/bar cache
            if let Some(bars) = crate::bar_cache::get(sym, "1d") {
                if bars.len() >= 2 {
                    let price = bars.last().map(|b| b.close as f32).unwrap_or(0.0);
                    let prev = bars[bars.len()-2].close as f32;
                    let volume = bars.last().map(|b| b.volume as u64).unwrap_or(0);
                    crate::send_to_native_chart(ChartCommand::ScannerPrice {
                        symbol: sym.to_string(), price, prev_close: prev, volume,
                    });
                    continue;
                }
            }

            // 2. Try ApexIB
            let apexib_ok = (|| -> Option<()> {
                let url = format!("{}/bars/{}?interval=1d&limit=2", APEXIB_URL, sym);
                let resp = client.get(&url).send().ok()?;
                if !resp.status().is_success() { return None; }
                let json = resp.json::<serde_json::Value>().ok()?;
                let bars = json.as_array()?;
                if bars.len() < 2 { return None; }
                let prev = bars[0].get("close").and_then(|v| v.as_f64())? as f32;
                let price = bars[1].get("close").and_then(|v| v.as_f64())? as f32;
                let volume = bars[1].get("volume").and_then(|v| v.as_u64()).unwrap_or(0);
                crate::send_to_native_chart(ChartCommand::ScannerPrice {
                    symbol: sym.to_string(), price, prev_close: prev, volume,
                });
                Some(())
            })();
            if apexib_ok.is_some() { continue; }

            // 3. Fallback: Yahoo Finance v8
            let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=1d&range=5d", sym);
            if let Ok(resp) = client.get(&url).send() {
                if let Ok(json) = resp.json::<serde_json::Value>() {
                    if let Some(bars) = crate::data::parse_yahoo_v8(&json) {
                        crate::bar_cache::set(sym, "1d", &bars);
                        if bars.len() >= 2 {
                            let price = bars.last().map(|b| b.close as f32).unwrap_or(0.0);
                            let prev = bars[bars.len()-2].close as f32;
                            let volume = bars.last().map(|b| b.volume as u64).unwrap_or(0);
                            crate::send_to_native_chart(ChartCommand::ScannerPrice {
                                symbol: sym.to_string(), price, prev_close: prev, volume,
                            });
                        }
                    }
                }
            }
        }
    });
}


/// Fetch source bars for a cross-timeframe indicator on a background thread.
pub(crate) fn fetch_indicator_source(sym: String, tf: String, indicator_id: u32) {
    let txs: Vec<std::sync::mpsc::Sender<ChartCommand>> = crate::NATIVE_CHART_TXS
        .get().and_then(|m| m.lock().ok()).map(|g| g.clone()).unwrap_or_default();
    if txs.is_empty() { return; }
    std::thread::spawn(move || {
        // Try Redis cache first
        if let Some(bars) = crate::bar_cache::get(&sym, &tf) {
            if !bars.is_empty() {
                let timestamps: Vec<i64> = bars.iter().map(|b| b.time).collect();
                let gpu_bars: Vec<Bar> = bars.iter().map(|b| Bar {
                    open: b.open as f32, high: b.high as f32, low: b.low as f32,
                    close: b.close as f32, volume: b.volume as f32, _pad: 0.0,
                }).collect();
                let cmd = ChartCommand::IndicatorSourceBars { indicator_id, timeframe: tf.clone(), bars: gpu_bars, timestamps };
                for tx in &txs { let _ = tx.send(cmd.clone()); }
                return;
            }
        }
        // Fetch from Yahoo Finance
        let (yf_interval, yf_range) = match tf.as_str() {
            "1m" => ("1m","5d"), "5m" => ("5m","5d"), "15m" => ("15m","60d"),
            "30m" => ("30m","60d"), "1h" => ("60m","60d"), "4h" => ("1h","730d"),
            "1d" => ("1d","5y"), "1wk" => ("1wk","10y"), _ => ("5m","5d"),
        };
        let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{}?interval={}&range={}", sym, yf_interval, yf_range);
        let client = reqwest::blocking::Client::builder().user_agent("Mozilla/5.0").build().unwrap_or_else(|_| reqwest::blocking::Client::new());
        if let Ok(resp) = client.get(&url).timeout(std::time::Duration::from_secs(5)).send() {
            if let Ok(json) = resp.json::<serde_json::Value>() {
                if let Some(bars) = crate::data::parse_yahoo_v8(&json) {
                    crate::bar_cache::set(&sym, &tf, &bars);
                    let timestamps: Vec<i64> = bars.iter().map(|b| b.time).collect();
                    let gpu_bars: Vec<Bar> = bars.iter().map(|b| Bar {
                        open: b.open as f32, high: b.high as f32, low: b.low as f32,
                        close: b.close as f32, volume: b.volume as f32, _pad: 0.0,
                    }).collect();
                    let cmd = ChartCommand::IndicatorSourceBars { indicator_id, timeframe: tf, bars: gpu_bars, timestamps };
                    for tx in &txs { let _ = tx.send(cmd.clone()); }
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

    std::thread::spawn(move || {
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
            for tx in &txs { let _ = tx.send(cmd.clone()); }
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
            Some(resp) if !resp.bars.is_empty() => {
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
                for tx in &txs { let _ = tx.send(cmd.clone()); }
            }
            _ => {
                crate::apex_log!("option.history",
                    "EMPTY for {} {} — marking exhausted", occ, tf);
                // Empty result → PrependBars handler sets history_exhausted.
                let cmd = ChartCommand::PrependBars {
                    symbol: display_sym, timeframe: tf,
                    bars: vec![], timestamps: vec![],
                };
                for tx in &txs { let _ = tx.send(cmd.clone()); }
            }
        }
    });
}

pub(crate) fn fetch_history_background(sym: String, tf: String, before_ts: i64) {
    let txs: Vec<std::sync::mpsc::Sender<ChartCommand>> = crate::NATIVE_CHART_TXS
        .get().and_then(|m| m.lock().ok()).map(|g| g.clone()).unwrap_or_default();
    if txs.is_empty() { return; }

    std::thread::spawn(move || {
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
            if let Some(resp) = crate::apex_data::rest::get_replay(class, &sym, &tf, from_ms, to_ms, None, Some(1000), crate::apex_data::BarSource::Last) {
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
                    for tx in &txs { let _ = tx.send(cmd.clone()); }
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
            "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval={}&period1={}&period2={}",
            sym, yf_interval, period1, period2
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
                        for tx in &txs { let _ = tx.send(cmd.clone()); }
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
        for tx in &txs { let _ = tx.send(cmd.clone()); }
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
    std::thread::spawn(move || {
        let db_drawings = crate::drawing_db::load_symbol(&sym);
        let drawings: Vec<Drawing> = db_drawings.iter().filter_map(|dd| db_to_drawing(dd)).collect();
        let db_groups = crate::drawing_db::load_groups();
        let groups: Vec<DrawingGroup> = db_groups.into_iter()
            .map(|(id, name, color)| DrawingGroup { id, name, color }).collect();
        let cmd = ChartCommand::LoadDrawings { symbol: sym, drawings, groups };
        for tx in &txs { let _ = tx.send(cmd.clone()); }
    });
}

/// Public entry point for standalone binary to trigger initial data load.
pub fn fetch_bars_background_pub(sym: String, tf: String) { fetch_bars_background(sym, tf); }

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
        return;
    }

    let ws_was = crate::apex_data::ws::is_connected();
    crate::apex_log!("option.fetch", "WS connected={ws_was} — calling add_{}bar_sub", if mark {"mark_"} else {""});
    if mark {
        crate::apex_data::ws::add_mark_bar_sub(&occ, &tf);
        // Make sure we're not also receiving last-source frames for this contract.
        crate::apex_data::ws::remove_bar_sub(&occ, &tf);
    } else {
        crate::apex_data::ws::add_bar_sub(&occ, &tf);
        crate::apex_data::ws::remove_mark_bar_sub(&occ, &tf);
    }
    let mut quote_set: Vec<String> = vec![occ.clone()];
    // Keep any existing quote subs alongside this one.
    // (set_quotes is replace-set semantics, so we need the full set — but our
    //  frame hook rebuilds the quote set each frame from watched_list, so just
    //  adding via live_state watch is enough — it'll be included next tick.)
    let _ = quote_set;

    std::thread::spawn(move || {
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
            };
            for tx in &txs { let _ = tx.send(cmd.clone()); }
        };

        // History (may be empty for a brand-new contract — first close after sub
        // populates it; live ticks stream immediately via the WS sub above).
        let src = if mark { crate::apex_data::BarSource::Mark } else { crate::apex_data::BarSource::Last };
        crate::apex_log!("option.fetch", "REST get_bars Option {occ} {tf} source={}", src.as_str());
        match crate::apex_data::rest::get_bars(
            crate::apex_data::AssetClass::Option, &occ, &tf, src)
        {
            Some(bars) if !bars.is_empty() => {
                crate::apex_log!("option.fetch", "OK {} bars for {occ}", bars.len());
                let adapted: Vec<crate::data::Bar> = bars.into_iter().map(|b| crate::data::Bar {
                    time: b.time, open: b.open, high: b.high, low: b.low, close: b.close, volume: b.volume,
                }).collect();
                send(adapted, "ApexData");
            }
            other => {
                let was_empty = matches!(other, Some(_));
                crate::apex_log!("option.fetch",
                    "{} for {occ} {tf} source={} — trying mark fallback",
                    if was_empty { "EMPTY history" } else { "UNREACHABLE/breaker" },
                    src.as_str());
                // Auto-fallback: if Last has no data (current server state for
                // most option contracts), retry with Mark so the chart still
                // populates instead of staying blank. Without this, a fresh
                // option pane would never load until the user manually toggles.
                if !mark {
                    if let Some(bars) = crate::apex_data::rest::get_bars(
                        crate::apex_data::AssetClass::Option, &occ, &tf,
                        crate::apex_data::BarSource::Mark)
                    {
                        if !bars.is_empty() {
                            crate::apex_log!("option.fetch",
                                "OK {} bars for {occ} (FALLBACK to mark — Last had nothing)",
                                bars.len());
                            // Swap WS subs to mark too so live updates match.
                            crate::apex_data::ws::add_mark_bar_sub(&occ, &tf);
                            crate::apex_data::ws::remove_bar_sub(&occ, &tf);
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
            }
        }
    });
}

pub(crate) fn fetch_bars_background(sym: String, tf: String) {
    let txs: Vec<std::sync::mpsc::Sender<ChartCommand>> = crate::NATIVE_CHART_TXS
        .get()
        .and_then(|m| m.lock().ok())
        .map(|g| g.clone())
        .unwrap_or_default();
    if txs.is_empty() { return; }
    std::thread::spawn(move || {
        let send_bars = |bars: &[crate::data::Bar], src: &str| -> bool {
            if bars.is_empty() { return false; }
            let gpu_bars: Vec<Bar> = bars.iter().map(|b| Bar {
                open: b.open as f32, high: b.high as f32, low: b.low as f32,
                close: b.close as f32, volume: b.volume as f32, _pad: 0.0,
            }).collect();
            let timestamps: Vec<i64> = bars.iter().map(|b| b.time).collect();
            eprintln!("[native-chart] {} bars for {} {} from {}", gpu_bars.len(), sym, tf, src);
            let cmd = ChartCommand::LoadBars { symbol: sym.clone(), timeframe: tf.clone(), bars: gpu_bars, timestamps };
            for tx in &txs { let _ = tx.send(cmd.clone()); }
            true
        };

        let client = reqwest::blocking::Client::builder()
            .user_agent("Mozilla/5.0")
            .build().unwrap_or_else(|_| reqwest::blocking::Client::new());

        // 0. Crypto → ApexCrypto (skip local cache, ApexCrypto manages its own + Binance backfill)
        if crate::data::is_crypto(&sym) {
            let apex_url = format!("http://192.168.1.56:30840/api/bars/{}/{}", sym, tf);
            if let Ok(resp) = client.get(&apex_url).timeout(std::time::Duration::from_secs(5)).send() {
                if let Ok(bars) = resp.json::<Vec<crate::data::Bar>>() {
                    if !bars.is_empty() {
                        if send_bars(&bars, "ApexCrypto") { return; }
                    }
                }
            }
            // Crypto-only: don't fall through to Yahoo
            return;
        }

        // 0. ApexData — authoritative source (REST + WS live updates)
        if crate::apex_data::is_enabled() {
            let class = crate::apex_data::AssetClass::from_symbol(&sym);
            if let Some(bars) = crate::apex_data::rest::get_bars(class, &sym, &tf, crate::apex_data::BarSource::Last) {
                if !bars.is_empty() {
                    // ApexData's ChartBar has the same shape as our data::Bar (time in secs).
                    let adapted: Vec<crate::data::Bar> = bars.into_iter().map(|b| crate::data::Bar {
                        time: b.time, open: b.open, high: b.high, low: b.low, close: b.close, volume: b.volume,
                    }).collect();
                    crate::bar_cache::set(&sym, &tf, &adapted);
                    // Subscribe to live updates for this (symbol, tf).
                    crate::apex_data::ws::add_bar_sub(&sym, &tf);
                    if send_bars(&adapted, "ApexData") { return; }
                }
            }
        }

        // 1. Redis cache — instant (stocks only)
        if let Some(cached) = crate::bar_cache::get(&sym, &tf) {
            if send_bars(&cached, "Redis cache") { return; }
        }

        // 2. OCOCO (InfluxDB cache)
        let ococo_url = format!("http://192.168.1.60:30300/api/bars?symbol={}&interval={}&limit=500", sym, tf);
        if let Ok(resp) = client.get(&ococo_url).timeout(std::time::Duration::from_secs(2)).send() {
            if let Ok(bars) = resp.json::<Vec<crate::data::Bar>>() {
                if !bars.is_empty() {
                    crate::bar_cache::set(&sym, &tf, &bars);
                    if send_bars(&bars, "OCOCO") { return; }
                }
            }
        }

        // 2. yfinance sidecar
        let (yf_interval, yf_range) = match tf.as_str() {
            "1m" => ("1m","5d"), "2m" => ("2m","5d"), "5m" => ("5m","5d"),
            "15m" => ("15m","60d"), "30m" => ("30m","60d"), "1h" => ("60m","60d"),
            "4h" => ("1h","730d"), "1d" => ("1d","5y"), "1wk" => ("1wk","10y"),
            _ => ("5m","5d"),
        };
        let yf_url = format!("http://127.0.0.1:8777/bars?symbol={}&interval={}&period={}", sym, yf_interval, yf_range);
        if let Ok(resp) = client.get(&yf_url).timeout(std::time::Duration::from_secs(3)).send() {
            if let Ok(bars) = resp.json::<Vec<crate::data::Bar>>() {
                if !bars.is_empty() {
                    crate::bar_cache::set(&sym, &tf, &bars);
                    if send_bars(&bars, "yfinance-sidecar") { return; }
                }
            }
        }

        // 3. Direct Yahoo Finance v8 API — universal fallback
        let yahoo_url = format!(
            "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval={}&range={}",
            sym, yf_interval, yf_range
        );
        if let Ok(resp) = client.get(&yahoo_url).timeout(std::time::Duration::from_secs(5)).send() {
            if let Ok(json) = resp.json::<serde_json::Value>() {
                if let Some(bars) = crate::data::parse_yahoo_v8(&json) {
                    crate::bar_cache::set(&sym, &tf, &bars);
                    send_bars(&bars, "Yahoo Finance");
                }
            }
        }
    });
}

pub(crate) fn fetch_overlay_bars_background(sym: String, tf: String) {
    eprintln!("[overlay-fetch] Starting fetch for {} {}", sym, tf);
    let txs: Vec<std::sync::mpsc::Sender<ChartCommand>> = crate::NATIVE_CHART_TXS
        .get().and_then(|m| m.lock().ok()).map(|g| g.clone()).unwrap_or_default();
    if txs.is_empty() { eprintln!("[overlay-fetch] No TXS channels!"); return; }
    std::thread::spawn(move || {
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
        let ococo_url = format!("http://192.168.1.60:30300/api/bars?symbol={}&interval={}&limit=500", sym, tf);
        if let Some(bars) = fetch(&ococo_url).filter(|b| !b.is_empty()) {
            let gpu_bars: Vec<Bar> = bars.iter().map(|b| Bar { open: b.open as f32, high: b.high as f32, low: b.low as f32, close: b.close as f32, volume: b.volume as f32, _pad: 0.0 }).collect();
            let timestamps: Vec<i64> = bars.iter().map(|b| b.time).collect();
            let cmd = ChartCommand::OverlayBars { symbol: sym.clone(), bars: gpu_bars, timestamps };
            for tx in &txs { let _ = tx.send(cmd.clone()); }
            return;
        }
        let yahoo_url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{}?interval={}&range={}", sym, yf_interval, yf_range);
        if let Ok(resp) = client.get(&yahoo_url).timeout(std::time::Duration::from_secs(5)).send() {
            if let Ok(json) = resp.json::<serde_json::Value>() {
                if let Some(bars) = crate::data::parse_yahoo_v8(&json) {
                    let gpu_bars: Vec<Bar> = bars.iter().map(|b| Bar { open: b.open as f32, high: b.high as f32, low: b.low as f32, close: b.close as f32, volume: b.volume as f32, _pad: 0.0 }).collect();
                    let timestamps: Vec<i64> = bars.iter().map(|b| b.time).collect();
                    let cmd = ChartCommand::OverlayBars { symbol: sym.clone(), bars: gpu_bars, timestamps };
                    for tx in &txs { let _ = tx.send(cmd.clone()); }
                }
            }
        }
    });
}

