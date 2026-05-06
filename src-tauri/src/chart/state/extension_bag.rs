//! Forward-compat scratch space.
//!
//! When XOL (or a future native file) carries fields this build doesn't know
//! about, we don't drop them. They live here as opaque JSON values, keyed by
//! field name, and survive round-trips: read → in-memory → write emits them
//! back verbatim.
//!
//! v1 stores values as `serde_json::Value` for simplicity. The spec calls for
//! CBOR once we wire up the native binary format; that's a Phase 2 concern
//! (swap the storage type, keep the API).

use std::collections::BTreeMap;

use arcstr::ArcStr;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExtensionBag {
    fields: BTreeMap<ArcStr, serde_json::Value>,
}

impl ExtensionBag {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub fn insert(&mut self, key: impl Into<ArcStr>, value: serde_json::Value) {
        self.fields.insert(key.into(), value);
    }

    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.fields.get(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ArcStr, &serde_json::Value)> {
        self.fields.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_unknown_fields() {
        let mut bag = ExtensionBag::new();
        bag.insert("future_field", serde_json::json!({"hello": 42}));
        let json = serde_json::to_string(&bag).unwrap();
        let parsed: ExtensionBag = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed.get("future_field"),
            Some(&serde_json::json!({"hello": 42}))
        );
    }
}
