//! `Notification` — typed toast model for the transient event overlay.
//!
//! Replaces the legacy `(String, f32, bool)` tuple that lived in
//! `PENDING_TOASTS`.  Every push site now fills a `Notification`; the render
//! path (`top_nav.rs`) reads `NotificationSeverity` to pick the accent colour
//! from the active `Theme` instead of inspecting a stringly-typed prefix byte.
//!
//! ## Dedup
//! `NotificationQueue::push` collapses identical `message` strings that arrive
//! within the same logical "drain window" (same batch drain call).  Each drain
//! call first deduplicates the pending queue by message before returning it,
//! so repeated pushes from multiple panes in one frame surface as a single
//! notification.
//!
//! ## NotificationManager
//! All three stores (pending toasts, history, badge feed) are unified into a
//! single `NotificationManager` held in a render-thread `thread_local`.  Public
//! façade functions preserve the original call-site names so callers outside
//! this module do not change.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::time::Instant;

// ── Caps ──────────────────────────────────────────────────────────────────────
const HISTORY_CAP:  usize = 300;
const BADGE_CAP:    usize = 50;

// ── AlertKind / AlertItem (moved here from alert_feed.rs) ─────────────────────

/// Severity / source of the alert — controls badge colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AlertKind {
    /// Order fully filled.
    OrderFilled,
    /// Order rejected or cancelled.
    OrderRejected,
    /// Order submitted, awaiting fill.
    OrderPending,
    /// A price-level alert triggered.
    PriceAlert,
    /// A signal fired on a chart.
    Signal,
    /// Hard error (connection loss, broker rejection, etc.).
    Error,
    /// Soft warning.
    Warning,
}

impl AlertKind {
    /// Map this kind to the canonical `NotificationSeverity` so badge colours
    /// and icons flow from the single shared severity model rather than being
    /// maintained twice.
    ///
    /// | Kind           | Severity | Colour |
    /// |----------------|----------|--------|
    /// | OrderFilled    | Success  | bull / green |
    /// | OrderRejected  | Error    | bear / red   |
    /// | Error          | Error    | bear / red   |
    /// | Warning        | Warning  | warn / amber |
    /// | OrderPending   | Info     | accent       |
    /// | PriceAlert     | Info     | accent       |
    /// | Signal         | Info     | accent       |
    pub fn severity(self) -> NotificationSeverity {
        match self {
            AlertKind::OrderFilled                      => NotificationSeverity::Success,
            AlertKind::OrderRejected | AlertKind::Error => NotificationSeverity::Error,
            AlertKind::Warning                          => NotificationSeverity::Warning,
            AlertKind::OrderPending
            | AlertKind::PriceAlert
            | AlertKind::Signal                         => NotificationSeverity::Info,
        }
    }
}

/// A single entry in the badge feed.
#[derive(Debug, Clone)]
pub struct AlertItem {
    pub id: u64,
    pub kind: AlertKind,
    /// Optional ticker the alert relates to.
    pub symbol: Option<String>,
    /// Short human-readable description.
    pub message: String,
}

// ── NotificationManager ───────────────────────────────────────────────────────

/// Single backing store for all three notification surfaces:
///   - `pending`  — toasts queued for the current frame's drain
///   - `history`  — persistent log fed from each drain (cap 300, newest-last)
///   - `badges`   — badge feed items (cap 50, FIFO)
///
/// Lives in a render-thread `thread_local`; no cross-thread access needed.
pub struct NotificationManager {
    pending:        VecDeque<Notification>,
    history:        VecDeque<Notification>,
    badges:         VecDeque<AlertItem>,
    next_badge_id:  u64,
}

impl NotificationManager {
    const fn new() -> Self {
        Self {
            pending:       VecDeque::new(),
            history:       VecDeque::new(),
            badges:        VecDeque::new(),
            next_badge_id: 1,
        }
    }
}

thread_local! {
    static MANAGER: RefCell<NotificationManager> = const { RefCell::new(NotificationManager::new()) };
}

// ── Pending-toast façade (replaces PENDING_TOASTS) ───────────────────────────

// ── Routing: which surface (toolbar ticker / toasts) gets each notification ──

/// Coarse notification category used for user-configurable routing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NotifCategory { Orders, Signals, Alerts, System }

/// Where a category's notifications are delivered.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NotifDest { Off, Toolbar, Toast, Both }

impl NotifDest {
    pub fn to_u8(self) -> u8 {
        match self { NotifDest::Off => 0, NotifDest::Toolbar => 1, NotifDest::Toast => 2, NotifDest::Both => 3 }
    }
    pub fn from_u8(v: u8) -> Self {
        match v { 1 => NotifDest::Toolbar, 2 => NotifDest::Toast, 3 => NotifDest::Both, _ => NotifDest::Off }
    }
    fn to_toolbar(self) -> bool { matches!(self, NotifDest::Toolbar | NotifDest::Both) }
    fn to_toast(self)   -> bool { matches!(self, NotifDest::Toast | NotifDest::Both) }
}

/// Master enable flags + per-category destinations.
#[derive(Clone, Copy)]
pub struct RoutingPrefs {
    pub toolbar_enabled: bool,
    pub toasts_enabled:  bool,
    pub orders:  NotifDest,
    pub signals: NotifDest,
    pub alerts:  NotifDest,
    pub system:  NotifDest,
}

impl Default for RoutingPrefs {
    fn default() -> Self {
        Self {
            toolbar_enabled: true,
            toasts_enabled:  true,
            // Trading events → toolbar ticker; system/connection → toasts.
            orders:  NotifDest::Toolbar,
            signals: NotifDest::Toolbar,
            alerts:  NotifDest::Toolbar,
            system:  NotifDest::Toast,
        }
    }
}

impl RoutingPrefs {
    pub fn dest(&self, cat: NotifCategory) -> NotifDest {
        match cat {
            NotifCategory::Orders  => self.orders,
            NotifCategory::Signals => self.signals,
            NotifCategory::Alerts  => self.alerts,
            NotifCategory::System  => self.system,
        }
    }
}

thread_local! {
    static ROUTING: std::cell::Cell<RoutingPrefs> = std::cell::Cell::new(RoutingPrefs::default());
}

/// Update the live routing prefs (called once per frame from the render with the
/// persisted user settings).
pub fn set_routing(p: RoutingPrefs) { ROUTING.with(|r| r.set(p)); }
/// Current routing prefs (read by `push_pending` and the surface renderers).
pub fn routing() -> RoutingPrefs { ROUTING.with(|r| r.get()) }

/// Classify a notification by source for routing.
fn category_for(source: Option<&'static str>) -> NotifCategory {
    match source {
        Some("orders")                                        => NotifCategory::Orders,
        Some("alerts") | Some("price_alert")                  => NotifCategory::Alerts,
        Some("precursor") | Some("trade_plan") | Some("feed") => NotifCategory::Signals,
        _                                                     => NotifCategory::System,
    }
}

/// Toolbar badge kind for a category + severity.
fn kind_for_category(cat: NotifCategory, sev: NotificationSeverity) -> AlertKind {
    match cat {
        NotifCategory::Orders => match sev {
            NotificationSeverity::Success => AlertKind::OrderFilled,
            NotificationSeverity::Error   => AlertKind::OrderRejected,
            _                             => AlertKind::OrderPending,
        },
        NotifCategory::Signals => AlertKind::Signal,
        NotifCategory::Alerts  => AlertKind::PriceAlert,
        NotifCategory::System  => match sev {
            NotificationSeverity::Error => AlertKind::Error,
            _                           => AlertKind::Warning,
        },
    }
}

/// Read the persisted routing prefs out of egui memory (with defaults).
pub fn routing_from_ctx(ctx: &egui::Context) -> RoutingPrefs {
    use egui::Id;
    let gb = |k: &str, d: bool| ctx.data_mut(|dd| dd.get_persisted::<bool>(Id::new(k))).unwrap_or(d);
    let gd = |k: &str, d: NotifDest|
        NotifDest::from_u8(ctx.data_mut(|dd| dd.get_persisted::<u8>(Id::new(k))).unwrap_or(d.to_u8()));
    RoutingPrefs {
        toolbar_enabled: gb("notif_toolbar_enabled", true),
        toasts_enabled:  gb("notif_toasts_enabled",  true),
        orders:  gd("notif_route_orders",  NotifDest::Toolbar),
        signals: gd("notif_route_signals", NotifDest::Toolbar),
        alerts:  gd("notif_route_alerts",  NotifDest::Toolbar),
        system:  gd("notif_route_system",  NotifDest::Toast),
    }
}

/// Persist routing prefs into egui memory (survives restart).
pub fn routing_to_ctx(ctx: &egui::Context, p: RoutingPrefs) {
    use egui::Id;
    ctx.data_mut(|dd| {
        dd.insert_persisted(Id::new("notif_toolbar_enabled"), p.toolbar_enabled);
        dd.insert_persisted(Id::new("notif_toasts_enabled"),  p.toasts_enabled);
        dd.insert_persisted(Id::new("notif_route_orders"),  p.orders.to_u8());
        dd.insert_persisted(Id::new("notif_route_signals"), p.signals.to_u8());
        dd.insert_persisted(Id::new("notif_route_alerts"),  p.alerts.to_u8());
        dd.insert_persisted(Id::new("notif_route_system"),  p.system.to_u8());
    });
}

/// Push a notification, routing it to the toolbar ticker and/or the toast stack
/// per the user's routing prefs (master enables + per-category destination).
pub fn push_pending(n: Notification) {
    let r = routing();
    let cat = category_for(n.source);
    let dest = r.dest(cat);
    if r.toolbar_enabled && dest.to_toolbar() {
        // Separate manager borrow, fully released before the pending push below.
        push_badge(kind_for_category(cat, n.severity), None, n.message.clone());
    }
    if r.toasts_enabled && dest.to_toast() {
        MANAGER.with(|m| m.borrow_mut().pending.push_back(n));
    }
}

/// Drain all pending notifications, returning them as a `Vec`.
/// Called once per frame at the toast-drain site in `gpu.rs`.
pub fn drain_pending() -> Vec<Notification> {
    MANAGER.with(|m| m.borrow_mut().pending.drain(..).collect())
}

// ── History façade (same signatures as before) ───────────────────────────────

/// Append freshly-drained notifications to the history log (oldest trimmed).
pub fn record_history(items: &[Notification]) {
    if items.is_empty() { return; }
    MANAGER.with(|m| {
        let mut mgr = m.borrow_mut();
        for n in items { mgr.history.push_back(n.clone()); }
        while mgr.history.len() > HISTORY_CAP { mgr.history.pop_front(); }
    });
}

/// Snapshot the notification history, newest first.
pub fn history_snapshot() -> Vec<Notification> {
    MANAGER.with(|m| m.borrow().history.iter().rev().cloned().collect())
}

// ── Badge-feed façade (replaces alert_feed::ALERTS) ──────────────────────────

/// Push a new badge onto the feed. Oldest entry is dropped when the cap is reached.
pub fn push_badge(kind: AlertKind, symbol: Option<String>, message: impl Into<String>) {
    MANAGER.with(|m| {
        let mut mgr = m.borrow_mut();
        let id = mgr.next_badge_id;
        mgr.next_badge_id += 1;
        if mgr.badges.len() >= BADGE_CAP { mgr.badges.pop_front(); }
        mgr.badges.push_back(AlertItem { id, kind, symbol, message: message.into() });
    });
}

/// Snapshot all current badge-feed items (cloned, in push order).
pub fn badges_snapshot() -> Vec<AlertItem> {
    MANAGER.with(|m| m.borrow().badges.iter().cloned().collect())
}

/// Dismiss (remove) a single badge by id.
pub fn dismiss_badge(id: u64) {
    MANAGER.with(|m| m.borrow_mut().badges.retain(|a| a.id != id));
}

/// Remove all badges from the feed.
pub fn clear_all_badges() {
    MANAGER.with(|m| m.borrow_mut().badges.clear());
}

/// Severity level for a `Notification`.  Maps to a `Theme` colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotificationSeverity {
    /// Neutral informational (accent tint).
    Info,
    /// Something succeeded (bull / green tint).
    Success,
    /// Non-critical heads-up (warn / amber tint).
    Warning,
    /// Error / rejection / critical (bear / red tint).
    Error,
}

/// Alpha value applied to the severity accent colour when filling a notification
/// chrome background (e.g. toast pill, badge tint). Keeps the background subtle
/// so text legibility is preserved against dark panel backgrounds.
pub const NOTIF_TINT_ALPHA: u8 = 24;

/// Alpha value for a notification border/accent bar drawn at full perceptual
/// weight — visible enough to be instantly recognisable without being overpowering.
pub const NOTIF_BORDER_ALPHA: u8 = 200;

impl NotificationSeverity {
    /// Resolve to the appropriate theme colour.
    pub fn color(self, t: &crate::chart_renderer::gpu::Theme) -> egui::Color32 {
        match self {
            NotificationSeverity::Info    => t.accent,
            NotificationSeverity::Success => t.bull,
            NotificationSeverity::Warning => t.warn,
            NotificationSeverity::Error   => t.bear,
        }
    }

    /// Canonical Phosphor icon glyph for this severity level.
    ///
    /// Matches the icons already used by the toast overlay in `top_nav.rs` so
    /// every surface that renders a notification (toasts, badge feed, history
    /// dock) displays a consistent icon without duplicating the mapping.
    pub fn icon(self) -> &'static str {
        use crate::ui_kit::icons::Icon;
        match self {
            NotificationSeverity::Info    => Icon::INFO,
            NotificationSeverity::Success => Icon::CHECK_CIRCLE,
            NotificationSeverity::Warning => Icon::WARNING,
            NotificationSeverity::Error   => Icon::SHIELD_WARNING_FILL,
        }
    }
}

/// A typed, transient UI notification ("toast").
#[derive(Debug, Clone)]
pub struct Notification {
    /// Unique id — a stable hash of `(message, created)` used to key egui
    /// temp-memory for pin state.  Computed lazily when first pushed.
    pub id: u64,
    /// Human-readable message text.
    pub message: String,
    /// Severity drives accent colour.
    pub severity: NotificationSeverity,
    /// Optional source label (subsystem name).  Not rendered yet but carried
    /// for future structured display.
    pub source: Option<&'static str>,
    /// Wall-clock time the notification was created.
    pub created: Instant,
    /// Legacy float payload (price / conviction / lead_minutes).  Preserved
    /// so the render path can display it if desired; callers that have no
    /// meaningful float pass `0.0`.
    pub value: f32,
}

impl Notification {
    /// Construct with the minimum required fields.
    pub fn new(message: impl Into<String>, severity: NotificationSeverity) -> Self {
        let message = message.into();
        let created = Instant::now();
        let id = {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut h = DefaultHasher::new();
            message.hash(&mut h);
            created.elapsed().as_nanos().hash(&mut h);
            h.finish()
        };
        Self {
            id,
            message,
            severity,
            source: None,
            created,
            value: 0.0,
        }
    }

    /// Attach an optional float value (price / conviction / etc.).
    pub fn with_value(mut self, v: f32) -> Self { self.value = v; self }

    /// Attach an optional source tag.
    pub fn with_source(mut self, src: &'static str) -> Self { self.source = Some(src); self }
}

// ── Severity mapping from legacy encoding ────────────────────────────────────

/// Map the old `is_buy: bool` flag onto a `NotificationSeverity`.
///
/// Pre-`Notification`, every push used a bare bool: `true` mapped to a bull
/// (green) accent and `false` mapped to the neutral accent path.  After the
/// model upgrade:
///   - `true`  → `Success`  (buy-fill, alert triggered above, plan posted)
///   - `false` → `Info`     (neutral events, info toasts)
pub fn severity_from_bool(is_buy: bool) -> NotificationSeverity {
    if is_buy { NotificationSeverity::Success } else { NotificationSeverity::Info }
}

/// Map the legacy control-byte prefix used by `apex_data::live_state` onto a
/// `NotificationSeverity`, and strip the prefix from the message.
///
/// Byte vocabulary (mirrors the comment in `live_state.rs`):
///   `\x01` → Warning
///   `\x02` → Error (danger)
///   `\x03` → Error (critical — callers treat the same as Error for colour)
///   `\x04` → Success
///   no prefix → Info
pub fn decode_apex_message(msg: &str) -> (&str, NotificationSeverity) {
    if let Some(rest) = msg.strip_prefix('\x01') {
        (rest, NotificationSeverity::Warning)
    } else if let Some(rest) = msg.strip_prefix('\x02') {
        (rest, NotificationSeverity::Error)
    } else if let Some(rest) = msg.strip_prefix('\x03') {
        (rest, NotificationSeverity::Error)
    } else if let Some(rest) = msg.strip_prefix('\x04') {
        (rest, NotificationSeverity::Success)
    } else {
        (msg, NotificationSeverity::Info)
    }
}

/// Map an order-manager toast message to a severity.
///
/// Convention used by `order_manager.rs`:
///   "FILLED …"  → Success
///   "PARTIAL …" → Success
///   "REJECTED…" → Error
///   "CANCEL…"   → Warning
///   everything else → Info
pub fn severity_for_order_msg(msg: &str) -> NotificationSeverity {
    let upper = msg.to_ascii_uppercase();
    if upper.starts_with("FILLED") || upper.starts_with("PARTIAL") {
        NotificationSeverity::Success
    } else if upper.starts_with("REJECT") {
        NotificationSeverity::Error
    } else if upper.starts_with("CANCEL") {
        NotificationSeverity::Warning
    } else {
        NotificationSeverity::Info
    }
}
