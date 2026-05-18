//! `PersistableStore` — object-safe trait the persist supervisor walks.
//!
//! Implemented for `Store<T> where T: Persistable + Serialize + DeserializeOwned`.
//!
//! ## Flush implementation
//!
//! `flush()` reads a snapshot from the store and delegates to the existing
//! `state::persistence::save` free function (which writes a versioned JSON
//! envelope). No new serialization machinery is introduced here — the
//! `Persistable` contract from `state::persistence` is the single source of
//! truth for on-disk format.

use std::io;

/// Object-safe trait that the persist supervisor walks.
///
/// Types that implement this trait can be registered in a [`super::store_registry::StoreRegistry`]
/// and will be periodically flushed to disk by the background supervisor thread.
pub trait PersistableStore: Send + Sync {
    /// Stable key used in error messages and telemetry.
    fn key(&self) -> &'static str;

    /// True iff the debounce window has elapsed since the last mutation.
    fn needs_persist(&self) -> bool;

    /// Write the current value to disk. Reuses `state::persistence::save`.
    fn flush(&self) -> io::Result<()>;

    /// Clear the pending-persist flag after a successful flush.
    fn mark_persisted(&self);
}

// Blanket impl: any `Store<T>` where T is `Persistable + Serialize` can be
// treated as a `PersistableStore`. The flush delegates to the free-function
// `state::persistence::save` so no parallel serializer is introduced.
impl<T> PersistableStore for super::store::Store<T>
where
    T: super::persistence::Persistable
        + serde::Serialize
        + Clone
        + Send
        + Sync
        + 'static,
{
    fn key(&self) -> &'static str {
        super::store::Store::key(self)
    }

    fn needs_persist(&self) -> bool {
        super::store::Store::needs_persist(self)
    }

    fn flush(&self) -> io::Result<()> {
        if let Some(path) = self.persist_path.as_ref() {
            let snapshot = self.read_clone();
            super::persistence::save(path, &snapshot)?;
        }
        Ok(())
    }

    fn mark_persisted(&self) {
        super::store::Store::mark_persisted(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::store::Store;
    use super::super::persistence::Persistable;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Dummy {
        x: u32,
    }

    impl Persistable for Dummy {
        const KEY: &'static str = "dummy_persistable_store";
        const VERSION: u32 = 1;
    }

    fn tmp_path(stem: &str) -> PathBuf {
        std::env::temp_dir().join(format!("apex_pstore_{stem}.json"))
    }

    fn make_store(stem: &str, x: u32) -> Arc<Store<Dummy>> {
        Store::new("dummy", Dummy { x }, Some(tmp_path(stem)))
    }

    #[test]
    fn flush_writes_to_disk_and_reloads() {
        let path = tmp_path("flush_rw");
        let _ = std::fs::remove_file(&path);
        let store = Store::new("dummy", Dummy { x: 7 }, Some(path.clone()));

        store.flush().expect("flush should succeed");

        let loaded: Option<Dummy> = super::super::persistence::load(&path);
        assert_eq!(loaded, Some(Dummy { x: 7 }));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn flush_no_op_when_no_path() {
        let store: Arc<Store<Dummy>> = Store::new("dummy", Dummy { x: 0 }, None);
        // Should not error when there is no persist path.
        store.flush().expect("flush with no path is a no-op");
    }

    #[test]
    fn needs_persist_false_immediately() {
        let store = make_store("np_false", 0);
        store.update(|d| d.x = 1);
        assert!(!store.needs_persist());
    }

    #[test]
    fn needs_persist_true_after_debounce() {
        let store = make_store("np_true", 0);
        store.update(|d| d.x = 1);
        thread::sleep(Duration::from_millis(super::super::store::DEBOUNCE_MS + 50));
        assert!(store.needs_persist());
        store.mark_persisted();
        assert!(!store.needs_persist());
    }
}
