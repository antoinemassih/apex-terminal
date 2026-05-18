//! Bidirectional conId ↔ symbol resolver, populated lazily as IB ticks arrive.
//!
//! The IB tick stream carries both `symbol` and `conid` fields, and the
//! ApexIB backend exposes `GET /contract/{symbol}` for explicit symbol →
//! conId lookups (see `chart::renderer::trading::broker::LiveBroker`).
//!
//! Wave 9b: this resolver caches mappings observed from either direction
//! so SubscriptionManager (which keys by string symbol) can route per-symbol
//! subs to the per-conId WS calls the IB feed actually requires.
//!
//! Both lookups are O(1) under a `RwLock<HashMap>` — we deliberately avoid
//! pulling in `dashmap` to keep the dependency surface lean. The maps are
//! tiny (one entry per traded symbol per session), reads dominate writes,
//! and contention has not been observed in practice.

use std::collections::HashMap;
use std::sync::OnceLock;
use parking_lot::RwLock;

use crate::data::connectivity::ApiError;

/// Trait abstraction over the broker's `GET /contract/{symbol}` endpoint so
/// `resolve_conid` can be unit-tested without standing up a real HTTP stack.
///
/// Kept intentionally minimal — production callers wire this through the
/// `HttpConIdLookup` blocking implementation; tests provide an in-memory mock.
#[async_trait::async_trait]
pub trait ConIdLookup: Send + Sync {
    async fn lookup(&self, symbol: &str) -> Result<i64, ApiError>;
}

/// Canonicalize a symbol before keying. IB symbols are case-insensitive on
/// the wire; storing the uppercase form keeps "aapl" / "AAPL" from forking.
fn canon(symbol: &str) -> String {
    symbol.trim().to_ascii_uppercase()
}

pub struct ConIdResolver {
    by_conid: RwLock<HashMap<i64, String>>,
    by_symbol: RwLock<HashMap<String, i64>>,
}

impl ConIdResolver {
    pub fn new() -> Self {
        Self {
            by_conid: RwLock::new(HashMap::new()),
            by_symbol: RwLock::new(HashMap::new()),
        }
    }

    /// Observe a (symbol, conid) pair from any source — a tick frame, an
    /// explicit `/contract` lookup, etc. Idempotent: re-observing the same
    /// pair is a no-op. If `conid <= 0` or `symbol` is empty after trim,
    /// the call is silently ignored (defensive against malformed frames).
    pub fn observe(&self, symbol: &str, conid: i64) {
        if conid <= 0 {
            return;
        }
        let sym = canon(symbol);
        if sym.is_empty() {
            return;
        }
        {
            let mut m = self.by_conid.write();
            m.insert(conid, sym.clone());
        }
        {
            let mut m = self.by_symbol.write();
            m.insert(sym, conid);
        }
    }

    pub fn symbol_for(&self, conid: i64) -> Option<String> {
        self.by_conid
            .read()
            .get(&conid)
            .cloned()
    }

    pub fn conid_for(&self, symbol: &str) -> Option<i64> {
        let sym = canon(symbol);
        self.by_symbol
            .read()
            .get(&sym)
            .copied()
    }

    /// Lazy resolution: returns the cached conid if present, otherwise calls
    /// the provided `lookup` (typically the broker's `/contract/{symbol}`
    /// HTTP endpoint), caches both directions, and returns the result.
    pub async fn resolve_conid(
        &self,
        symbol: &str,
        lookup: &dyn ConIdLookup,
    ) -> Result<i64, ApiError> {
        if let Some(cid) = self.conid_for(symbol) {
            return Ok(cid);
        }
        let cid = lookup.lookup(symbol).await?;
        self.observe(symbol, cid);
        Ok(cid)
    }
}

impl Default for ConIdResolver {
    fn default() -> Self {
        Self::new()
    }
}

static RESOLVER: OnceLock<ConIdResolver> = OnceLock::new();

/// Process-wide singleton — first access lazily constructs it.
pub fn resolver() -> &'static ConIdResolver {
    RESOLVER.get_or_init(ConIdResolver::new)
}

// ─── Production lookup: blocking reqwest against ApexIB ────────────────────
//
// Lives here (rather than in `broker.rs`) so the resolver stays usable from
// any module without pulling in the `chart::renderer::trading` surface. The
// HTTP shape mirrors `LiveBroker::resolve_con_id` exactly: GET the URL,
// read `conId` off the JSON body.

/// Blocking HTTP implementation of `ConIdLookup`. Hits
/// `{apexib_url}/contract/{symbol}` with a 5s timeout. Runs the blocking
/// call on `tokio::task::spawn_blocking` so it can satisfy the async trait
/// without parking the runtime.
#[allow(dead_code)] // wired in once a Rust-side IB subscribe path appears
pub struct HttpConIdLookup {
    pub base_url: String,
}

#[allow(dead_code)]
impl HttpConIdLookup {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }
}

#[async_trait::async_trait]
impl ConIdLookup for HttpConIdLookup {
    async fn lookup(&self, symbol: &str) -> Result<i64, ApiError> {
        let url = format!("{}/contract/{}", self.base_url, symbol);
        let result = tokio::task::spawn_blocking(move || {
            let client = reqwest::blocking::Client::new();
            client
                .get(&url)
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .and_then(|r| r.json::<serde_json::Value>())
        })
        .await
        .map_err(|e| ApiError::Network(format!("join: {e}")))?;

        let json = result.map_err(|e| ApiError::Network(e.to_string()))?;
        json["conId"]
            .as_i64()
            .ok_or_else(|| ApiError::Parse(format!("no conId in /contract response: {json}")))
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observe_caches_both_directions() {
        let r = ConIdResolver::new();
        r.observe("AAPL", 12345);
        assert_eq!(r.symbol_for(12345).as_deref(), Some("AAPL"));
        assert_eq!(r.conid_for("AAPL"), Some(12345));
    }

    #[test]
    fn observe_is_idempotent() {
        let r = ConIdResolver::new();
        r.observe("MSFT", 999);
        r.observe("MSFT", 999);
        assert_eq!(r.conid_for("MSFT"), Some(999));
        assert_eq!(r.symbol_for(999).as_deref(), Some("MSFT"));
    }

    #[test]
    fn observe_canonicalizes_symbol_case() {
        let r = ConIdResolver::new();
        r.observe("aapl", 42);
        // Lookups are case-insensitive — upper or lower both hit.
        assert_eq!(r.conid_for("AAPL"), Some(42));
        assert_eq!(r.conid_for("aapl"), Some(42));
        assert_eq!(r.symbol_for(42).as_deref(), Some("AAPL"));
    }

    #[test]
    fn observe_rejects_invalid_inputs() {
        let r = ConIdResolver::new();
        r.observe("", 1);
        r.observe("AAPL", 0);
        r.observe("AAPL", -5);
        assert_eq!(r.conid_for("AAPL"), None);
        assert_eq!(r.symbol_for(1), None);
    }

    #[test]
    fn singleton_is_stable() {
        // Same pointer across calls — confirms OnceLock wiring.
        let a = resolver() as *const _;
        let b = resolver() as *const _;
        assert_eq!(a, b);
    }

    struct MockLookup {
        conid: i64,
        calls: parking_lot::Mutex<u32>,
    }
    #[async_trait::async_trait]
    impl ConIdLookup for MockLookup {
        async fn lookup(&self, _symbol: &str) -> Result<i64, ApiError> {
            *self.calls.lock() += 1;
            Ok(self.conid)
        }
    }

    #[tokio::test]
    async fn resolve_conid_uses_cache_when_present() {
        let r = ConIdResolver::new();
        r.observe("TSLA", 7777);
        let mock = MockLookup { conid: 1, calls: parking_lot::Mutex::new(0) };
        let cid = r.resolve_conid("TSLA", &mock).await.unwrap();
        assert_eq!(cid, 7777);
        assert_eq!(*mock.calls.lock(), 0, "cache hit must not call lookup");
    }

    #[tokio::test]
    async fn resolve_conid_falls_back_to_lookup_and_caches() {
        let r = ConIdResolver::new();
        let mock = MockLookup { conid: 4242, calls: parking_lot::Mutex::new(0) };
        let cid = r.resolve_conid("NVDA", &mock).await.unwrap();
        assert_eq!(cid, 4242);
        assert_eq!(*mock.calls.lock(), 1);

        // Second call hits the cache.
        let cid2 = r.resolve_conid("NVDA", &mock).await.unwrap();
        assert_eq!(cid2, 4242);
        assert_eq!(*mock.calls.lock(), 1, "second resolve must be cached");
    }
}
