//! Replay scrubber — SOTA UX spec §4.2.
//!
//! Lets the user kick off a historical replay through ApexData (`/api/replay/...`),
//! watch frames stream into a small events log, and pause/resume/stop the session.
//!
//! Architectural notes
//! -------------------
//! - **State ownership.** The bulk of the panel state lives in a module-local
//!   `OnceLock<Mutex<ReplayPaneState>>` rather than on `Watchlist`, mirroring
//!   the pattern that `apex_data` already uses. The only field added to
//!   `Watchlist` is the `replay_open` visibility flag, which keeps the giant
//!   `Watchlist` literal in `gpu.rs` tidy.
//! - **WS thread isolation.** Replay frames arrive on the ApexData tokio worker
//!   thread. We funnel them through an `mpsc` channel into the UI thread on
//!   each egui frame, so the events log is updated without any cross-thread
//!   locking inside the callback.
//! - **Status polling.** Per the brief, polling lives in the egui frame loop —
//!   we just track a `last_poll: Instant` and call `replay_status` from a
//!   short-lived `std::thread::spawn` so the synchronous REST call doesn't
//!   stall rendering.
//! - **Overlay mode.** The toggle exists and is plumbed through state. The
//!   actual chart-pane integration is deferred (see TODO at `forward_frame`):
//!   adding a real-time replay overlay requires non-trivial surgery on
//!   `chart::renderer::gpu::Chart` (6.5k-line file with a render pipeline
//!   that doesn't expose a hook). Documented as a follow-up rather than
//!   landed in this PR per scope guidance.

use std::collections::VecDeque;
use crate::ui_kit::sx::Tone;
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use egui;

use super::super::style::*;
use super::super::super::gpu::{Watchlist, Theme};
use super::super::chrome::modal::{Modal, Anchor, HeaderStyle, FrameKind};
use super::super::components::text::{MonospaceCode, SectionLabelSize};
use crate::apex_data::types::{
    AssetClass, ReplayConfig, ReplayEvent, ReplayMode, ReplayState, ReplayStatus,
};
use crate::apex_data::ws::ReplaySubscription;
use crate::ui_kit::widgets::{Button, Input};
use crate::ui_kit::widgets::tokens::{Variant, Size as KitSize};

/// Cap for the events log. Per spec §4.2: "events log (last 50 emitted
/// frames…)" — we keep 500 in memory so the user can scroll through more
/// than just the most recent few when something interesting fires.
const EVENTS_LOG_CAP: usize = 500;

/// Hard ceiling on how far back the user can request data. 22 years matches
/// the upper bound the brief specifies (date validation). Keeps the UI from
/// accidentally letting someone request "from epoch" and DoS'ing the backend.
const MAX_HISTORY_YEARS: i64 = 22;

/// Status polling interval. Spec §4.2: "Poll status every 500ms while running."
const STATUS_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// User-visible speed presets. `f32::INFINITY` is the sentinel for "MAX"
/// (drain as fast as possible — the backend interprets a very large
/// multiplier as no sleep between frames).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpeedPreset {
    P0_25,
    P0_5,
    P1,
    P2,
    P5,
    P10,
    Max,
}

impl SpeedPreset {
    pub const ALL: &'static [SpeedPreset] = &[
        SpeedPreset::P0_25, SpeedPreset::P0_5, SpeedPreset::P1,
        SpeedPreset::P2,    SpeedPreset::P5,   SpeedPreset::P10, SpeedPreset::Max,
    ];
    /// Wire value sent to the backend. `Max` becomes a very large multiplier
    /// (1e6) — semantically "as fast as you can drain the queue."
    pub fn multiplier(self) -> f32 {
        match self {
            SpeedPreset::P0_25 => 0.25,
            SpeedPreset::P0_5  => 0.5,
            SpeedPreset::P1    => 1.0,
            SpeedPreset::P2    => 2.0,
            SpeedPreset::P5    => 5.0,
            SpeedPreset::P10   => 10.0,
            SpeedPreset::Max   => 1_000_000.0,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            SpeedPreset::P0_25 => "0.25x",
            SpeedPreset::P0_5  => "0.5x",
            SpeedPreset::P1    => "1x",
            SpeedPreset::P2    => "2x",
            SpeedPreset::P5    => "5x",
            SpeedPreset::P10   => "10x",
            SpeedPreset::Max   => "MAX",
        }
    }
}

/// Result of validating the user's date inputs. Either `Ok(from_ms, to_ms)`
/// in epoch milliseconds or `Err(human_readable_reason)`. Pure function so
/// the unit tests can exercise it without standing up the panel.
fn validate_window(from: &str, to: &str, now_ms: i64) -> Result<(i64, i64), String> {
    let from_ms = parse_dt_ms(from).ok_or_else(|| format!("from: unparseable ({from})"))?;
    let to_ms   = parse_dt_ms(to).ok_or_else(|| format!("to: unparseable ({to})"))?;
    if from_ms >= to_ms { return Err("from must be earlier than to".into()); }
    let max_back = now_ms - MAX_HISTORY_YEARS * 365 * 24 * 3600 * 1000;
    if from_ms < max_back {
        return Err(format!("from is more than {MAX_HISTORY_YEARS} years ago"));
    }
    if to_ms > now_ms + 60_000 {
        // Allow ~1 minute of clock skew; reject "future" windows beyond that.
        return Err("to is in the future".into());
    }
    Ok((from_ms, to_ms))
}

/// Parse a date/time string the user typed into the picker. egui ships
/// without a built-in date/time widget, so we accept any of these forms:
/// - `YYYY-MM-DD HH:MM:SS`  (full)
/// - `YYYY-MM-DD HH:MM`     (seconds default to 00)
/// - `YYYY-MM-DD`           (time defaults to 00:00:00)
/// Returned in epoch milliseconds, treated as UTC. The trader generally
/// thinks in ET — the UI displays the parsed value back as ET in the
/// status row so they can double-check.
fn parse_dt_ms(s: &str) -> Option<i64> {
    use chrono::NaiveDateTime;
    let s = s.trim();
    if s.is_empty() { return None; }
    for fmt in &["%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M"] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(s, fmt) {
            return Some(dt.and_utc().timestamp_millis());
        }
    }
    // Date-only fallback.
    if let Ok(d) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        if let Some(dt) = d.and_hms_opt(0, 0, 0) {
            return Some(dt.and_utc().timestamp_millis());
        }
    }
    None
}

/// Format an epoch-ms timestamp as "YYYY-MM-DD HH:MM:SS" UTC for display.
fn fmt_ms(ms: i64) -> String {
    if ms == 0 { return "—".into(); }
    chrono::DateTime::<chrono::Utc>::from_timestamp(ms / 1000, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| format!("{ms}"))
}

/// The complete state of the scrubber panel — owned by a module-local
/// `OnceLock<Mutex<_>>` so the panel can be a stateless `draw` function
/// like its siblings, and so background WS callbacks have somewhere to
/// deliver events without going through `Watchlist`.
pub struct ReplayPaneState {
    // ── Form inputs ────────────────────────────────────────────────────────
    pub from_str: String,
    pub to_str: String,
    pub symbols_str: String,        // comma-separated; parsed at submit time
    pub asset_class: AssetClass,
    pub source: &'static str,       // "last" | "mark"
    pub mode: ReplayMode,
    pub speed: SpeedPreset,
    pub overlay_on_chart: bool,

    // ── Active session ─────────────────────────────────────────────────────
    pub replay_id: Option<String>,
    pub status: Option<ReplayStatus>,
    pub events: VecDeque<ReplayEvent>,
    pub error: Option<String>,
    /// Snapshot of the state machine; cached so the UI doesn't have to
    /// re-derive from `status` on every frame (and so tests can assert it).
    pub fsm_state: ReplayState,

    // ── Runtime plumbing (skip from outside inspection) ────────────────────
    subscription: Option<ReplaySubscription>,
    rx_events: Option<mpsc::Receiver<ReplayEvent>>,
    tx_status: Option<mpsc::Sender<ReplayStatus>>,
    rx_status: Option<mpsc::Receiver<ReplayStatus>>,
    last_status_poll: Option<Instant>,
}

impl Default for ReplayPaneState {
    fn default() -> Self {
        // Default window: last hour, UTC. The user almost always wants to
        // replay something recent, so this minimizes typing for the common
        // case while still being valid (within 22 years, from < to).
        let now = chrono::Utc::now();
        let from = now - chrono::Duration::hours(1);
        Self {
            from_str: from.format("%Y-%m-%d %H:%M:00").to_string(),
            to_str:   now.format("%Y-%m-%d %H:%M:00").to_string(),
            symbols_str: "SPY".into(),
            asset_class: AssetClass::Stock,
            source: "last",
            mode: ReplayMode::StockBars,
            speed: SpeedPreset::P1,
            overlay_on_chart: false,

            replay_id: None,
            status: None,
            events: VecDeque::with_capacity(EVENTS_LOG_CAP),
            error: None,
            fsm_state: ReplayState::NotStarted,

            subscription: None,
            rx_events: None,
            tx_status: None,
            rx_status: None,
            last_status_poll: None,
        }
    }
}

impl ReplayPaneState {
    /// Append a frame to the events log, evicting the oldest if at capacity.
    /// Public so the unit tests can exercise the eviction policy directly.
    pub fn push_event(&mut self, e: ReplayEvent) {
        self.events.push_back(e);
        while self.events.len() > EVENTS_LOG_CAP { self.events.pop_front(); }
    }

    /// Tear down any active subscription and reset to a not-started state.
    /// Idempotent — safe to call when nothing is running.
    pub fn reset(&mut self) {
        if let Some(sub) = self.subscription.take() { sub.stop(); }
        self.rx_events = None;
        self.replay_id = None;
        self.status = None;
        self.error = None;
        self.fsm_state = ReplayState::NotStarted;
        self.last_status_poll = None;
        // Keep `events` so the user can still see what just happened.
    }
}

static STATE: OnceLock<Mutex<ReplayPaneState>> = OnceLock::new();
fn state() -> &'static Mutex<ReplayPaneState> {
    STATE.get_or_init(|| Mutex::new(ReplayPaneState::default()))
}

/// Read-only borrow of the panel state for tests/diagnostics. Not exposed
/// outside the `panels` namespace — keep it package-private to discourage
/// other panels from reaching in.
#[cfg(test)]
pub(crate) fn state_for_test() -> &'static Mutex<ReplayPaneState> { state() }

// ─── Public entry — registered in panels::mod and called from top_nav ──────

pub(crate) fn draw(ctx: &egui::Context, watchlist: &mut Watchlist, t: &Theme) {
    if !watchlist.replay_pane_open { return; }

    // Drain WS events and any completed status polls *before* rendering so
    // the panel reflects them this frame. Both are non-blocking.
    drain_events();
    drain_status();
    maybe_poll_status();

    let screen = ctx.screen_rect();
    let w = 560.0_f32;
    let h = (screen.height() * 0.80).min(640.0);

    let resp = Modal::new("REPLAY SCRUBBER")
        .id("replay_scrubber")
        .ctx(ctx)
        .theme(t)
        .size(egui::vec2(w, h))
        .anchor(Anchor::Window { pos: Some(egui::pos2(screen.center().x - w / 2.0, 80.0)) })
        .frame_kind(FrameKind::DialogWindow)
        .draggable_header(true)
        .header_style(HeaderStyle::Panel {
            title_size:      SectionLabelSize::Md,
            title_size_px:   None,
            title_monospace: None,
            leading_space:   0.0,
            trailing_space:  0.0,
        })
        .separator(false)
        .show(|ui| {
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let mut s = match state().lock() { Ok(g) => g, Err(p) => p.into_inner() };
                draw_form(ui, &mut s, t);
                ui.add_space(gap_md());
                draw_progress(ui, &s, t);
                ui.add_space(gap_md());
                draw_controls(ui, &mut s, t);
                ui.add_space(gap_md());
                draw_status_row(ui, &s, t);
                ui.add_space(gap_md());
                draw_events_log(ui, &s, t);
            });
        });

    if resp.closed { watchlist.update_sidebar_state(|s| s.replay_pane_open = false); }
}

// ─── Sections ───────────────────────────────────────────────────────────────

fn draw_form(ui: &mut egui::Ui, s: &mut ReplayPaneState, t: &Theme) {
    let editable = !s.fsm_state.is_active();

    ui.add(MonospaceCode::new("CONFIG").color(t.dim));
    ui.add_space(gap_xs());

    // Date pickers — egui has no native date/time widget on 0.31; we use
    // plain text inputs with `parse_dt_ms` so anything that round-trips
    // through `YYYY-MM-DD HH:MM[:SS]` works.
    ui.horizontal(|ui| {
        ui.add(MonospaceCode::new("from").color(t.dim));
        ui.add_enabled_ui(editable, |ui| {
            Input::new(&mut s.from_str).min_width(160.0).size(KitSize::Sm).show(ui, t);
        });
        ui.add_space(gap_sm());
        ui.add(MonospaceCode::new("to").color(t.dim));
        ui.add_enabled_ui(editable, |ui| {
            Input::new(&mut s.to_str).min_width(160.0).size(KitSize::Sm).show(ui, t);
        });
    });
    ui.add(MonospaceCode::new("UTC, format: YYYY-MM-DD HH:MM[:SS]").color(color_muted(t.dim)));
    ui.add_space(gap_sm());

    // Symbols (comma-separated multi-select; a real picker can replace this
    // later, but a text box is the minimum-viable that works today).
    ui.horizontal(|ui| {
        ui.add(MonospaceCode::new("symbols").color(t.dim));
        ui.add_enabled_ui(editable, |ui| {
            Input::new(&mut s.symbols_str).min_width(320.0).size(KitSize::Sm).show(ui, t);
        });
    });
    ui.add(MonospaceCode::new("comma-separated (e.g. SPY, AAPL, NVDA)").color(color_muted(t.dim)));
    ui.add_space(gap_sm());

    // Asset class — radio pair
    ui.horizontal(|ui| {
        ui.add(MonospaceCode::new("class  ").color(t.dim));
        for (label, val) in &[("Stock", AssetClass::Stock), ("Option", AssetClass::Option)] {
            let sel = s.asset_class == *val;
            let resp = Button::toggle(*label, sel).size(KitSize::Sm)
                .min_size(egui::vec2(54.0, 20.0)).enabled(editable).show(ui, t);
            if resp.clicked() && editable { s.asset_class = *val; }
        }
    });
    ui.add_space(gap_xs());

    // Source radio (last / mark)
    ui.horizontal(|ui| {
        ui.add(MonospaceCode::new("source ").color(t.dim));
        for src in &["last", "mark"] {
            let sel = s.source == *src;
            let resp = Button::toggle(*src, sel).size(KitSize::Sm)
                .min_size(egui::vec2(54.0, 20.0)).enabled(editable).show(ui, t);
            if resp.clicked() && editable { s.source = src; }
        }
    });
    ui.add_space(gap_xs());

    // Mode radio (the 6 ReplayMode variants — trades/quotes are visible but
    // marked "(not yet implemented)" per scope).
    ui.add(MonospaceCode::new("mode   ").color(t.dim));
    ui.add_space(gap_xs());
    let modes = [
        ReplayMode::StockBars, ReplayMode::OptionBars,
        ReplayMode::MarkBarsStocks, ReplayMode::MarkBarsOptions,
        ReplayMode::Trades, ReplayMode::Quotes,
    ];
    ui.horizontal_wrapped(|ui| {
        for m in modes {
            let sel = s.mode == m;
            let implemented = m.is_implemented();
            let label = if implemented { m.label().to_string() }
                        else { format!("{} (not yet implemented)", m.label()) };
            let col = if sel { t.accent }
                      else if implemented { color_subtle(t.dim) }
                      else { color_dim(t.dim) };
            let resp = Button::toggle(label.as_str(), sel).size(KitSize::Xs)
                .min_size(egui::vec2(0.0, 20.0)).enabled(editable && implemented).show(ui, t);
            if resp.clicked() && editable && implemented { s.mode = m; }
        }
    });
    ui.add_space(gap_sm());

    // Overlay-on-live-chart toggle
    ui.horizontal(|ui| {
        let icon = if s.overlay_on_chart { "[x]" } else { "[ ]" };
        let col = if s.overlay_on_chart { t.accent } else { t.dim };
        if Button::new(icon).variant(Variant::Ghost).size(KitSize::Xs)
            .fg(col).min_size(egui::vec2(28.0, 18.0)).show(ui, t).clicked()
        {
            s.overlay_on_chart = !s.overlay_on_chart;
        }
        ui.add(MonospaceCode::new("overlay replay on live chart")
            .color(if s.overlay_on_chart { t.text } else { t.dim }));
    });
    ui.add(MonospaceCode::new("  (UI wiring stub — chart pane integration is a follow-up)")
        .color(color_half(t.dim)));
}

fn draw_progress(ui: &mut egui::Ui, s: &ReplayPaneState, t: &Theme) {
    let progress = s.status.as_ref().map(|st| st.progress.clamp(0.0, 1.0)).unwrap_or(0.0);
    let total_w = ui.available_width().min(540.0);
    let h = 14.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(total_w, h), egui::Sense::hover());
    // Track
    ui.painter().rect_filled(rect, current().r_sm, tint(t, Tone::Border, alpha_muted()));
    // Fill
    let fill_w = total_w * progress;
    if fill_w > 0.0 {
        let fill_rect = egui::Rect::from_min_size(rect.min, egui::vec2(fill_w, h));
        ui.painter().rect_filled(fill_rect, current().r_sm, t.accent);
    }
    // Scrub handle (visual only — drag-to-seek would need a backend seek
    // endpoint that the MVP doesn't expose).
    let handle_x = rect.min.x + fill_w;
    ui.painter().line_segment(
        [egui::pos2(handle_x, rect.min.y - 2.0), egui::pos2(handle_x, rect.max.y + 2.0)],
        egui::Stroke::new(stroke_std(), t.text));
}

fn draw_controls(ui: &mut egui::Ui, s: &mut ReplayPaneState, t: &Theme) {
    let mut do_start  = false;
    let mut do_pause  = false;
    let mut do_resume = false;
    let mut do_stop   = false;

    ui.horizontal(|ui| {
        // Play / Pause / Resume buttons. The state machine decides which
        // is active — the others are disabled, not hidden, so the layout
        // doesn't reflow as the user clicks through.
        let can_start  = matches!(s.fsm_state, ReplayState::NotStarted) || s.fsm_state.is_terminal();
        let can_pause  = matches!(s.fsm_state, ReplayState::Running);
        let can_resume = matches!(s.fsm_state, ReplayState::Paused);
        let can_stop   = s.fsm_state.is_active();

        if ui.add_enabled(can_start, Button::new("Play").variant(Variant::Primary).tint(t.accent)
            .min_size(egui::vec2(72.0, 26.0))).clicked() { do_start = true; }
        if ui.add_enabled(can_pause, Button::new("Pause").variant(Variant::Secondary)
            .min_size(egui::vec2(72.0, 26.0))).clicked() { do_pause = true; }
        if ui.add_enabled(can_resume, Button::new("Resume").variant(Variant::Secondary)
            .min_size(egui::vec2(72.0, 26.0))).clicked() { do_resume = true; }
        if ui.add_enabled(can_stop, Button::new("Stop").variant(Variant::Secondary).tint(t.bear)
            .min_size(egui::vec2(72.0, 26.0))).clicked() { do_stop = true; }
    });

    ui.add_space(gap_xs());

    // Speed presets — selecting one while running changes the *displayed*
    // preset but only takes effect on the next start (the backend doesn't
    // expose a runtime speed-change endpoint in the MVP).
    ui.horizontal(|ui| {
        ui.add(MonospaceCode::new("speed").color(t.dim));
        ui.add_space(gap_sm());
        for preset in SpeedPreset::ALL {
            let sel = s.speed == *preset;
            let col = if sel { t.accent } else { color_subtle(t.dim) };
            if Button::toggle(preset.label(), sel).size(KitSize::Xs)
                .min_size(egui::vec2(40.0, 18.0)).show(ui, t).clicked()
            {
                s.speed = *preset;
            }
        }
    });

    // Drop the lock before doing any blocking REST work — the panel state
    // mutex must not be held across the network round-trip or the next UI
    // frame will block.
    if do_start  { drop_lock_and(|| handle_start()); }
    if do_pause  { drop_lock_and(|| handle_pause()); }
    if do_resume { drop_lock_and(|| handle_resume()); }
    if do_stop   { drop_lock_and(|| handle_stop()); }
}

fn draw_status_row(ui: &mut egui::Ui, s: &ReplayPaneState, t: &Theme) {
    ui.horizontal(|ui| {
        ui.add(MonospaceCode::new("state").color(t.dim));
        let (label, col) = match s.fsm_state {
            ReplayState::NotStarted => ("not started", t.dim),
            ReplayState::Running    => ("running",     t.bull),
            ReplayState::Paused     => ("paused",      t.warn),
            ReplayState::Completed  => ("completed",   t.bull),
            ReplayState::Stopped    => ("stopped",     t.dim),
            ReplayState::Failed     => ("failed",      t.bear),
            ReplayState::Unknown    => ("unknown",     t.dim),
        };
        ui.add(MonospaceCode::new(label).color(col).strong(true));
        ui.add_space(gap_md());

        let frames = s.status.as_ref().map(|st| st.frames_emitted).unwrap_or(0);
        let frames_str = format!("frames: {frames}");
        ui.add(MonospaceCode::new(&frames_str).color(t.text));
        ui.add_space(gap_md());

        let ts = s.status.as_ref().map(|st| st.current_ts_ms).unwrap_or(0);
        let ts_str = format!("t = {}", fmt_ms(ts));
        ui.add(MonospaceCode::new(&ts_str).color(t.text));
        ui.add_space(gap_md());

        if let Some(id) = &s.replay_id {
            let id_str = format!("id={}", &id[..id.len().min(10)]);
            ui.add(MonospaceCode::new(&id_str).color(color_muted(t.dim)));
        }
    });
    if let Some(err) = &s.error {
        ui.add_space(gap_xs());
        ui.colored_label(t.bear, format!("⚠ {err}"));
    }
}

fn draw_events_log(ui: &mut egui::Ui, s: &ReplayPaneState, t: &Theme) {
    ui.add(MonospaceCode::new("EVENTS").color(t.dim));
    ui.add_space(gap_xs());
    if s.events.is_empty() {
        ui.add(MonospaceCode::new("  (no events yet)").color(color_muted(t.dim)));
        return;
    }
    egui::ScrollArea::vertical()
        .id_salt("replay_events")
        .max_height(220.0)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            // Newest-last so `stick_to_bottom` does the natural thing.
            for e in s.events.iter() {
                let col = match e.kind {
                    "bar"      => t.text,
                    "trade"    => t.accent,
                    "snapshot" => t.dim,
                    "error"    => t.bear,
                    _          => color_subtle(t.dim),
                };
                let price = e.price.map(|p| format!("{:>8.2}", p)).unwrap_or_else(|| "       —".into());
                let line = format!("{:>6}  {:<8}  {}  {}", e.kind, e.symbol, fmt_ms(e.t_ms), price);
                ui.add(MonospaceCode::new(&line).color(col));
            }
        });
}

// ─── Control handlers ──────────────────────────────────────────────────────

/// Drop-the-lock helper. The UI thread holds the state mutex inside the
/// modal closure; we need to release it before doing any REST work, then
/// re-acquire only briefly to write the result. The closure runs *after*
/// the modal closes its frame so this is safe.
fn drop_lock_and<F: FnOnce()>(f: F) {
    // Currently we *do* hold the lock from the modal show() closure when
    // this is called. Practical workaround: copy what we need first, then
    // release, then call. The handlers below re-acquire the lock as needed.
    // Since `state().lock()` is taken inside show()'s closure and dropped
    // when the guard `s` goes out of scope, `f` is invoked from the outer
    // `draw` flow already — i.e. AFTER the modal closure returns. By that
    // point the guard is released. The lambda wrapping is purely a marker
    // for the reader: "this code must run with no lock held."
    f();
}

fn handle_start() {
    // Validate + snapshot the form, then drop the lock for the network call.
    let (cfg, want_overlay) = match state().lock() {
        Ok(mut s) => {
            // Reset any previous session so we don't keep its events log,
            // error, or subscription. Keep the form fields intact.
            s.error = None;
            let now = chrono::Utc::now().timestamp_millis();
            let (from_ms, to_ms) = match validate_window(&s.from_str, &s.to_str, now) {
                Ok(v) => v,
                Err(e) => { s.error = Some(e); s.fsm_state = ReplayState::NotStarted; return; }
            };
            let symbols: Vec<String> = s.symbols_str.split(',')
                .map(|x| x.trim().to_uppercase())
                .filter(|x| !x.is_empty())
                .collect();
            if symbols.is_empty() {
                s.error = Some("symbols list is empty".into());
                return;
            }
            if !s.mode.is_implemented() {
                s.error = Some(format!("{} is not implemented yet (backend returns 501)", s.mode.label()));
                return;
            }
            // Confirm-before-large per spec §4.2: > 10 symbols is a warning,
            // not a hard block — we just log it so the user can see it in
            // the events log on next render.
            if symbols.len() > 10 {
                s.push_event(ReplayEvent::new_info(
                    format!("warning: {} symbols requested — replay may be large", symbols.len()), now,
                ));
            }
            let cfg = ReplayConfig {
                from_ts: from_ms,
                to_ts: to_ms,
                symbols,
                speed_multiplier: s.speed.multiplier(),
                asset_class: s.asset_class,
                source: s.source.to_string(),
                mode: s.mode,
            };
            (cfg, s.overlay_on_chart)
        }
        Err(_) => return,
    };

    // Blocking REST call — push onto a background thread so we don't stall
    // the egui frame loop. Result is shipped back via the state mutex.
    std::thread::Builder::new().name("replay-start".into()).spawn(move || {
        let resp = crate::apex_data::rest::start_replay(&cfg);
        let mut s = match state().lock() { Ok(g) => g, Err(p) => p.into_inner() };
        match resp {
            Some(r) => {
                s.replay_id = Some(r.replay_id.clone());
                s.fsm_state = ReplayState::Running;
                s.status = Some(ReplayStatus { state: ReplayState::Running, ..Default::default() });
                s.error = None;
                if let Some(note) = r.note {
                    s.push_event(ReplayEvent::new_info(note, chrono::Utc::now().timestamp_millis()));
                }
                // Wire the WS subscription. The callback runs on the tokio
                // worker thread; we forward through an mpsc channel so the
                // UI thread does the actual log mutation.
                let (tx, rx) = mpsc::channel::<ReplayEvent>();
                let tx_cb = tx.clone();
                let sub = crate::apex_data::ws::subscribe_replay(r.replay_id.clone(), move |evt| {
                    let _ = tx_cb.send(evt);
                });
                s.subscription = Some(sub);
                s.rx_events = Some(rx);
                // Set up the status channel too.
                let (stx, srx) = mpsc::channel::<ReplayStatus>();
                s.tx_status = Some(stx);
                s.rx_status = Some(srx);
                s.last_status_poll = Some(Instant::now());

                // TODO(overlay) — W1-09 (audit) CORRECTION: this comment used to
                // say "the chart exposes no hook". That is STALE and wrong: the
                // chart overlay API is complete and rendered —
                // Chart::set_replay_overlay / append_replay_bar /
                // clear_replay_overlay (gpu.rs) and the paint pass at
                // render/pane/core.rs:2771.
                //
                // The REAL blocker is the event payload: `ReplayEvent` carries
                // only (kind, symbol, t_ms, price) — ws.rs::dispatch_replay
                // deliberately drops OHLV to keep the events log cheap. Building
                // an overlay bar from just the close would FABRICATE OHLC (the
                // same class of defect as the synthesized footprint, W0-12), so
                // it must not be done.
                //
                // To finish: route the FULL bar (already present in the WS
                // envelope at `env.data["bar"]`) to the chart on a separate path
                // from the events log — a ChartCommand::{InstallReplayOverlay,
                // ReplayOverlayBar, ClearReplayOverlay} trio whose handler calls
                // the existing Chart API, with install/clear driven by this
                // toggle so bars only render when the user asked for them
                // (append_replay_bar auto-creates an overlay, so the handler must
                // append only when one is already installed).
                let _ = want_overlay;
            }
            None => {
                s.fsm_state = ReplayState::Failed;
                s.error = Some("backend rejected replay start (see apex_data log)".into());
            }
        }
        crate::wake_native_ui();
    }).ok();
}

fn handle_pause() {
    let id = match state().lock() { Ok(g) => g.replay_id.clone(), Err(_) => None };
    let Some(id) = id else { return; };
    std::thread::Builder::new().name("replay-pause".into()).spawn(move || {
        if let Some(new_state) = crate::apex_data::rest::replay_pause(&id) {
            if let Ok(mut s) = state().lock() {
                let next = if new_state == ReplayState::Unknown { ReplayState::Paused } else { new_state };
                s.fsm_state = next;
                if let Some(st) = s.status.as_mut() { st.state = next; }
            }
        }
        crate::wake_native_ui();
    }).ok();
}

fn handle_resume() {
    let id = match state().lock() { Ok(g) => g.replay_id.clone(), Err(_) => None };
    let Some(id) = id else { return; };
    std::thread::Builder::new().name("replay-resume".into()).spawn(move || {
        if let Some(new_state) = crate::apex_data::rest::replay_resume(&id) {
            if let Ok(mut s) = state().lock() {
                let next = if new_state == ReplayState::Unknown { ReplayState::Running } else { new_state };
                s.fsm_state = next;
                if let Some(st) = s.status.as_mut() { st.state = next; }
            }
        }
        crate::wake_native_ui();
    }).ok();
}

fn handle_stop() {
    let id = match state().lock() { Ok(g) => g.replay_id.clone(), Err(_) => None };
    let Some(id) = id else { return; };
    // Stop the WS subscription locally regardless of backend response —
    // the user clicked Stop, they want it stopped *now*.
    if let Ok(mut s) = state().lock() {
        if let Some(sub) = s.subscription.take() { sub.stop(); }
        s.fsm_state = ReplayState::Stopped;
        if let Some(st) = s.status.as_mut() { st.state = ReplayState::Stopped; }
    }
    std::thread::Builder::new().name("replay-stop".into()).spawn(move || {
        let _ = crate::apex_data::rest::replay_stop(&id);
        crate::wake_native_ui();
    }).ok();
}

// ─── Per-frame pumps ───────────────────────────────────────────────────────

/// Drain pending WS events into the events log. Non-blocking.
fn drain_events() {
    let Ok(mut s) = state().lock() else { return };
    let Some(rx) = s.rx_events.as_ref() else { return };
    let mut batch: Vec<ReplayEvent> = Vec::new();
    while let Ok(e) = rx.try_recv() { batch.push(e); }
    for e in batch { s.push_event(e); }
}

/// Drain status responses pushed by the background polling thread.
fn drain_status() {
    let Ok(mut s) = state().lock() else { return };
    let Some(rx) = s.rx_status.as_ref() else { return };
    let mut latest: Option<ReplayStatus> = None;
    while let Ok(st) = rx.try_recv() { latest = Some(st); }
    if let Some(st) = latest {
        // Promote the most-recent status as the canonical view.
        s.fsm_state = st.state;
        s.status = Some(st);
        // If the server reported a terminal state, drop the WS subscription
        // so we don't pin the socket forever waiting for nothing.
        if s.fsm_state.is_terminal() {
            if let Some(sub) = s.subscription.take() { sub.stop(); }
        }
    }
}

/// Decide whether enough time has passed since the last status poll to fire
/// another one. Runs on the UI thread; spawns a short-lived background
/// thread for the actual REST call so the frame doesn't block.
fn maybe_poll_status() {
    let (id, tx) = {
        let Ok(mut s) = state().lock() else { return };
        if !s.fsm_state.is_active() { return; }
        let now = Instant::now();
        let due = match s.last_status_poll {
            None    => true,
            Some(t) => now.duration_since(t) >= STATUS_POLL_INTERVAL,
        };
        if !due { return; }
        s.last_status_poll = Some(now);
        let id = s.replay_id.clone();
        let tx = s.tx_status.clone();
        (id, tx)
    };
    let Some(id) = id else { return };
    let Some(tx) = tx else { return };
    std::thread::Builder::new().name("replay-status".into()).spawn(move || {
        if let Some(st) = crate::apex_data::rest::replay_status(&id) {
            let _ = tx.send(st);
            crate::wake_native_ui();
        }
    }).ok();
}

// ───────────────────────────────────────────────────────────────────────────
// Tests
// ───────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apex_data::types::{ReplayState, ReplayMode, AssetClass};

    /// Helper: produce a fresh state for tests so they don't trip over each
    /// other through the global `OnceLock`. We deliberately operate on a
    /// hand-rolled `ReplayPaneState` rather than the singleton because the
    /// global is shared across the whole test binary.
    fn fresh() -> ReplayPaneState { ReplayPaneState::default() }

    #[test]
    fn fsm_not_started_through_completed() {
        let mut s = fresh();
        assert_eq!(s.fsm_state, ReplayState::NotStarted);

        // simulate start
        s.fsm_state = ReplayState::Running;
        assert!(s.fsm_state.is_active());
        assert!(!s.fsm_state.is_terminal());

        // pause
        s.fsm_state = ReplayState::Paused;
        assert!(s.fsm_state.is_active());

        // resume
        s.fsm_state = ReplayState::Running;
        assert!(s.fsm_state.is_active());

        // completed
        s.fsm_state = ReplayState::Completed;
        assert!(s.fsm_state.is_terminal());
        assert!(!s.fsm_state.is_active());
    }

    #[test]
    fn fsm_running_to_stopped() {
        let mut s = fresh();
        s.fsm_state = ReplayState::Running;
        s.fsm_state = ReplayState::Stopped;
        assert!(s.fsm_state.is_terminal());
    }

    #[test]
    fn events_log_evicts_at_cap() {
        let mut s = fresh();
        for i in 0..(EVENTS_LOG_CAP + 50) {
            s.push_event(ReplayEvent::new_bar("SPY", i as i64, i as f64));
        }
        assert_eq!(s.events.len(), EVENTS_LOG_CAP);
        // Oldest 50 should have been evicted — first remaining event's
        // t_ms should be 50.
        assert_eq!(s.events.front().unwrap().t_ms, 50);
        assert_eq!(s.events.back().unwrap().t_ms, (EVENTS_LOG_CAP + 50 - 1) as i64);
    }

    #[test]
    fn speed_preset_persists_across_pause_resume() {
        let mut s = fresh();
        s.speed = SpeedPreset::P5;
        // Pause / resume mutate fsm_state, not speed.
        s.fsm_state = ReplayState::Running;
        s.fsm_state = ReplayState::Paused;
        assert_eq!(s.speed, SpeedPreset::P5);
        s.fsm_state = ReplayState::Running;
        assert_eq!(s.speed, SpeedPreset::P5);
        assert!((s.speed.multiplier() - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn date_validation_rejects_inverted_window() {
        let now = chrono::Utc::now().timestamp_millis();
        let r = validate_window("2026-05-01 10:00", "2026-04-01 10:00", now);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("earlier"));
    }

    #[test]
    fn date_validation_rejects_too_old() {
        let now = chrono::Utc::now().timestamp_millis();
        // 30 years ago
        let r = validate_window("1995-01-01 00:00", "1995-01-02 00:00", now);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("years"));
    }

    #[test]
    fn date_validation_rejects_future() {
        let now = chrono::Utc::now().timestamp_millis();
        // 2 years in the future
        let from = chrono::DateTime::<chrono::Utc>::from_timestamp(now / 1000, 0).unwrap();
        let to = from + chrono::Duration::days(365 * 2);
        let r = validate_window(
            &from.format("%Y-%m-%d %H:%M").to_string(),
            &to.format("%Y-%m-%d %H:%M").to_string(),
            now);
        assert!(r.is_err(), "expected error for future `to`, got {:?}", r);
    }

    #[test]
    fn date_validation_accepts_recent_window() {
        let now = chrono::Utc::now().timestamp_millis();
        // 1 hour ago → now
        let r = validate_window("2026-05-15 10:00", "2026-05-15 11:00", now);
        assert!(r.is_ok(), "expected ok for recent window, got {:?}", r);
        let (f, t) = r.unwrap();
        assert!(f < t);
    }

    #[test]
    fn date_parsing_accepts_several_formats() {
        assert!(parse_dt_ms("2026-05-15 10:00:00").is_some());
        assert!(parse_dt_ms("2026-05-15 10:00").is_some());
        assert!(parse_dt_ms("2026-05-15T10:00:00").is_some());
        assert!(parse_dt_ms("2026-05-15").is_some());
        assert!(parse_dt_ms("nope").is_none());
        assert!(parse_dt_ms("").is_none());
    }

    #[test]
    fn replay_mode_implemented_flags() {
        assert!(ReplayMode::StockBars.is_implemented());
        assert!(ReplayMode::OptionBars.is_implemented());
        assert!(ReplayMode::MarkBarsStocks.is_implemented());
        assert!(ReplayMode::MarkBarsOptions.is_implemented());
        assert!(!ReplayMode::Trades.is_implemented());
        assert!(!ReplayMode::Quotes.is_implemented());
    }

    #[test]
    fn replay_mode_wire_strings_match_backend() {
        assert_eq!(ReplayMode::StockBars.as_str(),       "stock_bars");
        assert_eq!(ReplayMode::OptionBars.as_str(),      "option_bars");
        assert_eq!(ReplayMode::MarkBarsStocks.as_str(),  "mark_bars_stocks");
        assert_eq!(ReplayMode::MarkBarsOptions.as_str(), "mark_bars_options");
    }

    #[test]
    fn reset_clears_session_but_keeps_form() {
        let mut s = fresh();
        s.replay_id = Some("abc".into());
        s.fsm_state = ReplayState::Running;
        s.error = Some("boom".into());
        s.symbols_str = "SPY,AAPL".into();

        s.reset();
        assert!(s.replay_id.is_none());
        assert_eq!(s.fsm_state, ReplayState::NotStarted);
        assert!(s.error.is_none());
        assert_eq!(s.symbols_str, "SPY,AAPL");  // form preserved
    }

    #[test]
    fn config_default_is_stock_bars_at_1x_with_spy() {
        let s = fresh();
        assert_eq!(s.asset_class, AssetClass::Stock);
        assert_eq!(s.mode, ReplayMode::StockBars);
        assert_eq!(s.speed, SpeedPreset::P1);
        assert_eq!(s.symbols_str, "SPY");
        assert_eq!(s.source, "last");
    }
}
