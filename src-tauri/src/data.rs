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
