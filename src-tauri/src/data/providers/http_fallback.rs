//! Single HTTP-based fallback provider that covers OCOCO / yfinance / Yahoo
//! Finance v8. Bars-only; everything else returns `NotSupported`.
//!
//! Replaces three near-identical blocks in `chart/renderer/io/fetch.rs` with
//! one parametrized struct. Parses are split into two modes:
//! - `Mode::BarsJson`  — server returns `Vec<crate::data::Bar>` directly.
//! - `Mode::YahooV8`   — Yahoo v8 chart endpoint, parsed via
//!   `crate::data::parse_yahoo_v8`.

use super::provider::{
    BarStream, ChainSnapshot, ChainStream, MarketDataProvider, ProviderCapabilities,
    QuoteStream, TradeStream,
};
use crate::data::connectivity::{
    ApiError, Connection, ConnectionMetrics, ConnectionState,
};
use crate::data::feeds::apex_data::types::{AssetClass, BarWire};

#[derive(Clone, Copy)]
pub enum Mode {
    /// Body is `Vec<crate::data::Bar>` (time in seconds).
    BarsJson,
    /// Body is Yahoo Finance v8 chart envelope.
    YahooV8,
}

pub struct HttpFallbackProvider {
    name: String,
    url_template: String,
    mode: Mode,
    timeout_ms: u64,
    /// Per-timeframe `(interval, range)` pairs for Yahoo-style endpoints.
    /// For OCOCO-style endpoints the second value is ignored.
    yf_table: bool,
}

impl HttpFallbackProvider {
    /// `url_template` placeholders: `{symbol}`, `{interval}`, `{range}`.
    pub fn new(name: impl Into<String>, url_template: impl Into<String>, mode: Mode) -> Self {
        Self {
            name: name.into(),
            url_template: url_template.into(),
            mode,
            timeout_ms: 5000,
            yf_table: true,
        }
    }

    pub fn with_timeout_ms(mut self, ms: u64) -> Self { self.timeout_ms = ms; self }

    fn render_url(&self, sym: &str, tf: &str) -> String {
        let (interval, range) = if self.yf_table {
            yf_interval_range(tf)
        } else {
            (tf, "")
        };
        self.url_template
            .replace("{symbol}", sym)
            .replace("{interval}", interval)
            .replace("{range}", range)
            .replace("{timeframe}", tf)
    }
}

fn yf_interval_range(tf: &str) -> (&'static str, &'static str) {
    match tf {
        "1m" => ("1m", "5d"),
        "2m" => ("2m", "5d"),
        "5m" => ("5m", "5d"),
        "15m" => ("15m", "60d"),
        "30m" => ("30m", "60d"),
        "1h" => ("60m", "60d"),
        "4h" => ("1h", "730d"),
        "1d" => ("1d", "5y"),
        "1wk" => ("1wk", "10y"),
        _ => ("5m", "5d"),
    }
}

impl Connection for HttpFallbackProvider {
    fn name(&self) -> &str { &self.name }
    fn state(&self) -> ConnectionState { ConnectionState::Idle }
    fn metrics(&self) -> ConnectionMetrics { ConnectionMetrics::default() }
}

#[async_trait::async_trait]
impl MarketDataProvider for HttpFallbackProvider {
    async fn bars(
        &self,
        symbol: &str,
        timeframe: &str,
        _start_ms: i64,
        _end_ms: i64,
        _limit: Option<usize>,
    ) -> Result<Vec<BarWire>, ApiError> {
        let url = self.render_url(symbol, timeframe);
        let timeout = self.timeout_ms;
        let mode = self.mode;
        let sym = symbol.to_string();
        let tf = timeframe.to_string();
        let name = self.name.clone();
        // Run the blocking reqwest call on a worker thread.
        let res: Result<Vec<crate::data::Bar>, ApiError> = tokio::task::spawn_blocking(move || {
            let client = reqwest::blocking::Client::builder()
                .user_agent("Mozilla/5.0")
                .build()
                .map_err(|e| ApiError::Network(e.to_string()))?;
            let resp = client
                .get(&url)
                .timeout(std::time::Duration::from_millis(timeout))
                .send()
                .map_err(|e| ApiError::Network(e.to_string()))?;
            let status = resp.status().as_u16();
            if !resp.status().is_success() {
                return Err(ApiError::Http {
                    status,
                    body: format!("{} {}", name, status),
                });
            }
            match mode {
                Mode::BarsJson => {
                    resp.json::<Vec<crate::data::Bar>>()
                        .map_err(|e| ApiError::Parse(e.to_string()))
                }
                Mode::YahooV8 => {
                    let json: serde_json::Value =
                        resp.json().map_err(|e| ApiError::Parse(e.to_string()))?;
                    crate::data::parse_yahoo_v8(&json)
                        .ok_or_else(|| ApiError::Parse("yahoo v8: empty".into()))
                }
            }
        })
        .await
        .map_err(|e| ApiError::Network(format!("join error: {e}")))?;

        let bars = res?;
        let class = AssetClass::from_symbol(&sym);
        Ok(bars
            .into_iter()
            .map(|b| BarWire {
                symbol: sym.clone(),
                asset_class: class,
                timeframe: tf.clone(),
                time: b.time * 1000,
                open: b.open, high: b.high, low: b.low, close: b.close, volume: b.volume,
                vwap: 0.0, trades: 0, closed: true,
            })
            .collect())
    }

    fn subscribe_bars(&self, _: &str, _: &str) -> Result<BarStream, ApiError> { Err(ApiError::NotSupported("http_fallback: realtime".into())) }
    fn unsubscribe_bars(&self, _: &str, _: &str) {}
    fn subscribe_quotes(&self, _: &str) -> Result<QuoteStream, ApiError> { Err(ApiError::NotSupported("http_fallback: quotes".into())) }
    fn unsubscribe_quotes(&self, _: &str) {}
    fn subscribe_trades(&self, _: &str) -> Result<TradeStream, ApiError> { Err(ApiError::NotSupported("http_fallback: trades".into())) }
    fn unsubscribe_trades(&self, _: &str) {}
    async fn chain_snapshot(&self, _: &str) -> Result<ChainSnapshot, ApiError> { Err(ApiError::NotSupported("http_fallback: chain".into())) }
    fn subscribe_chain(&self, _: &str) -> Result<ChainStream, ApiError> { Err(ApiError::NotSupported("http_fallback: chain".into())) }
    fn unsubscribe_chain(&self, _: &str) {}
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities { bars: true, historical: true, realtime: false, ..Default::default() }
    }
}
