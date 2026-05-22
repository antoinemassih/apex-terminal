//! Versioned persistence aggregate.
//!
//! Replaces the ad-hoc `{"version": 3, ...}` envelopes scattered across
//! `Watchlist::save_state` / `load_state` with a typed contract. Each
//! `Persistable` declares its `KEY` (a stable file/db identifier) and
//! a numeric `VERSION` (bumped whenever the on-disk schema changes).
//! Loading routes through `migrate` so older saves are forward-fixed
//! on read instead of being silently dropped.
//!
//! Wave 5 introduces the contract and migrates *one* call site
//! (`order_manager.rs::orders.json`) as proof-of-concept. The other
//! persistence sites stay on their current home-rolled flows until
//! follow-up waves; the goal here is to make `impl Persistable for
//! NewAggregate` cheap so each migration is a small, mechanical PR.

use std::path::Path;

/// Trait for any value that can be saved to / loaded from JSON with a
/// stable key + schema version.
pub trait Persistable: Sized {
    /// Stable identity for this aggregate. Used in error messages, the
    /// (future) per-aggregate config table, and as a default file stem.
    const KEY: &'static str;

    /// Current schema version. Bump on every breaking change to the
    /// serialized shape; pair the bump with a `migrate` arm.
    const VERSION: u32;

    /// Migrate a loaded JSON value from an older `prev_version` to the
    /// current `VERSION` shape. Default: pass through unchanged (assume
    /// the on-disk shape is already the latest).
    fn migrate(_prev_version: u32, json: serde_json::Value) -> serde_json::Value {
        json
    }
}

/// On-disk envelope so we can read the version without deserializing
/// the (possibly schema-incompatible) inner payload first.
#[derive(serde::Serialize, serde::Deserialize)]
struct Envelope {
    key: String,
    version: u32,
    payload: serde_json::Value,
}

/// Write `value` as a versioned JSON envelope to `path`.
///
/// The write is crash-safe: bytes are written to a sibling `.tmp` file,
/// flushed, then atomically renamed over the real path. Because `rename`
/// is atomic on a single filesystem, a crash mid-write leaves the previous
/// file intact; the `.tmp` file is simply abandoned and ignored on the
/// next load.
pub fn save<T>(path: &Path, value: &T) -> std::io::Result<()>
where
    T: Persistable + serde::Serialize,
{
    let envelope = Envelope {
        key: T::KEY.to_string(),
        version: T::VERSION,
        payload: serde_json::to_value(value).map_err(io_err)?,
    };
    let bytes = serde_json::to_vec_pretty(&envelope).map_err(io_err)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    atomic_write(path, &bytes)
}

/// Write `bytes` to `path` atomically: write to `<path>.tmp`, flush, then
/// rename over `path`. Rename is atomic on a single filesystem, so a crash
/// mid-write leaves the previous file intact and only a stale `.tmp` behind.
///
/// After the rename the parent directory entry is also synced so the new name
/// is durable even if the OS crashes immediately after the rename syscall.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    // Build the sibling temp path by appending ".tmp".
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp_path = std::path::Path::new(&tmp);

    {
        let mut f = std::fs::File::create(tmp_path)?;
        f.write_all(bytes)?;
        f.flush()?;
        // Propagate sync_all errors — a failure here means the data may not
        // be durable and the caller should not treat the write as succeeded.
        f.sync_all()?;
    }
    std::fs::rename(tmp_path, path)?;

    // Sync the parent directory so the directory entry (new filename) is
    // durable.  This is a no-op on Windows (opening a directory is unsupported
    // there) — we ignore errors on platforms that don't support it.
    if let Some(parent) = path.parent() {
        if let Ok(dir) = std::fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }

    Ok(())
}

/// Load a versioned JSON envelope from `path`, migrating if needed.
/// Returns `None` for: file missing, malformed envelope, key mismatch,
/// payload-after-migration that fails to deserialize. All failure modes
/// log to stderr so silent corruption is visible.
pub fn load<T>(path: &Path) -> Option<T>
where
    T: Persistable + serde::de::DeserializeOwned,
{
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => return None,
    };
    let envelope: Envelope = match serde_json::from_slice(&bytes) {
        Ok(e) => e,
        Err(e) => {
            eprintln!(
                "[state::persistence] {}: envelope parse failed: {e}",
                T::KEY
            );
            return None;
        }
    };
    if envelope.key != T::KEY {
        eprintln!(
            "[state::persistence] {}: key mismatch (got '{}')",
            T::KEY,
            envelope.key
        );
        return None;
    }
    let migrated = if envelope.version == T::VERSION {
        envelope.payload
    } else {
        T::migrate(envelope.version, envelope.payload)
    };
    match serde_json::from_value(migrated) {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!(
                "[state::persistence] {}: payload deserialize failed: {e}",
                T::KEY
            );
            None
        }
    }
}

fn io_err<E: std::fmt::Display>(e: E) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Sample {
        a: u32,
        b: String,
    }
    impl Persistable for Sample {
        const KEY: &'static str = "test_sample";
        const VERSION: u32 = 2;

        fn migrate(prev: u32, mut json: serde_json::Value) -> serde_json::Value {
            // v1 had `count` instead of `a`; rename forward.
            if prev == 1 {
                if let Some(obj) = json.as_object_mut() {
                    if let Some(c) = obj.remove("count") {
                        obj.insert("a".into(), c);
                    }
                }
            }
            json
        }
    }

    #[test]
    fn round_trip_current_version() {
        let dir = std::env::temp_dir().join("apex_state_persistence_round_trip");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("sample.json");
        let value = Sample { a: 7, b: "hello".into() };
        save(&path, &value).unwrap();
        let loaded: Sample = load(&path).unwrap();
        assert_eq!(loaded, value);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn migrate_lifts_old_payload_to_new_shape() {
        let dir = std::env::temp_dir().join("apex_state_persistence_migrate");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sample.json");
        // Hand-write a v1 envelope.
        let envelope = serde_json::json!({
            "key": "test_sample",
            "version": 1,
            "payload": {"count": 42, "b": "old"}
        });
        std::fs::write(&path, serde_json::to_vec_pretty(&envelope).unwrap()).unwrap();
        let loaded: Sample = load(&path).expect("migrate should yield a Sample");
        assert_eq!(loaded, Sample { a: 42, b: "old".into() });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn key_mismatch_returns_none() {
        let dir = std::env::temp_dir().join("apex_state_persistence_key_mismatch");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sample.json");
        let envelope = serde_json::json!({
            "key": "wrong_key",
            "version": 2,
            "payload": {"a": 1, "b": "x"}
        });
        std::fs::write(&path, serde_json::to_vec_pretty(&envelope).unwrap()).unwrap();
        let loaded: Option<Sample> = load(&path);
        assert!(loaded.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
