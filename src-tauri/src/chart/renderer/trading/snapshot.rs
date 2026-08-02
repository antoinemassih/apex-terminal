//! Lock-free-ish snapshot of the orders list, published after every mutation.
//!
//! Render sites can read via `current()` instead of locking the global
//! `ORDER_MANAGER` mutex. Implementation uses a `Mutex<Arc<OrdersSnapshot>>` —
//! reads only briefly hold the snapshot mutex to clone an Arc (cheap), then
//! release. This avoids contention with the heavy `ORDER_MANAGER` mutex which
//! is held during all order mutations + HTTP-spawn setup.

use std::sync::{Arc, Mutex, OnceLock};

use super::order_manager::ManagedOrder;

#[derive(Default, Clone)]
pub(crate) struct OrdersSnapshot {
    pub(crate) orders: Vec<ManagedOrder>,
}

/// `(seq, snapshot)`. The sequence is assigned under the ORDER_MANAGER guard
/// so it orders publishes by the moment their data was CLONED, not by the
/// moment they happened to reach this slot.
static ORDERS_SNAPSHOT: OnceLock<Mutex<(u64, Arc<OrdersSnapshot>)>> = OnceLock::new();

fn slot() -> &'static Mutex<(u64, Arc<OrdersSnapshot>)> {
    ORDERS_SNAPSHOT.get_or_init(|| Mutex::new((0, Arc::new(OrdersSnapshot::default()))))
}

/// Publish a fresh snapshot.
///
/// Called by `with_mgr` AFTER the `ORDER_MANAGER` mutex guard has been dropped
/// (CC1 fix: prevents the nested ORDER_MANAGER → ORDERS_SNAPSHOT lock-chain).
/// The `orders` slice is captured from the manager while the guard was still
/// held, then published here without any manager lock.
/// AUDIT 2026-08-02 (AT-097): publishes carry a sequence and STALE ONES ARE
/// DROPPED.
///
/// `with_mgr` clones the orders inside the ORDER_MANAGER guard and publishes
/// after releasing it (CC1, to break a nested lock chain). That release is what
/// created the window: thread A can clone v1, unlock, get descheduled; thread B
/// clones v2, unlocks, publishes v2; then A publishes v1 — and the slot holds
/// v1 until the next mutation. The order ledger and the on-chart order lines
/// then show a cancelled order as live, or miss a fill, indefinitely.
///
/// `seq` is assigned under the manager guard, so it reflects clone order rather
/// than arrival order. A publish older than what is already stored is discarded.
pub(crate) fn publish(seq: u64, orders: &[ManagedOrder]) {
    let snap = Arc::new(OrdersSnapshot { orders: orders.to_vec() });
    if let Ok(mut g) = slot().lock() {
        if seq >= g.0 {
            *g = (seq, snap);
        }
    }
}

/// Read the latest published snapshot without locking the manager.
#[allow(dead_code)]
pub(crate) fn current() -> Arc<OrdersSnapshot> {
    match slot().lock() {
        Ok(g) => g.1.clone(),
        Err(p) => p.into_inner().1.clone(),
    }
}

/// Sequence currently held by the slot. Test-only: the guard's whole job is
/// ordering, and asserting on it directly avoids constructing a `ManagedOrder`
/// (which has no `Default`, deliberately — it is a money-path struct).
#[cfg(test)]
pub(crate) fn current_seq() -> u64 {
    match slot().lock() { Ok(g) => g.0, Err(p) => p.into_inner().0 }
}

#[cfg(test)]
mod at097_tests {
    use super::*;

    /// AUDIT 2026-08-02 (AT-097): `with_mgr` clones the orders under the
    /// ORDER_MANAGER guard and publishes AFTER releasing it (CC1, to break a
    /// nested lock chain). That release is the window: thread A can clone v1,
    /// unlock, stall; thread B clones v2, unlocks, publishes v2; then A
    /// publishes v1 — and the slot holds the OLDER data until the next
    /// mutation. The ledger and on-chart order lines then show a cancelled
    /// order as live, indefinitely.
    ///
    /// Replays exactly that interleaving.
    #[test]
    fn a_late_publish_of_older_data_is_discarded() {
        let empty: Vec<ManagedOrder> = vec![];

        // Establish a high-water mark well above anything another test may have
        // published — this slot is process-global.
        let base = current_seq() + 100;
        publish(base + 2, &empty);
        assert_eq!(current_seq(), base + 2, "newest publish must be visible");

        // Thread A arrives late carrying an EARLIER clone. It must not win.
        publish(base + 1, &empty);
        assert_eq!(current_seq(), base + 2,
            "a publish whose data was cloned EARLIER must not overwrite newer              state just because it arrived later — that is the stale-slot bug");

        // A genuinely newer publish still lands.
        publish(base + 3, &empty);
        assert_eq!(current_seq(), base + 3, "a later sequence must apply");
    }
}
