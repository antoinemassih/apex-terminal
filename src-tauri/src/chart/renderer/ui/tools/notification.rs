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

use std::time::Instant;

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
