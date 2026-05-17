//! Wave 6: black-box tests of the connectivity stack.
//!
//! Drives the `FallbackProvider` / `SubscriptionManager` over scripted
//! `MockMarketDataProvider`s so we can exercise disconnect/reconnect,
//! fallback chains, parse-error counting, and historical bar replay without
//! standing up real services.
//!
//! Order-management scenarios (fat-finger override, OCO leg cancel, bracket
//! reconciliation) live in `#[cfg(test)] mod tests` inside `order_manager.rs`
//! because `OrderManager`/`Broker` are `pub(crate)` — see the `#[ignore]`
//! stubs below for the contract those would assert if those types were lifted
//! to `pub`.

use std::sync::Arc;
use std::time::Duration;

use _scaffold_lib::data::feeds::apex_data::types::{AssetClass, BarWire};
use _scaffold_lib::data::providers::{
    FallbackProvider, MarketDataProvider, MockFrame, MockMarketDataProvider, SubscriptionManager,
};

fn bw(t: i64, c: f64) -> BarWire {
    BarWire {
        symbol: "AAPL".into(),
        asset_class: AssetClass::Stock,
        timeframe: "1m".into(),
        time: t,
        open: c, high: c, low: c, close: c, volume: 0.0,
        vwap: 0.0, trades: 0, closed: true,
    }
}

#[tokio::test]
async fn provider_fallback_picks_second_on_first_failure() {
    let a = Arc::new(
        MockMarketDataProvider::new("a").historical_error("boom")
    );
    let b = Arc::new(
        MockMarketDataProvider::new("b").historical_bars(vec![bw(1, 1.0), bw(2, 2.0)])
    );
    let fb = FallbackProvider::new("test", vec![a, b]);
    let bars = fb.bars("AAPL", "1m", 0, 0, Some(10)).await.unwrap();
    assert_eq!(bars.len(), 2, "second provider should serve");
}

#[tokio::test]
async fn provider_fallback_aggregates_errors_when_all_fail() {
    let a = Arc::new(MockMarketDataProvider::new("a").historical_error("a-down"));
    let b = Arc::new(MockMarketDataProvider::new("b").historical_error("b-down"));
    let fb = FallbackProvider::new("test", vec![a, b]);
    let err = fb.bars("AAPL", "1m", 0, 0, None).await.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("a-down"));
    assert!(msg.contains("b-down"));
}

#[tokio::test]
async fn subscription_manager_dedups_and_fans_out() {
    let mock = Arc::new(
        MockMarketDataProvider::new("m").script_bars(vec![
            MockFrame::Bar(bw(1, 1.0)),
            MockFrame::Bar(bw(2, 2.0)),
            MockFrame::Bar(bw(3, 3.0)),
        ])
    );
    let mgr = SubscriptionManager::new(mock);
    // Two callers subscribe to the same key — should share the upstream sub.
    let mut rx1 = mgr.subscribe_bars("AAPL", "1m").unwrap();
    let mut rx2 = mgr.subscribe_bars("AAPL", "1m").unwrap();
    let a = tokio::time::timeout(Duration::from_millis(500), rx1.recv()).await.unwrap().unwrap();
    let b = tokio::time::timeout(Duration::from_millis(500), rx2.recv()).await.unwrap().unwrap();
    assert_eq!(a.time, b.time, "both receivers should see the same first bar via broadcast fanout");
}

#[tokio::test]
async fn disconnect_reconnect_keeps_streaming() {
    // Scenario: mock emits a bar, simulates a disconnect, sleeps, reconnects,
    // emits another bar. Asserts the receiver sees both bars and the
    // provider's reconnect counter ticked up.
    let mock = Arc::new(
        MockMarketDataProvider::new("m").script_bars(vec![
            MockFrame::Bar(bw(100, 100.0)),
            MockFrame::Disconnect,
            MockFrame::Sleep(Duration::from_millis(30)),
            MockFrame::Reconnect,
            MockFrame::Bar(bw(200, 200.0)),
        ])
    );
    let mock_for_state: Arc<MockMarketDataProvider> = mock.clone();
    let mgr = SubscriptionManager::new(mock);
    let mut rx = mgr.subscribe_bars("AAPL", "1m").unwrap();
    let b1 = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await.unwrap().unwrap();
    let b2 = tokio::time::timeout(Duration::from_millis(1500), rx.recv()).await.unwrap().unwrap();
    assert_eq!(b1.time, 100);
    assert_eq!(b2.time, 200);
    assert!(mock_for_state.metrics_snapshot().reconnect_count >= 1, "reconnect should have ticked");
}

#[tokio::test]
async fn parse_error_in_mock_bumps_counter() {
    let mock = Arc::new(
        MockMarketDataProvider::new("m").script_bars(vec![
            MockFrame::Bar(bw(1, 1.0)),
            MockFrame::Error("malformed payload".into()),
            MockFrame::Bar(bw(2, 2.0)),
        ])
    );
    let mock_for_state: Arc<MockMarketDataProvider> = mock.clone();
    let mut rx = mock.subscribe_bars("AAPL", "1m").unwrap();
    let _ = tokio::time::timeout(Duration::from_millis(300), rx.recv()).await.unwrap().unwrap();
    let _ = tokio::time::timeout(Duration::from_millis(300), rx.recv()).await.unwrap().unwrap();
    assert!(mock_for_state.metrics_snapshot().parse_errors >= 1);
}

// ── Order-management scenarios (#[ignore] until pub API exists) ─────────────
//
// `OrderManager`, `Broker`, and `MockBroker` are `pub(crate)` so they cannot
// be reached from an integration test. The scenarios below document what we
// want to assert once those surfaces are lifted. They run as in-crate tests
// in `order_manager.rs` today (e.g. `cancel_working_order_transitions_to_cancelled`,
// `reconcile_partner_cancelled_when_one_pair_leg_fills`).

#[tokio::test]
#[ignore = "requires pub OrderManager/Broker; lives as in-crate test today"]
async fn fill_during_disconnect_reconnect() {
    // 1. MockBroker + MockMarketDataProvider scripted to disconnect mid-stream
    // 2. Submit a working limit order
    // 3. Inject WS disconnect (mock state -> Backoff)
    // 4. Inject reconnect (mock state -> Subscribed)
    // 5. Call reconcile_with_ib() with a fill
    // 6. Assert ManagedOrder state transitioned Working -> Filled
}

#[tokio::test]
#[ignore = "requires pub OrderManager; lives as in-crate test today"]
async fn fat_finger_override_flow() {
    // 1. MockBroker
    // 2. Submit limit > 5% from last_price -> NeedsApproval / Rejected
    // 3. Resubmit with override_warnings=true -> Accepted
}

#[tokio::test]
#[ignore = "requires pub OrderManager; covered by reconcile_partner_cancelled_when_one_pair_leg_fills"]
async fn oco_leg_cancellation_on_partner_fill() {
    // Two OCO legs; fill one via reconcile; assert the partner cancels.
}
