//! `ReplayProvider` — replays a JSONL recording from disk for deterministic
//! testing / debugging.
//!
//! Wave-6: minimal functional impl. Each line of the recording file is a JSON
//! object representing one `MockFrame`-equivalent event. Supported shapes:
//!
//!   {"bar":   {<BarWire fields>}}
//!   {"quote": {<Quote fields>}}
//!   {"trade": {<Trade fields>}}
//!   {"sleep_ms": 250}
//!
//! The `rate` field scales `sleep_ms` (e.g. `rate=2.0` doubles speed).
//! Historical `bars(...)` returns every `{"bar":...}` entry up-front.
//! Streams replay in file order. Subscribe is fire-and-forget — the receiver
//! drops when the file is exhausted.
//!
//! Not a production replay engine: no rewinding, no per-symbol filtering, no
//! random-access seek. Good enough for a single test scenario.

use super::provider::{
    BarStream, ChainSnapshot, ChainStream, MarketDataProvider, ProviderCapabilities,
    QuoteStream, TradeStream,
};
use crate::data::connectivity::{
    ApiError, Connection, ConnectionMetrics, ConnectionState,
};
use crate::data::feeds::apex_data::types::{BarWire, Quote, Trade};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;

pub struct ReplayProvider {
    name: String,
    recording_path: PathBuf,
    rate: f32,
}

impl ReplayProvider {
    pub fn new(name: impl Into<String>, recording_path: PathBuf, rate: f32) -> Self {
        Self { name: name.into(), recording_path, rate: if rate <= 0.0 { 1.0 } else { rate } }
    }

    fn load_lines(&self) -> Result<Vec<serde_json::Value>, ApiError> {
        let bytes = std::fs::read(&self.recording_path).map_err(|e| {
            ApiError::Network(format!("replay: open {}: {e}", self.recording_path.display()))
        })?;
        let text = String::from_utf8_lossy(&bytes);
        let mut out = Vec::new();
        for (i, line) in text.lines().enumerate() {
            let l = line.trim();
            if l.is_empty() || l.starts_with('#') { continue; }
            match serde_json::from_str::<serde_json::Value>(l) {
                Ok(v) => out.push(v),
                Err(e) => return Err(ApiError::Network(format!("replay: line {i}: {e}"))),
            }
        }
        Ok(out)
    }
}

impl Connection for ReplayProvider {
    fn name(&self) -> &str { &self.name }
    fn state(&self) -> ConnectionState { ConnectionState::Idle }
    fn metrics(&self) -> ConnectionMetrics { ConnectionMetrics::default() }
}

fn extract<T: serde::de::DeserializeOwned>(v: &serde_json::Value, key: &str) -> Option<T> {
    v.get(key).and_then(|x| serde_json::from_value(x.clone()).ok())
}

#[async_trait::async_trait]
impl MarketDataProvider for ReplayProvider {
    async fn bars(
        &self,
        symbol: &str,
        _tf: &str,
        _a: i64,
        _b: i64,
        limit: Option<usize>,
    ) -> Result<Vec<BarWire>, ApiError> {
        let lines = self.load_lines()?;
        let mut out: Vec<BarWire> = lines.iter()
            .filter_map(|v| extract::<BarWire>(v, "bar"))
            .filter(|b| b.symbol == symbol)
            .collect();
        if let Some(n) = limit { out.truncate(n); }
        Ok(out)
    }

    fn subscribe_bars(&self, symbol: &str, _tf: &str) -> Result<BarStream, ApiError> {
        let lines = self.load_lines()?;
        let (tx, rx) = mpsc::unbounded_channel::<BarWire>();
        let rate = self.rate;
        let sym = symbol.to_string();
        tokio::spawn(async move {
            for line in lines {
                if let Some(ms) = line.get("sleep_ms").and_then(|v| v.as_u64()) {
                    let scaled = (ms as f32 / rate).max(0.0) as u64;
                    tokio::time::sleep(Duration::from_millis(scaled)).await;
                    continue;
                }
                if let Some(bar) = extract::<BarWire>(&line, "bar") {
                    if bar.symbol == sym && tx.send(bar).is_err() { return; }
                }
            }
        });
        Ok(rx)
    }
    fn unsubscribe_bars(&self, _: &str, _: &str) {}

    fn subscribe_quotes(&self, symbol: &str) -> Result<QuoteStream, ApiError> {
        let lines = self.load_lines()?;
        let (tx, rx) = mpsc::unbounded_channel::<Quote>();
        let rate = self.rate;
        let sym = symbol.to_string();
        tokio::spawn(async move {
            for line in lines {
                if let Some(ms) = line.get("sleep_ms").and_then(|v| v.as_u64()) {
                    tokio::time::sleep(Duration::from_millis((ms as f32 / rate) as u64)).await;
                    continue;
                }
                if let Some(q) = extract::<Quote>(&line, "quote") {
                    if q.symbol == sym && tx.send(q).is_err() { return; }
                }
            }
        });
        Ok(rx)
    }
    fn unsubscribe_quotes(&self, _: &str) {}

    fn subscribe_trades(&self, symbol: &str) -> Result<TradeStream, ApiError> {
        let lines = self.load_lines()?;
        let (tx, rx) = mpsc::unbounded_channel::<Trade>();
        let rate = self.rate;
        let sym = symbol.to_string();
        tokio::spawn(async move {
            for line in lines {
                if let Some(ms) = line.get("sleep_ms").and_then(|v| v.as_u64()) {
                    tokio::time::sleep(Duration::from_millis((ms as f32 / rate) as u64)).await;
                    continue;
                }
                if let Some(t) = extract::<Trade>(&line, "trade") {
                    if t.symbol == sym && tx.send(t).is_err() { return; }
                }
            }
        });
        Ok(rx)
    }
    fn unsubscribe_trades(&self, _: &str) {}

    async fn chain_snapshot(&self, _: &str) -> Result<ChainSnapshot, ApiError> {
        Err(ApiError::NotSupported("replay: chain not implemented".into()))
    }
    fn subscribe_chain(&self, _: &str) -> Result<ChainStream, ApiError> {
        Err(ApiError::NotSupported("replay: chain not implemented".into()))
    }
    fn unsubscribe_chain(&self, _: &str) {}
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            bars: true, quotes: true, trades: true, chain: false,
            crypto_only: false, historical: true, realtime: true,
            fundamentals: false, news: false, earnings: false, corporate_actions: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::feeds::apex_data::types::AssetClass;
    use std::io::Write;

    fn write_recording(content: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "replay-test-{}-{}.jsonl",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed),
        ));
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        p
    }

    fn bar_line(t: i64, c: f64) -> String {
        format!(r#"{{"bar":{{"symbol":"AAPL","asset_class":"stock","timeframe":"1m","time":{t},"open":{c},"high":{c},"low":{c},"close":{c},"volume":0,"vwap":0,"trades":0,"closed":true}}}}"#)
    }

    #[tokio::test]
    async fn historical_bars_loads_jsonl() {
        let path = write_recording(&format!("{}\n{}\n", bar_line(1, 1.0), bar_line(2, 2.0)));
        let p = ReplayProvider::new("r", path.clone(), 10.0);
        let bars = p.bars("AAPL", "1m", 0, 0, None).await.unwrap();
        assert_eq!(bars.len(), 2);
        assert_eq!(bars[0].time, 1);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn subscribe_replays_in_order() {
        let path = write_recording(&format!("{}\n{}\n", bar_line(10, 10.0), bar_line(20, 20.0)));
        let p = ReplayProvider::new("r", path.clone(), 100.0);
        let mut rx = p.subscribe_bars("AAPL", "1m").unwrap();
        let b1 = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await.unwrap().unwrap();
        let b2 = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await.unwrap().unwrap();
        assert_eq!(b1.time, 10);
        assert_eq!(b2.time, 20);
        let _ = std::fs::remove_file(path);
        let _ = AssetClass::from_symbol("X"); // keep import used
    }
}
