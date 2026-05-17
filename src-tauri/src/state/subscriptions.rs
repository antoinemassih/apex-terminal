//! Cross-pane event subscriptions — replaces ad-hoc manual iteration over
//! panes for link-group propagation, broadcast mode, layout sync, etc.
//!
//! Channels are *synchronous* — egui's idiom is synchronous within a
//! frame, and the cost of `Vec::iter_mut` over a handful of listeners is
//! negligible compared to even one allocation per event. Fanout happens
//! immediately when an event is `publish`ed.
//!
//! Each `Watchlist` owns one `SubscriptionBus`. Panels that mutate
//! pane-shared state (symbol, timeframe, layout, broadcast) `publish`
//! events; the bus's listeners (registered at `Watchlist::default`
//! construction or by panel init code) fan the change out to sibling
//! panes. Today only one publish site is wired (command palette symbol
//! change) — see `chart::renderer::ui::command_palette::execute`. Future
//! waves migrate the remaining imperative `for chart in panes.iter_mut`
//! sites in `gpu.rs` onto this bus.

/// Events that a pane (or panel acting on a pane) can publish for
/// sibling panes to react to. Variants are intentionally narrow so the
/// fanout site stays cheap and the listener side stays exhaustive.
#[derive(Debug, Clone)]
pub enum PaneEvent {
    /// A pane in `group` changed its `symbol`. Sibling panes in the
    /// same non-zero link group should follow.
    SymbolChanged { group: u8, symbol: String },
    /// A pane in `group` changed its `timeframe`. Sibling panes in the
    /// same non-zero link group should follow.
    TimeframeChanged { group: u8, timeframe: String },
    /// The user picked a different pane layout (1, 2H, 3L, ...).
    /// Subscribers may need to recompute focus, persist favorites, etc.
    LayoutChanged,
    /// Broadcast mode toggled. When `enabled`, toolbar actions apply
    /// to all panes regardless of link group.
    BroadcastEnabled { enabled: bool },
}

/// In-memory, synchronous pub/sub bus. Each `Watchlist` owns one.
///
/// Listeners are boxed `FnMut(&PaneEvent)` so they can capture mutable
/// state (e.g. a counter, a flag, a `Vec<u8>` of pending group ids).
/// They cannot capture `&mut Watchlist` directly because the bus lives
/// inside it — for now, listeners stage their intent and the caller
/// drains the staged intent after `publish` returns. Future waves can
/// migrate toward a typed sink if the pattern proves load-bearing.
pub struct SubscriptionBus {
    listeners: Vec<Box<dyn FnMut(&PaneEvent) + Send>>,
}

impl SubscriptionBus {
    pub fn new() -> Self {
        Self { listeners: Vec::new() }
    }

    /// Register a new listener. Listeners are invoked in registration order
    /// for every published event.
    pub fn on(&mut self, listener: impl FnMut(&PaneEvent) + Send + 'static) {
        self.listeners.push(Box::new(listener));
    }

    /// Synchronously fan an event out to every registered listener.
    pub fn publish(&mut self, event: PaneEvent) {
        for l in self.listeners.iter_mut() {
            l(&event);
        }
    }

    /// Returns the number of registered listeners. Primarily for tests
    /// and operator observability — there is no production fast-path
    /// that needs this.
    pub fn listener_count(&self) -> usize {
        self.listeners.len()
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
            .field("listeners", &self.listener_count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn publish_fans_out_to_all_listeners() {
        let mut bus = SubscriptionBus::new();
        let seen_a = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen_b = Arc::new(Mutex::new(Vec::<String>::new()));
        {
            let s = seen_a.clone();
            bus.on(move |evt| {
                if let PaneEvent::SymbolChanged { symbol, .. } = evt {
                    s.lock().unwrap().push(symbol.clone());
                }
            });
        }
        {
            let s = seen_b.clone();
            bus.on(move |evt| {
                if let PaneEvent::SymbolChanged { symbol, .. } = evt {
                    s.lock().unwrap().push(symbol.clone());
                }
            });
        }
        assert_eq!(bus.listener_count(), 2);
        bus.publish(PaneEvent::SymbolChanged { group: 1, symbol: "AAPL".into() });
        bus.publish(PaneEvent::SymbolChanged { group: 1, symbol: "TSLA".into() });
        assert_eq!(*seen_a.lock().unwrap(), vec!["AAPL", "TSLA"]);
        assert_eq!(*seen_b.lock().unwrap(), vec!["AAPL", "TSLA"]);
    }

    #[test]
    fn listeners_filter_by_event_kind() {
        let mut bus = SubscriptionBus::new();
        let layout_changes = Arc::new(Mutex::new(0u32));
        {
            let lc = layout_changes.clone();
            bus.on(move |evt| {
                if matches!(evt, PaneEvent::LayoutChanged) {
                    *lc.lock().unwrap() += 1;
                }
            });
        }
        bus.publish(PaneEvent::SymbolChanged { group: 0, symbol: "X".into() });
        bus.publish(PaneEvent::LayoutChanged);
        bus.publish(PaneEvent::BroadcastEnabled { enabled: true });
        bus.publish(PaneEvent::LayoutChanged);
        assert_eq!(*layout_changes.lock().unwrap(), 2);
    }
}
