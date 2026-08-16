//! Inspect-mode anchor registry — the portable half of Bug Inspect mode.
//!
//! ## Why this lives in `ui_kit`
//!
//! ~10 `ui_kit` widgets (Button, Checkbox, Input, Select, Tabs, MenuItem,
//! NumberStepper, SelectableRow, PanelSection, Modal) register a bug-report
//! anchor for the rect they just painted. Before the P6 inversion those calls
//! read `crate::chart_renderer::bug_anchor::{register, mark, slug, button_key}`
//! — a canonical-layer -> legacy-layer dependency that blocked retiring the
//! chart-app's component systems and blocked extracting `ui_kit` as a crate.
//!
//! The registry itself is pure `egui` + `std` (a thread-local `Vec<AnchorHit>`
//! plus two flags), so it belongs here. It was MOVED, not copied — there is
//! exactly one registry, and `chart_renderer::bug_anchor` now re-exports these
//! items so its existing call sites (and the `bug_anchor!` macro) are untouched.
//!
//! ## What stayed in `chart_renderer::bug_anchor`
//!
//! Everything chart-app / host specific: the inspect overlay painting, the
//! report-draft window, `BUG_REPORTS.md` writing, clipboard-image decoding and
//! the Win32 GDI window-region capture. Those consume this module through
//! [`with_regions`] / [`set_pending`] — the correct direction
//! (`chart_renderer` -> `ui_kit`).

use egui::{Color32, Rect, Stroke, Ui};
use std::cell::{Cell, RefCell};

/// A region the user can anchor a bug report to.
#[derive(Clone)]
pub struct AnchorHit {
    pub key: String,
    pub rect: Rect,
    pub file: &'static str,
    pub line: u32,
}

thread_local! {
    static INSPECT: Cell<bool> = const { Cell::new(false) };
    static FRAME: RefCell<Vec<AnchorHit>> = const { RefCell::new(Vec::new()) };
    static PENDING: RefCell<Option<AnchorHit>> = const { RefCell::new(None) };
}

pub fn set_inspect(on: bool) { INSPECT.with(|c| c.set(on)); }
pub fn toggle_inspect() { INSPECT.with(|c| c.set(!c.get())); }
pub fn inspect() -> bool { INSPECT.with(|c| c.get()) }

// ── UI DEBUG mode (Ctrl+Shift+D) — egui's built-in widget inspector ──────────
//
// egui ships a DevTools-grade overlay (`Style::debug`) that draws every widget's
// allocated rect, the widget that would be hit at the cursor, and expansion
// warnings. Distinct from `INSPECT` above: that one is the *bug-report* anchor
// picker (registered regions only); this one is egui's own view of EVERY widget,
// including ones nothing registered.
thread_local! {
    static UI_DEBUG: Cell<bool> = const { Cell::new(false) };
}

pub fn set_ui_debug(on: bool) { UI_DEBUG.with(|c| c.set(on)); }
pub fn toggle_ui_debug() { UI_DEBUG.with(|c| c.set(!c.get())); }
pub fn ui_debug() -> bool { UI_DEBUG.with(|c| c.get()) }

/// Push the UI-debug flags into egui's style. Call once per frame, early
/// (before widgets are laid out) so the overlay matches this frame's geometry.
///
/// # Debug builds only
///
/// `egui::Style::debug` is itself `#[cfg(debug_assertions)]`, so referencing it
/// unconditionally does not compile in release — **the release build was
/// broken**, and had been: this code is byte-identical at the commit this
/// session's work started from, so nothing here caused it. It surfaced when a
/// `--release` test run was attempted to measure layout-solve cost.
///
/// Worth stating why it went unnoticed: every build in this workflow is a debug
/// or `design-mode` build, so the only configuration that fails is the one
/// nobody compiles day to day — and the one that ships.
///
/// In release this is a no-op: the overlay is a development affordance and the
/// flags it drives do not exist there.
#[cfg(debug_assertions)]
pub fn apply_ui_debug(ctx: &egui::Context) {
    let on = ui_debug();
    // Avoid a style write (which clones the Arc<Style>) on the common path.
    let already = ctx.style().debug.debug_on_hover;
    if on == already && !on { return; }
    ctx.style_mut(|s| {
        s.debug.debug_on_hover = on;
        s.debug.show_widget_hits = on;
        s.debug.show_interactive_widgets = on;
        s.debug.show_expand_width = on;
        s.debug.show_expand_height = on;
        s.debug.show_resize = on;
    });
}

/// Release no-op — see the debug variant.
#[cfg(not(debug_assertions))]
pub fn apply_ui_debug(_ctx: &egui::Context) {}

// ── VIEWPORT SIZING (dev harness) ────────────────────────────────────────────
//
// WHY THIS EXISTS. The dev harness could not drive the app's WIDTH at all. An
// OS-level resize (Win32 `MoveWindow` on the frame) moves the window without
// the app re-laying-out: the clip rect stays at its startup width and every
// widget rect comes back byte-identical. The consequence was that every corpus
// scenario ran at one fixed viewport, so the entire responsive dimension of the
// UI — overflow menus, collapsing toolbars, min-width behaviour — was
// unassertable. Layout bugs at narrow widths could only ever be eyeballed.
//
// Going through egui's own `ViewportCommand` instead makes the resize an event
// the framework processes, so the next frame genuinely lays out at the new size.
// NOT a thread_local: the dev-inspector command may be applied on a different
// thread from the one that owns the winit window, and a thread-local would then
// be silently written and never read — the exact failure mode this whole
// mechanism exists to rule out.
static PENDING_VIEWPORT: std::sync::Mutex<Option<(f32, f32)>> = std::sync::Mutex::new(None);

/// Request a logical viewport (inner) size. Applied on the next frame.
pub fn request_viewport_size(w: f32, h: f32) {
    if let Ok(mut g) = PENDING_VIEWPORT.lock() {
        *g = Some((
            w.clamp(MIN_VIEWPORT.0, MAX_VIEWPORT.0),
            h.clamp(MIN_VIEWPORT.1, MAX_VIEWPORT.1),
        ));
    }
}

// A resize necessarily un-maximizes the window, which would leave every LATER
// corpus scenario running at a different size than the one they were written
// against. So a scenario that resizes must be able to put the window back.
static PENDING_MAXIMIZE: std::sync::Mutex<Option<bool>> = std::sync::Mutex::new(None);

/// Request the window be (un)maximized. Applied on the next frame.
pub fn request_maximized(on: bool) {
    if let Ok(mut g) = PENDING_MAXIMIZE.lock() { *g = Some(on); }
}

/// Take a pending maximize request, if any.
pub fn take_pending_maximize() -> Option<bool> {
    PENDING_MAXIMIZE.lock().ok().and_then(|mut g| g.take())
}

/// Take a pending viewport resize, if any. Called by the winit event loop, which
/// owns the `Window` and is the only place that can actually apply it.
///
/// This does NOT go through `egui::ViewportCommand`: apex-native drives a
/// hand-rolled winit + egui-wgpu loop rather than eframe, so nothing consumes
/// egui's `viewport_output` and a viewport command is accepted then dropped on
/// the floor. Sizes are clamped on the way in, so a zero or absurd request can't
/// wedge the window from a dev command.
pub fn take_pending_viewport() -> Option<(f32, f32)> {
    PENDING_VIEWPORT.lock().ok().and_then(|mut g| g.take())
}

/// Floor for a dev-requested viewport. Below this the shell has no room for
/// even a collapsed toolbar, and the resize is not a useful test.
pub const MIN_VIEWPORT: (f32, f32) = (320.0, 240.0);
/// Ceiling for a dev-requested viewport (generous — 8K-wide spans).
pub const MAX_VIEWPORT: (f32, f32) = (7680.0, 4320.0);

/// Clear the per-frame region list. Call once before rendering the shell.
pub fn begin_frame() { FRAME.with(|f| f.borrow_mut().clear()); }

/// Take (and clear) a pending anchor capture from the last frame.
pub fn take_pending() -> Option<AnchorHit> { PENDING.with(|p| p.borrow_mut().take()) }

/// Record the anchor the user just clicked. Called by the host's inspect
/// overlay (`chart_renderer::bug_anchor::resolve_frame`).
pub fn set_pending(hit: AnchorHit) { PENDING.with(|p| *p.borrow_mut() = Some(hit)); }

/// Read this frame's registered regions. The host's overlay uses this to
/// hit-test the pointer without owning the registry.
pub fn with_regions<R>(f: impl FnOnce(&[AnchorHit]) -> R) -> R {
    FRAME.with(|c| f(&c.borrow()))
}

/// Register an instrumented region. No-op unless inspect mode is on. Draws a
/// faint outline so the user can see what is addressable; the bright highlight
/// for the hovered region is painted by the host's `resolve_frame`.
pub fn anchor(ui: &Ui, key: &str, rect: Rect, file: &'static str, line: u32) {
    if !inspect() || !rect.is_finite() || rect.area() <= 0.0 {
        return;
    }
    FRAME.with(|f| f.borrow_mut().push(AnchorHit { key: key.to_string(), rect, file, line }));
    ui.painter().rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(120, 170, 255, 70)),
        egui::StrokeKind::Inside,
    );
}

/// Register a region without painting an outline (for use where no `Ui` is
/// handy). No-op unless inspect mode is on.
pub fn register(key: &str, rect: Rect, file: &'static str, line: u32) {
    if !inspect() || !rect.is_finite() || rect.area() <= 0.0 {
        return;
    }
    FRAME.with(|f| f.borrow_mut().push(AnchorHit { key: key.to_string(), rect, file, line }));
}

/// Anchor an individual control by wrapping its `Response`. Returns the response
/// unchanged so it threads through call sites. Source location is the call site.
#[track_caller]
pub fn tag(ui: &Ui, key: &str, resp: egui::Response) -> egui::Response {
    if inspect() {
        let loc = std::panic::Location::caller();
        anchor(ui, key, resp.rect, loc.file(), loc.line());
    }
    resp
}

/// Register an anchor for a widget rect, using a call-site `Location` captured by
/// the *caller's* `#[track_caller]`. Lets a widget's `show()` (marked
/// `#[track_caller]`) attribute the anchor to the app code that called it:
/// `inspect::mark(std::panic::Location::caller(), "input", resp.rect)`.
/// No outline (quiet) — relies on the hover highlight. No-op unless inspect is on.
pub fn mark(loc: &'static std::panic::Location<'static>, key: &str, rect: Rect) {
    register(key, rect, loc.file(), loc.line());
}

/// Slugify arbitrary label text into an anchor-key fragment.
pub fn slug(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = false;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') { out.pop(); }
    if out.is_empty() { out.push_str("unnamed"); }
    out
}

/// Build a `button/<slug>` key from a button's label (falling back to its icon
/// glyph for icon-only buttons).
pub fn button_key(label: &str, icon: Option<&str>) -> String {
    let basis = if label.trim().is_empty() { icon.unwrap_or("") } else { label };
    format!("button/{}", slug(basis))
}

/// Trim a source path to its `src/...` tail.
pub fn short(f: &str) -> &str { short_file(f) }

pub(crate) fn short_file(f: &str) -> &str {
    if let Some(idx) = f.rfind("src/").or_else(|| f.rfind("src\\")) {
        &f[idx..]
    } else {
        f
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_normalises_labels() {
        assert_eq!(slug("Save As…"), "save-as");
        assert_eq!(slug("  "), "unnamed");
        assert_eq!(slug("A/B  Test"), "a-b-test");
    }

    #[test]
    fn button_key_falls_back_to_icon() {
        assert_eq!(button_key("Close", None), "button/close");
        assert_eq!(button_key("  ", Some("\u{e4f6}")), "button/unnamed");
        assert_eq!(button_key("", Some("Plus")), "button/plus");
    }

    #[test]
    fn short_trims_to_src_tail() {
        assert_eq!(short("C:\\repo\\src-tauri\\src/ui_kit/x.rs"), "src/ui_kit/x.rs");
        assert_eq!(short("no-src-here.rs"), "no-src-here.rs");
    }

    /// `register` is a no-op while inspect mode is off — the registry must stay
    /// empty so the per-frame Vec never grows in normal operation.
    #[test]
    fn register_is_noop_when_inspect_off() {
        set_inspect(false);
        begin_frame();
        register("x/y", Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(4.0, 4.0)), "f.rs", 1);
        assert_eq!(with_regions(|r| r.len()), 0);
    }

    /// With inspect on, `register` records the region and `begin_frame` clears it.
    #[test]
    fn register_records_and_begin_frame_clears() {
        set_inspect(true);
        begin_frame();
        register("x/y", Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(4.0, 4.0)), "f.rs", 7);
        assert_eq!(with_regions(|r| r.len()), 1);
        assert_eq!(with_regions(|r| r[0].key.clone()), "x/y");
        begin_frame();
        assert_eq!(with_regions(|r| r.len()), 0);
        set_inspect(false);
    }

    /// Degenerate rects are rejected (they would be unclickable anyway).
    #[test]
    fn register_rejects_degenerate_rects() {
        set_inspect(true);
        begin_frame();
        register("zero", Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(0.0, 0.0)), "f.rs", 1);
        register("nan", Rect::from_min_size(egui::pos2(f32::NAN, 0.0), egui::vec2(4.0, 4.0)), "f.rs", 2);
        assert_eq!(with_regions(|r| r.len()), 0);
        set_inspect(false);
    }
}
