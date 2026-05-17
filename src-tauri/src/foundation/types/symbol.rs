//! Canonical symbol with vendor-alias registry.
//!
//! Each `Symbol` carries a canonical identifier (e.g. `"AAPL"`, `"BTCUSDT"`,
//! `"SPX"`), an `AssetClass`, and a per-vendor alias map so callers can ask
//! "what does Polygon call this?" without baking vendor naming into every
//! call site. Replaces the brittle suffix-detection (`is_crypto`) and the
//! hardcoded `O:` options prefix.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Vendor {
    ApexData,
    ApexCrypto,
    InteractiveBrokers,
    Polygon,
    Alpaca,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum AssetClass {
    Equity,
    Crypto,
    Option,
    Future,
    Forex,
    Index,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Symbol {
    pub canonical: String,
    pub asset_class: AssetClass,
    /// Per-vendor symbology. `alias()` falls back to `canonical` when the
    /// vendor has no override.
    pub aliases: HashMap<Vendor, String>,
}

impl Symbol {
    fn new(canonical: impl Into<String>, asset_class: AssetClass) -> Self {
        Self {
            canonical: canonical.into(),
            asset_class,
            aliases: HashMap::new(),
        }
    }

    pub fn equity(canonical: impl Into<String>) -> Self {
        Self::new(canonical, AssetClass::Equity)
    }

    pub fn crypto(canonical: impl Into<String>) -> Self {
        Self::new(canonical, AssetClass::Crypto)
    }

    pub fn option(canonical: impl Into<String>) -> Self {
        Self::new(canonical, AssetClass::Option)
    }

    pub fn index(canonical: impl Into<String>) -> Self {
        Self::new(canonical, AssetClass::Index)
    }

    pub fn future(canonical: impl Into<String>) -> Self {
        Self::new(canonical, AssetClass::Future)
    }

    pub fn forex(canonical: impl Into<String>) -> Self {
        Self::new(canonical, AssetClass::Forex)
    }

    pub fn with_alias(mut self, vendor: Vendor, alias: impl Into<String>) -> Self {
        self.aliases.insert(vendor, alias.into());
        self
    }

    /// Return the vendor's name for this symbol, falling back to `canonical`
    /// when the vendor has no override registered.
    pub fn alias(&self, vendor: Vendor) -> &str {
        self.aliases.get(&vendor).map(String::as_str).unwrap_or(&self.canonical)
    }

    pub fn is_crypto(&self) -> bool { self.asset_class == AssetClass::Crypto }
    pub fn is_option(&self) -> bool { self.asset_class == AssetClass::Option }
    pub fn is_equity(&self) -> bool { self.asset_class == AssetClass::Equity }
    pub fn is_index(&self) -> bool { self.asset_class == AssetClass::Index }
}

// ── Global registry ─────────────────────────────────────────────────────────

/// Process-wide symbol registry. Populated at startup via `populate_default`
/// (or progressively as new symbols are encountered). The registry surface is
/// what matters for downstream code; the full population (index constituents,
/// option chains, etc.) is intentionally out of scope here.
pub struct SymbolRegistry {
    inner: std::sync::RwLock<HashMap<String, Symbol>>,
}

impl SymbolRegistry {
    fn new() -> Self {
        Self { inner: std::sync::RwLock::new(HashMap::new()) }
    }

    /// Insert (or replace) a symbol. Lookup key is the canonical (uppercased).
    pub fn insert(&self, sym: Symbol) {
        let key = sym.canonical.to_uppercase();
        if let Ok(mut w) = self.inner.write() {
            w.insert(key, sym);
        }
    }

    /// Lookup by canonical (case-insensitive). Returns a clone; callers that
    /// hold the result over a long boundary should not keep a registry lock.
    pub fn get(&self, canonical: &str) -> Option<Symbol> {
        let key = canonical.to_uppercase();
        self.inner.read().ok().and_then(|r| r.get(&key).cloned())
    }

    /// How many symbols are registered. Mostly for tests / diagnostics.
    pub fn len(&self) -> usize {
        self.inner.read().map(|r| r.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool { self.len() == 0 }

    /// Seed a handful of well-known symbols so callers always have *something*
    /// to look up at process start. Idempotent — repeated calls overwrite the
    /// same entries.
    pub fn populate_default(&self) {
        self.insert(Symbol::equity("AAPL")
            .with_alias(Vendor::InteractiveBrokers, "AAPL")
            .with_alias(Vendor::ApexData, "AAPL"));
        self.insert(Symbol::equity("MSFT")
            .with_alias(Vendor::InteractiveBrokers, "MSFT"));
        self.insert(Symbol::equity("SPY")
            .with_alias(Vendor::InteractiveBrokers, "SPY")
            .with_alias(Vendor::ApexData, "SPY"));
        self.insert(Symbol::index("SPX")
            .with_alias(Vendor::InteractiveBrokers, "SPX")
            .with_alias(Vendor::Polygon, "SPXW"));
        self.insert(Symbol::crypto("BTCUSDT")
            .with_alias(Vendor::ApexCrypto, "BTCUSDT"));
        self.insert(Symbol::crypto("ETHUSDT")
            .with_alias(Vendor::ApexCrypto, "ETHUSDT"));
    }
}

/// Process-wide registry accessor. First call initializes an empty registry;
/// callers (typically the connectivity boot path) invoke `populate_default`
/// once the runtime is up.
pub fn registry() -> &'static SymbolRegistry {
    static R: OnceLock<SymbolRegistry> = OnceLock::new();
    R.get_or_init(SymbolRegistry::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias_falls_back_to_canonical() {
        let s = Symbol::equity("AAPL");
        assert_eq!(s.alias(Vendor::InteractiveBrokers), "AAPL");
    }

    #[test]
    fn alias_returns_vendor_specific_when_set() {
        let s = Symbol::index("SPX").with_alias(Vendor::Polygon, "SPXW");
        assert_eq!(s.alias(Vendor::Polygon), "SPXW");
        assert_eq!(s.alias(Vendor::InteractiveBrokers), "SPX"); // fallback
    }

    #[test]
    fn is_crypto_uses_asset_class_not_suffix() {
        // The whole point of the new type: classification comes from the
        // declared asset class, not "ends with USDT".
        let s = Symbol::crypto("BTCUSDT");
        assert!(s.is_crypto());
        assert!(!s.is_option());
        // And a perverse case that the old suffix-detector would mishandle:
        let s2 = Symbol::equity("GBTC"); // a real ticker; not a crypto pair
        assert!(!s2.is_crypto());
    }

    #[test]
    fn is_option_and_index() {
        assert!(Symbol::option("AAPL  240920C00200000").is_option());
        assert!(Symbol::index("SPX").is_index());
    }

    #[test]
    fn registry_insert_lookup_roundtrip() {
        let r = SymbolRegistry::new();
        r.insert(Symbol::equity("TEST").with_alias(Vendor::Polygon, "TST"));
        let got = r.get("test").expect("case-insensitive lookup");
        assert_eq!(got.canonical, "TEST");
        assert_eq!(got.alias(Vendor::Polygon), "TST");
    }

    #[test]
    fn populate_default_seeds_some_symbols() {
        let r = SymbolRegistry::new();
        assert!(r.is_empty());
        r.populate_default();
        assert!(r.len() >= 4);
        assert!(r.get("AAPL").is_some());
        assert!(r.get("SPX").is_some());
        // Polygon alias for SPX is the Wave 3 motivating example.
        assert_eq!(r.get("SPX").unwrap().alias(Vendor::Polygon), "SPXW");
    }
}
