use serde::{Deserialize, Serialize};

/// Detect crypto symbols (Binance pairs)
pub fn is_crypto(symbol: &str) -> bool {
    let s = symbol.to_uppercase();
    s.ends_with("USDT") || s.ends_with("BUSD") || s.ends_with("USDC")
        || s.ends_with("BTC") && s.len() > 3 && s != "GBTC"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bar {
    pub time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionContract {
    pub strike: f64,
    #[serde(rename = "lastPrice")]
    pub last_price: f64,
    pub bid: f64,
    pub ask: f64,
    pub volume: i64,
    #[serde(rename = "openInterest")]
    pub open_interest: i64,
    #[serde(rename = "impliedVolatility")]
    pub implied_volatility: f64,
    #[serde(rename = "inTheMoney")]
    pub in_the_money: bool,
    #[serde(rename = "contractSymbol")]
    pub contract_symbol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionsChain {
    pub expirations: Vec<String>,
    pub date: Option<String>,
    pub calls: Vec<OptionContract>,
    pub puts: Vec<OptionContract>,
}

#[tauri::command]
pub async fn get_bars(symbol: String, interval: String, period: String) -> Result<Vec<Bar>, String> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0")
        .build().map_err(|e| e.to_string())?;

    // 0. Crypto → ApexCrypto directly (manages its own cache + Binance backfill)
    if is_crypto(&symbol) {
        let apex_url = format!("http://192.168.1.56:30840/api/bars/{}/{}", symbol, interval);
        if let Ok(resp) = client.get(&apex_url).timeout(std::time::Duration::from_secs(5)).send().await {
            if let Ok(bars) = resp.json::<Vec<Bar>>().await {
                if !bars.is_empty() {
                    eprintln!("[get_bars] {} bars for {} from ApexCrypto", bars.len(), symbol);
                    return Ok(bars);
                }
            }
        }
        return Ok(vec![]);
    }

    // 1. Redis cache (stocks only)
    if let Some(cached) = crate::bar_cache::get(&symbol, &interval) {
        if !cached.is_empty() {
            eprintln!("[get_bars] Cache hit for {}:{} ({} bars)", symbol, interval, cached.len());
            return Ok(cached);
        }
    }

    // 2. OCOCO
    let ococo_url = format!("http://192.168.1.60:30300/api/bars?symbol={}&interval={}&limit=500", symbol, interval);
    if let Ok(resp) = client.get(&ococo_url).timeout(std::time::Duration::from_secs(2)).send().await {
        if let Ok(bars) = resp.json::<Vec<Bar>>().await {
            if !bars.is_empty() {
                crate::bar_cache::set(&symbol, &interval, &bars);
                return Ok(bars);
            }
        }
    }

    // 3. yfinance sidecar
    let yf_url = format!("http://127.0.0.1:8777/bars?symbol={}&interval={}&period={}", symbol, interval, period);
    if let Ok(resp) = client.get(&yf_url).timeout(std::time::Duration::from_secs(3)).send().await {
        if let Ok(bars) = resp.json::<Vec<Bar>>().await {
            if !bars.is_empty() {
                crate::bar_cache::set(&symbol, &interval, &bars);
                return Ok(bars);
            }
        }
    }

    // 4. Direct Yahoo Finance v8 API
    let (yf_interval, yf_range) = match interval.as_str() {
        "1m" => ("1m","5d"), "2m" => ("2m","5d"), "5m" => ("5m","5d"),
        "15m" => ("15m","60d"), "30m" => ("30m","60d"),
        "1h" | "60m" => ("60m","60d"), "4h" => ("1h","730d"),
        "1d" => ("1d","5y"), "1wk" => ("1wk","10y"),
        _ => (interval.as_str(), &*period),
    };
    let yahoo_url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval={}&range={}",
        symbol, yf_interval, yf_range
    );
    if let Ok(resp) = client.get(&yahoo_url).timeout(std::time::Duration::from_secs(5)).send().await {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(bars) = parse_yahoo_v8(&json) {
                crate::bar_cache::set(&symbol, &interval, &bars);
                return Ok(bars);
            }
        }
    }

    Ok(vec![])
}

/// Parse Yahoo Finance v8 chart JSON response into a Bar vec.
pub fn parse_yahoo_v8(json: &serde_json::Value) -> Option<Vec<Bar>> {
    let result = json.get("chart")?.get("result")?.get(0)?;
    let timestamps = result.get("timestamp")?.as_array()?;
    let quote = result.get("indicators")?.get("quote")?.get(0)?;
    let opens = quote.get("open")?.as_array()?;
    let highs = quote.get("high")?.as_array()?;
    let lows = quote.get("low")?.as_array()?;
    let closes = quote.get("close")?.as_array()?;
    let volumes = quote.get("volume")?.as_array()?;
    let mut bars = Vec::with_capacity(timestamps.len());
    for i in 0..timestamps.len() {
        let o = opens.get(i).and_then(|v| v.as_f64());
        let h = highs.get(i).and_then(|v| v.as_f64());
        let l = lows.get(i).and_then(|v| v.as_f64());
        let c = closes.get(i).and_then(|v| v.as_f64());
        let v = volumes.get(i).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let t = timestamps.get(i).and_then(|v| v.as_i64()).unwrap_or(0);
        if let (Some(o), Some(h), Some(l), Some(c)) = (o, h, l, c) {
            bars.push(Bar { time: t, open: o, high: h, low: l, close: c, volume: v });
        }
    }
    if bars.is_empty() { None } else { Some(bars) }
}

#[tauri::command]
pub async fn get_options_chain(symbol: String, date: Option<String>) -> Result<OptionsChain, String> {
    let mut url = format!("http://127.0.0.1:8777/options?symbol={}", symbol);
    if let Some(d) = &date {
        url.push_str(&format!("&date={}", d));
    }
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("Failed to reach yfinance server: {}", e))?;
    let chain: OptionsChain = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse options chain: {}", e))?;
    Ok(chain)
}
