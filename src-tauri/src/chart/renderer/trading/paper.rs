//! Paper-trading broker — simulates order acceptance.
//!
//! Limit/stop/market orders are ACK-only: the simulated backend returns a
//! `paper:{client_id}` ID and the order sits Working until the user cancels.
//! All risk checks still run; this short-circuits only the live HTTP call.

/// Submit a paper order. Always succeeds.
pub(crate) fn submit_paper(client_id: &str) -> String {
    format!("paper:{}", client_id)
}

/// Paper cancel — always succeeds.
#[allow(dead_code)]
pub(crate) fn cancel_paper(_backend_id: &str) {}

/// Paper modify — always succeeds.
#[allow(dead_code)]
pub(crate) fn modify_paper(_backend_id: &str, _new_price: f32) {}
