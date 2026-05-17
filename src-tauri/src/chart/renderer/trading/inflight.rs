//! In-flight risk aggregators.
//!
//! Single-intent risk checks only see the candidate intent against current
//! account state — they miss working orders that haven't filled yet. Without
//! these aggregators, a user could stack working orders at the cap and
//! breach `max_position_qty`/`max_notional`.

use super::OrderSide;
use super::order_manager::{ManagedOrder, OrderState};

#[inline]
fn is_bullish_side(side: OrderSide) -> bool {
    matches!(side, OrderSide::Buy | OrderSide::TriggerBuy | OrderSide::OcoTarget)
}

#[inline]
fn is_working(state: OrderState) -> bool {
    !matches!(
        state,
        OrderState::Filled | OrderState::Cancelled | OrderState::Rejected | OrderState::Unknown
    )
}

/// Sum of remaining qty across orders that are NOT terminal (or Unknown) AND
/// match `(symbol, direction)`.
pub(crate) fn working_qty_same_direction(
    orders: &[ManagedOrder],
    symbol: &str,
    side: OrderSide,
) -> u32 {
    let bullish = is_bullish_side(side);
    orders
        .iter()
        .filter(|o| o.symbol == symbol)
        .filter(|o| is_working(o.state))
        .filter(|o| is_bullish_side(o.side) == bullish)
        .map(|o| o.qty.saturating_sub(o.filled_qty))
        .sum()
}

/// Notional of working orders for this symbol (sum of `remaining_qty * price`).
pub(crate) fn working_notional(orders: &[ManagedOrder], symbol: &str) -> f64 {
    orders
        .iter()
        .filter(|o| o.symbol == symbol)
        .filter(|o| is_working(o.state))
        .map(|o| o.qty.saturating_sub(o.filled_qty) as f64 * o.price.to_dollars())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::order_manager::{ManagedOrder, ManagedOrderType, OrderSource, OrderState};
    use crate::foundation::types::{Price, Timestamp, TimeSource, Symbol};

    fn mk(id: u64, symbol: &str, side: OrderSide, state: OrderState, price: f32, qty: u32, filled_qty: u32) -> ManagedOrder {
        let t0 = Timestamp::from_millis(0, TimeSource::Local);
        ManagedOrder {
            id,
            client_order_id: format!("test-{}", id),
            symbol: symbol.to_string(),
            symbol_typed: Symbol::equity(symbol),
            side,
            order_type: ManagedOrderType::Limit,
            price: Price::from_f32(price),
            stop_price: Price::ZERO,
            qty,
            filled_qty,
            avg_fill_price: Price::ZERO,
            state,
            pair_id: None,
            trail_amount: None,
            trail_percent: None,
            option_symbol: None,
            option_con_id: None,
            source: OrderSource::OrderPanel,
            created_at: t0,
            updated_at: t0,
            backend_order_id: None,
            tif: 0,
            outside_rth: false,
            state_history: vec![(state, t0)],
            rejection_reason: None,
            modify_version: 0,
            modify_inflight: false,
            modify_pending_price: None,
        }
    }

    #[test]
    fn working_qty_same_direction_sums_buys() {
        let orders = vec![
            mk(1, "AAPL", OrderSide::Buy, OrderState::Working, 100.0, 10, 0),
            mk(2, "AAPL", OrderSide::Buy, OrderState::PartialFill, 101.0, 20, 5),
            mk(3, "AAPL", OrderSide::Sell, OrderState::Working, 102.0, 100, 0),
        ];
        // 10 + (20 - 5) = 25
        assert_eq!(working_qty_same_direction(&orders, "AAPL", OrderSide::Buy), 25);
    }

    #[test]
    fn working_qty_skips_terminal_states() {
        let orders = vec![
            mk(1, "AAPL", OrderSide::Buy, OrderState::Filled,    100.0, 10, 10),
            mk(2, "AAPL", OrderSide::Buy, OrderState::Cancelled, 100.0, 20, 0),
            mk(3, "AAPL", OrderSide::Buy, OrderState::Rejected,  100.0, 30, 0),
            mk(4, "AAPL", OrderSide::Buy, OrderState::Unknown,   100.0, 40, 0),
            mk(5, "AAPL", OrderSide::Buy, OrderState::Working,   100.0, 7,  0),
        ];
        assert_eq!(working_qty_same_direction(&orders, "AAPL", OrderSide::Buy), 7);
    }

    #[test]
    fn working_qty_subtracts_filled_qty() {
        let orders = vec![
            mk(1, "AAPL", OrderSide::Buy, OrderState::PartialFill, 100.0, 50, 30),
        ];
        assert_eq!(working_qty_same_direction(&orders, "AAPL", OrderSide::Buy), 20);
    }

    #[test]
    fn working_qty_buckets_by_direction() {
        let orders = vec![
            mk(1, "AAPL", OrderSide::Buy,         OrderState::Working, 100.0, 10, 0),
            mk(2, "AAPL", OrderSide::TriggerBuy,  OrderState::Working, 100.0, 5,  0),
            mk(3, "AAPL", OrderSide::OcoTarget,   OrderState::Working, 100.0, 3,  0),
            mk(4, "AAPL", OrderSide::Sell,        OrderState::Working, 100.0, 7,  0),
            mk(5, "AAPL", OrderSide::Stop,        OrderState::Working, 100.0, 11, 0),
            mk(6, "AAPL", OrderSide::TriggerSell, OrderState::Working, 100.0, 13, 0),
        ];
        // Bullish bucket: Buy + TriggerBuy + OcoTarget = 10+5+3 = 18
        assert_eq!(working_qty_same_direction(&orders, "AAPL", OrderSide::Buy), 18);
        // Bearish bucket: Sell + Stop + TriggerSell = 7+11+13 = 31
        // (any non-bullish side puts you in the bearish bucket)
        assert_eq!(working_qty_same_direction(&orders, "AAPL", OrderSide::Sell), 31);
    }

    #[test]
    fn working_qty_filters_by_symbol() {
        let orders = vec![
            mk(1, "AAPL", OrderSide::Buy, OrderState::Working, 100.0, 10, 0),
            mk(2, "MSFT", OrderSide::Buy, OrderState::Working, 100.0, 20, 0),
        ];
        assert_eq!(working_qty_same_direction(&orders, "AAPL", OrderSide::Buy), 10);
        assert_eq!(working_qty_same_direction(&orders, "MSFT", OrderSide::Buy), 20);
        assert_eq!(working_qty_same_direction(&orders, "TSLA", OrderSide::Buy), 0);
    }

    #[test]
    fn working_notional_sums_remaining_times_price() {
        let orders = vec![
            mk(1, "AAPL", OrderSide::Buy,  OrderState::Working,     100.0, 10, 0),  // 10 * 100 = 1000
            mk(2, "AAPL", OrderSide::Sell, OrderState::PartialFill, 200.0, 20, 5),  // (20-5)*200 = 3000
        ];
        assert_eq!(working_notional(&orders, "AAPL"), 4000.0);
    }

    #[test]
    fn working_notional_skips_terminal() {
        let orders = vec![
            mk(1, "AAPL", OrderSide::Buy, OrderState::Filled,    100.0, 10, 10),
            mk(2, "AAPL", OrderSide::Buy, OrderState::Cancelled, 100.0, 10, 0),
            mk(3, "AAPL", OrderSide::Buy, OrderState::Rejected,  100.0, 10, 0),
            mk(4, "AAPL", OrderSide::Buy, OrderState::Unknown,   100.0, 10, 0),
            mk(5, "AAPL", OrderSide::Buy, OrderState::Working,   50.0,  4,  0), // 200
        ];
        assert_eq!(working_notional(&orders, "AAPL"), 200.0);
    }

    #[test]
    fn working_notional_filters_by_symbol() {
        let orders = vec![
            mk(1, "AAPL", OrderSide::Buy, OrderState::Working, 100.0, 10, 0),
            mk(2, "MSFT", OrderSide::Buy, OrderState::Working, 50.0,  20, 0),
        ];
        assert_eq!(working_notional(&orders, "AAPL"), 1000.0);
        assert_eq!(working_notional(&orders, "MSFT"), 1000.0);
        assert_eq!(working_notional(&orders, "TSLA"), 0.0);
    }
}
