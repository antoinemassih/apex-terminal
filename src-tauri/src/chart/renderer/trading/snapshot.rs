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

static ORDERS_SNAPSHOT: OnceLock<Mutex<Arc<OrdersSnapshot>>> = OnceLock::new();

fn slot() -> &'static Mutex<Arc<OrdersSnapshot>> {
    ORDERS_SNAPSHOT.get_or_init(|| Mutex::new(Arc::new(OrdersSnapshot::default())))
}

/// Publish a fresh snapshot.
///
/// Called by `with_mgr` AFTER the `ORDER_MANAGER` mutex guard has been dropped
/// (CC1 fix: prevents the nested ORDER_MANAGER → ORDERS_SNAPSHOT lock-chain).
/// The `orders` slice is captured from the manager while the guard was still
/// held, then published here without any manager lock.
pub(crate) fn publish(orders: &[ManagedOrder]) {
    let snap = Arc::new(OrdersSnapshot { orders: orders.to_vec() });
    if let Ok(mut g) = slot().lock() {
        *g = snap;
    }
}

/// Read the latest published snapshot without locking the manager.
#[allow(dead_code)]
pub(crate) fn current() -> Arc<OrdersSnapshot> {
    match slot().lock() {
        Ok(g) => g.clone(),
        Err(p) => p.into_inner().clone(),
    }
}
