//! Central in-flight request tracker — replaces the scattered `*_loading: bool`
//! flags currently sprinkled across `Watchlist` (chain_loading,
//! overlay_chain_loading, discord_channels_loading, discord_messages_loading,
//! scanner_fetching, etc.).
//!
//! The registry is intentionally allocation-light (a single `Vec<InFlight>`
//! with linear scans) because the steady-state population is at most a
//! handful of concurrent requests. If the population ever grows to hundreds,
//! switch the storage to a `HashMap<RequestId, InFlight>` — the public API is
//! deliberately scan-friendly so the change is local.
//!
//! Wave 5 wires the registry into `Watchlist` and migrates *one* legacy
//! `chain_loading` boolean as proof-of-concept. The boolean stays as-is to
//! avoid disturbing the ~20 read/write sites; the registry mirrors the same
//! lifecycle so observability and dedup work correctly today.

use std::time::{Duration, Instant};

/// Opaque, monotonic id for an in-flight request. Callers `start` to get
/// one and `complete` to release it.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct RequestId(pub u64);

/// Typed description of what kind of request is in flight. Used for
/// dedup, observability, and (in future waves) display in an operator
/// loading panel.
#[derive(Clone, Debug)]
pub enum InFlightKind {
    HistoryBars { symbol: String, timeframe: String },
    OptionsChain { underlying: String },
    OverlayChain { underlying: String },
    SymbolSearch { query: String },
    DiscordMessages,
    Scanner { name: String },
    Other(String),
}

impl InFlightKind {
    /// Stable identity tuple used by `dedup_kind` to find a duplicate
    /// outstanding request. Excludes any data fields whose change should
    /// re-issue (none today — all fields are part of identity).
    fn ident(&self) -> (u8, &str, &str) {
        match self {
            InFlightKind::HistoryBars { symbol, timeframe } => (0, symbol.as_str(), timeframe.as_str()),
            InFlightKind::OptionsChain { underlying } => (1, underlying.as_str(), ""),
            InFlightKind::OverlayChain { underlying } => (2, underlying.as_str(), ""),
            InFlightKind::SymbolSearch { query } => (3, query.as_str(), ""),
            InFlightKind::DiscordMessages => (4, "", ""),
            InFlightKind::Scanner { name } => (5, name.as_str(), ""),
            InFlightKind::Other(s) => (6, s.as_str(), ""),
        }
    }
}

/// A single tracked request.
#[derive(Clone, Debug)]
pub struct InFlight {
    pub id: RequestId,
    pub kind: InFlightKind,
    pub started_at: Instant,
    pub timeout: Duration,
}

/// In-memory tracker. One per `Watchlist`.
pub struct InFlightRegistry {
    next_id: u64,
    inflight: Vec<InFlight>,
}

impl InFlightRegistry {
    pub fn new() -> Self {
        Self { next_id: 1, inflight: Vec::new() }
    }

    /// Begin tracking a new request. Returns its `RequestId`.
    pub fn start(&mut self, kind: InFlightKind, timeout: Duration) -> RequestId {
        let id = RequestId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        self.inflight.push(InFlight {
            id,
            kind,
            started_at: Instant::now(),
            timeout,
        });
        id
    }

    /// Mark a request complete (success, failure, or cancel — the
    /// registry doesn't distinguish; callers handle outcome separately).
    pub fn complete(&mut self, id: RequestId) {
        self.inflight.retain(|r| r.id != id);
    }

    /// All currently-tracked requests.
    pub fn active(&self) -> &[InFlight] {
        &self.inflight
    }

    /// Drop and return any requests whose `timeout` has elapsed. Caller
    /// should toast / log / cancel as appropriate.
    pub fn cull_expired(&mut self) -> Vec<InFlight> {
        let now = Instant::now();
        let mut expired = Vec::new();
        self.inflight.retain(|r| {
            if now.duration_since(r.started_at) >= r.timeout {
                expired.push(r.clone());
                false
            } else {
                true
            }
        });
        expired
    }

    /// Return the id of an existing request with the same identity, if any.
    /// Callers use this to suppress redundant fetches.
    pub fn dedup_kind(&self, kind: &InFlightKind) -> Option<RequestId> {
        let ident = kind.ident();
        self.inflight
            .iter()
            .find(|r| r.kind.ident() == ident)
            .map(|r| r.id)
    }

    /// `true` if any in-flight request matches the predicate. Replacement
    /// for an ad-hoc `chain_loading: bool` read.
    pub fn is_loading(&self, matcher: impl Fn(&InFlightKind) -> bool) -> bool {
        self.inflight.iter().any(|r| matcher(&r.kind))
    }
}

impl Default for InFlightRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for InFlightRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InFlightRegistry")
            .field("active", &self.inflight.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_and_complete_round_trip() {
        let mut reg = InFlightRegistry::new();
        let id = reg.start(
            InFlightKind::HistoryBars { symbol: "AAPL".into(), timeframe: "1d".into() },
            Duration::from_secs(10),
        );
        assert_eq!(reg.active().len(), 1);
        reg.complete(id);
        assert!(reg.active().is_empty());
    }

    #[test]
    fn dedup_kind_finds_existing_request() {
        let mut reg = InFlightRegistry::new();
        let id = reg.start(
            InFlightKind::OptionsChain { underlying: "SPY".into() },
            Duration::from_secs(5),
        );
        let dup = reg.dedup_kind(&InFlightKind::OptionsChain { underlying: "SPY".into() });
        assert_eq!(dup, Some(id));
        // Different underlying → no dup.
        let other = reg.dedup_kind(&InFlightKind::OptionsChain { underlying: "QQQ".into() });
        assert_eq!(other, None);
    }

    #[test]
    fn is_loading_matches_predicate() {
        let mut reg = InFlightRegistry::new();
        assert!(!reg.is_loading(|k| matches!(k, InFlightKind::OptionsChain { .. })));
        reg.start(
            InFlightKind::OptionsChain { underlying: "SPY".into() },
            Duration::from_secs(5),
        );
        assert!(reg.is_loading(|k| matches!(k, InFlightKind::OptionsChain { .. })));
        assert!(!reg.is_loading(|k| matches!(k, InFlightKind::DiscordMessages)));
    }

    #[test]
    fn cull_expired_drops_timed_out() {
        let mut reg = InFlightRegistry::new();
        let _expired_id = reg.start(InFlightKind::DiscordMessages, Duration::from_nanos(1));
        let kept_id = reg.start(
            InFlightKind::Scanner { name: "gainers".into() },
            Duration::from_secs(60),
        );
        // Sleep a hair to ensure the nano-timeout has elapsed even on fast hosts.
        std::thread::sleep(Duration::from_millis(2));
        let expired = reg.cull_expired();
        assert_eq!(expired.len(), 1);
        assert!(matches!(expired[0].kind, InFlightKind::DiscordMessages));
        assert_eq!(reg.active().len(), 1);
        assert_eq!(reg.active()[0].id, kept_id);
    }
}
