//! `StoreRegistry` — flat holder of all `Arc<dyn PersistableStore>` for the
//! persist supervisor thread. Stores are accessed directly by handle; the
//! registry's only job is the periodic walk.

use super::persistable_store::PersistableStore;
use std::sync::Arc;
use parking_lot::RwLock;

#[derive(Default)]
pub struct StoreRegistry {
    stores: RwLock<Vec<Arc<dyn PersistableStore>>>,
}

impl StoreRegistry {
    pub fn new() -> Arc<Self> { Arc::new(Self::default()) }

    /// Register a store so the supervisor will walk it.
    pub fn register(&self, store: Arc<dyn PersistableStore>) {
        self.stores.write().push(store);
    }

    /// Snapshot of the registered stores (for the supervisor's walk).
    pub fn all(&self) -> Vec<Arc<dyn PersistableStore>> {
        self.stores.read().clone()
    }

    /// Force a synchronous flush of every store. Call on window-close.
    ///
    /// Returns a list of `(key, error)` pairs for any stores that failed to
    /// flush. Stores that flush successfully are marked as persisted.
    pub fn flush_all(&self) -> Vec<(&'static str, std::io::Error)> {
        let mut failures = Vec::new();
        for store in self.all() {
            if let Err(e) = store.flush() {
                failures.push((store.key(), e));
            } else {
                store.mark_persisted();
            }
        }
        failures
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::store::Store;
    use super::super::persistable_store::PersistableStore;
    use super::super::persistence::Persistable;
    use std::path::PathBuf;

    #[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Widget {
        n: i32,
    }

    impl Persistable for Widget {
        const KEY: &'static str = "registry_widget";
        const VERSION: u32 = 1;
    }

    fn tmp_path(stem: &str) -> PathBuf {
        std::env::temp_dir().join(format!("apex_registry_{stem}.json"))
    }

    #[test]
    fn register_then_all_round_trips() {
        let registry = StoreRegistry::new();
        assert_eq!(registry.all().len(), 0);

        let s1: Arc<Store<Widget>> = Store::new("w1", Widget { n: 1 }, None);
        let s2: Arc<Store<Widget>> = Store::new("w2", Widget { n: 2 }, None);

        registry.register(s1.clone() as Arc<dyn PersistableStore>);
        registry.register(s2.clone() as Arc<dyn PersistableStore>);

        let all = registry.all();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].key(), "w1");
        assert_eq!(all[1].key(), "w2");
    }

    #[test]
    fn flush_all_walks_every_store() {
        let path1 = tmp_path("flush_all_1");
        let path2 = tmp_path("flush_all_2");
        let _ = std::fs::remove_file(&path1);
        let _ = std::fs::remove_file(&path2);

        let registry = StoreRegistry::new();
        let s1: Arc<Store<Widget>> = Store::new("fa1", Widget { n: 10 }, Some(path1.clone()));
        let s2: Arc<Store<Widget>> = Store::new("fa2", Widget { n: 20 }, Some(path2.clone()));
        registry.register(s1 as Arc<dyn PersistableStore>);
        registry.register(s2 as Arc<dyn PersistableStore>);

        let failures = registry.flush_all();
        assert!(failures.is_empty(), "no failures expected: {failures:?}");

        let v1: Option<Widget> = super::super::persistence::load(&path1);
        let v2: Option<Widget> = super::super::persistence::load(&path2);
        assert_eq!(v1, Some(Widget { n: 10 }));
        assert_eq!(v2, Some(Widget { n: 20 }));

        let _ = std::fs::remove_file(&path1);
        let _ = std::fs::remove_file(&path2);
    }

    #[test]
    fn flush_all_returns_failure_for_bad_path() {
        let registry = StoreRegistry::new();
        // A path whose PARENT is a regular FILE, not a directory. Writing a
        // child under it fails on every platform (you can't create a directory
        // where a file already exists) — unlike the old "/nonexistent_dir_xyz/"
        // which was unwritable on Linux but writable on Windows (C:\ lets a user
        // create top-level dirs), so this test used to fail only on Windows.
        let file_as_parent = std::env::temp_dir()
            .join(format!("apex_notadir_{}.tmp", std::process::id()));
        std::fs::write(&file_as_parent, b"x").expect("seed the file-as-parent");
        let bad_path = file_as_parent.join("child.json");
        let s: Arc<Store<Widget>> = Store::new("bad", Widget { n: 0 }, Some(bad_path));
        registry.register(s as Arc<dyn PersistableStore>);

        let failures = registry.flush_all();
        let _ = std::fs::remove_file(&file_as_parent);
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].0, "bad");
    }
}
