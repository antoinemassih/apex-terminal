use serde::{Deserialize, Serialize};

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
    let url = format!(
        "http://127.0.0.1:8777/bars?symbol={}&interval={}&period={}",
        symbol, interval, period
    );
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("Failed to reach yfinance server: {}", e))?;
    let bars: Vec<Bar> = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse bars: {}", e))?;
    Ok(bars)
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
