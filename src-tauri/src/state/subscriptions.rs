//! Cross-pane event subscriptions — replaces ad-hoc manual iteration over
//! panes for link-group propagation, broadcast mode, layout sync, etc.
//!
//! ## Model: queue-drain (Wave 12c)
//!
//! Earlier wave shipped a synchronous fan-out bus (`publish` invoked every
//! registered listener immediately). That model could not actually drive
//! sibling-pane mutation: the listener closures live inside `Watchlist`,
//! but the listener would need `&mut Vec<Chart>` (and arguably `&mut
//! Watchlist` too) to do the sync — both already borrowed mutably by the
//! caller invoking `publish`. The Wave 5 listener could only `trace!`.
//!
//! Wave 12c flips the model: the bus is now a queue. Publishers push
//! `PaneEvent`s; the render loop drains the queue at a well-defined point
//! (end of `App::about_to_wait`, after per-pane `pending_*` handling) and
//! applies each event to the sibling panes from a context that holds the
//! `&mut Vec<Chart>` borrow without conflict.
//!
//! This is intentionally simpler than the listener model and serializes
//! the borrow conflict by deferring application. There is no per-listener
//! state — the application logic lives in one place in the render loop,
//! which makes it trivial to inspect and easy to test (`drain()` returns
//! the queue, no scheduling).
//!
//! ## Group sentinel
//!
//! `PaneEvent::SymbolChanged { group, .. }` carries a `u8` group id:
//!   - `0`       → do not propagate. Caller wants per-pane-only mutation.
//!                 (Publishers typically skip publishing in this case.)
//!   - `1..=254` → propagate to panes whose `chart.link_group == group`.
//!   - `255`     → broadcast — apply to all panes regardless of link group.
//!
//! Use [`BROADCAST_GROUP`] for the sentinel rather than the literal.

/// Sentinel value for `PaneEvent::SymbolChanged.group` (and friends)
/// meaning "broadcast to every pane, regardless of link group". Picked
/// as `0xFF` because real link groups index into
/// `Watchlist::link_groups`, which is bounded well below 255.
pub const BROADCAST_GROUP: u8 = 0xFF;

/// Identity of a single boolean-valued pane toggle. Each variant maps
/// 1:1 to a `Chart` field; the dispatcher in
/// `chart::renderer::gpu::apply_pane_events` writes that field on every
/// sibling pane targeted by the event's `group`.
///
/// New toggles are added by extending this enum and adding a match arm
/// in `apply_pane_events`. Keep variant names matching the `Chart`
/// field they map to so grep across the two files is trivial.
///
/// Cycling (non-bool) toggles live in a separate event variant
/// (`PaneEvent::SwingLegModeChanged`) — this enum is bool-valued only.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum PaneToggle {
    LogScale,
    OhlcTooltip,
    MeasureTooltip,
    ShowVolume,
    ShowDeltaVolume,
    ShowRvol,
    ShowMaRibbon,
    ShowCvd,
    ShowPrevClose,
    ShowPatternLabels,
    ShowFootprint,
    ShowAutoFib,
    HitHighlight,
}

/// Events that a pane (or panel acting on a pane) can publish for
/// sibling panes to react to. Variants are intentionally narrow so the
/// fanout site stays cheap and the listener side stays exhaustive.
#[derive(Debug, Clone)]
pub enum PaneEvent {
    /// A pane in `group` changed its `symbol`. Sibling panes in the
    /// same non-zero link group should follow. `group == BROADCAST_GROUP`
    /// means apply to every pane (broadcast mode).
    SymbolChanged { group: u8, symbol: String },
    /// A pane in `group` changed its `timeframe`. Sibling panes in the
    /// same non-zero link group should follow. `group == BROADCAST_GROUP`
    /// means apply to every pane.
    TimeframeChanged { group: u8, timeframe: String },
    /// A pane in `group` flipped a boolean display toggle. Sibling
    /// panes targeted by `group` should mirror `value` on the field
    /// identified by `kind`. Used by the top-nav indicator/tools/vol
    /// menus when the user holds Shift (or broadcast mode is on) to
    /// fan a toggle across panes.
    ToggleChanged { group: u8, kind: PaneToggle, value: bool },
    /// A pane in `group` cycled its `swing_leg_mode` (u8: 0=off,
    /// 1=vertical, 2=diagonal). Split from `ToggleChanged` because the
    /// value is tri-state, not bool.
    SwingLegModeChanged { group: u8, value: u8 },
    /// The user picked a different pane layout (1, 2H, 3L, ...).
    /// Subscribers may need to recompute focus, persist favorites, etc.
    LayoutChanged,
    /// Broadcast mode toggled. When `enabled`, toolbar actions apply
    /// to all panes regardless of link group.
    BroadcastEnabled { enabled: bool },
    /// Wave 14a: a pane in `group` flipped the `visible` flag on every
    /// instance of `kind` in its `indicators: Vec<Indicator>`. Sibling
    /// panes targeted by `group` should mirror the same mass-mutation
    /// (set `visible = visible` on every indicator whose `kind ==`).
    /// Distinct from `ToggleChanged` because the change is across N
    /// indicator instances rather than a single bool field on `Chart`.
    IndicatorVisibilityChanged {
        group: u8,
        kind: crate::chart::renderer::gpu::IndicatorType,
        visible: bool,
    },
    /// Wave 14a: a pane in `group` removed indicators by predicate.
    /// Siblings should `.retain` the negation. `period == None` means
    /// "remove all indicators of this kind" (the mass-clear case used
    /// by the MA dropdown's per-instance X button when broadcasting,
    /// where the predicate is `(kind == K && period == P)`). `period
    /// == Some(p)` restricts the predicate to that exact period.
    /// Indicator removal always also resets `indicator_bar_count` to
    /// 0 on the sibling (forces recompute against the new instance
    /// set), matching the imperative loop's behavior.
    IndicatorsRemoved {
        group: u8,
        kind: crate::chart::renderer::gpu::IndicatorType,
        period: Option<usize>,
    },
    /// Wave 14a: a pane in `group` added a new indicator instance.
    /// Each sibling clones the indicator, allocates a *fresh*
    /// per-pane id (from its own `next_indicator_id`), pushes it onto
    /// the pane's `indicators` Vec, and resets `indicator_bar_count`
    /// to 0. Cloning isolates per-pane mutable state (color rotation,
    /// edit-target id) from the originator's instance.
    IndicatorAdded {
        group: u8,
        indicator: crate::chart::renderer::gpu::Indicator,
    },
}

/// In-memory, queue-backed pub/sub bus. Each `Watchlist` owns one.
///
/// Publishers push events with [`publish`]; the render loop empties the
/// queue once per frame with [`drain`] and applies each event to the
/// panes. This sidesteps the `&mut Vec<Chart>` / `&mut Watchlist`
/// borrow-conflict that a fan-out-on-publish listener model hits.
pub struct SubscriptionBus {
    pending: Vec<(PaneEvent, Option<usize>)>,
}

impl SubscriptionBus {
    pub fn new() -> Self {
        Self { pending: Vec::new() }
    }

    /// Queue an event with no origin pane. The dispatcher applies
    /// matching events to every targeted pane (no originator skip).
    /// Use this when the publisher already mutated its own pane state
    /// and wants every other matching pane to follow — typical for
    /// out-of-band UI publishers (command palette, top nav toolbar).
    pub fn publish(&mut self, event: PaneEvent) {
        self.pending.push((event, None));
    }

    /// Queue an event tagged with its originating pane index. The
    /// dispatcher skips that pane during fan-out (it already has the
    /// new value applied). Used by the render-loop publisher that
    /// drives cross-pane symbol/timeframe sync.
    pub fn publish_from(&mut self, event: PaneEvent, origin: usize) {
        self.pending.push((event, Some(origin)));
    }

    /// Take all queued `(event, origin)` pairs, leaving the bus
    /// empty. The caller applies them (typically once per frame in
    /// the render loop). Returning an owned `Vec` avoids a borrow on
    /// `self` during the apply step (the caller needs `&mut Vec<Chart>`).
    pub fn drain(&mut self) -> Vec<(PaneEvent, Option<usize>)> {
        std::mem::take(&mut self.pending)
    }

    /// Number of events currently queued. Primarily for tests / metrics.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

impl Default for SubscriptionBus {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for SubscriptionBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubscriptionBus")
            .field("pending", &self.pending_count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_then_drain_returns_events_in_order() {
        let mut bus = SubscriptionBus::new();
        bus.publish(PaneEvent::SymbolChanged { group: 1, symbol: "AAPL".into() });
        bus.publish_from(PaneEvent::SymbolChanged { group: 1, symbol: "TSLA".into() }, 2);
        bus.publish(PaneEvent::LayoutChanged);
        assert_eq!(bus.pending_count(), 3);
        let drained = bus.drain();
        assert_eq!(drained.len(), 3);
        assert_eq!(bus.pending_count(), 0);
        match &drained[0] {
            (PaneEvent::SymbolChanged { group, symbol }, None) => {
                assert_eq!(*group, 1);
                assert_eq!(symbol, "AAPL");
            }
            _ => panic!("expected SymbolChanged with no origin"),
        }
        match &drained[1] {
            (PaneEvent::SymbolChanged { group, symbol }, Some(2)) => {
                assert_eq!(*group, 1);
                assert_eq!(symbol, "TSLA");
            }
            _ => panic!("expected SymbolChanged with origin=2"),
        }
        assert!(matches!(drained[2], (PaneEvent::LayoutChanged, None)));
    }

    #[test]
    fn drain_on_empty_bus_returns_empty_vec() {
        let mut bus = SubscriptionBus::new();
        assert!(bus.drain().is_empty());
    }

    #[test]
    fn broadcast_group_is_distinct_sentinel() {
        // Real link groups are u8 indices into Watchlist::link_groups
        // which never approaches 255 in practice. The sentinel must
        // not collide with any plausible real group id.
        assert_eq!(BROADCAST_GROUP, 0xFF);
        assert_ne!(BROADCAST_GROUP, 0);
        assert!(BROADCAST_GROUP > 16, "real link groups stay << 16");
    }
}
