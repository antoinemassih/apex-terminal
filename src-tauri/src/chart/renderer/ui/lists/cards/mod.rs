//! Card list widgets. The Wave-5 façade cards (earnings/event/news/play/
//! playbook/signal/stat/trade) were deleted in the WS-C migration cleanup —
//! they were never adopted (the live painters live in their panels, e.g.
//! plays_panel::render_play_card, and journal_panel uses ui_kit's TradeCard).
//! `metric_card` remains: it IS used (spread_panel) and is the canonical
//! MetricCard primitive.
pub mod metric_card;

pub use metric_card::*;
