//! Bug-report Inspect mode (Ctrl+Shift+I) + anchored bug reports with images.
//!
//! A port of supermodel's UX Inspect mode, extended for apex-terminal: point at
//! any *instrumented* UI region in the running app, see a stable anchor key plus
//! the source location it maps to, then file a rich bug report — free-text
//! description, a checklist, and attached images (auto region screenshot,
//! clipboard paste, or a file off disk). Reports append to `BUG_REPORTS.md` (repo
//! root) with images saved under `BUG_REPORTS/`, so a follow-up agent given that
//! block knows exactly where to work and what the user saw.
//!
//! Everything here is pure egui overlay + plain file/Win32-GDI I/O. It never
//! touches the GPU chart pipeline or the continuous-redraw model.
//!
//! Flow per frame:
//!   1. `begin_frame()` clears the per-frame region registry (top of draw_chart).
//!   2. UI code calls `anchor!(ui, "key", rect)` / `register(...)` for regions it
//!      wants addressable. The macro captures `file!()/line!()` at the call site.
//!   3. After all UI, `resolve_frame(ctx, draft_open)` highlights the region under
//!      the pointer, shows its key + source, and on click records a PENDING hit.
//!   4. `take_pending()` → `open_draft(hit)` → `prompt(ctx)` renders the report
//!      window. On save it writes the markdown block and (optionally) queues a
//!      region screenshot fulfilled in `gpu.rs` (which has the HWND).

use egui::{Align2, Color32, FontId, Rect, Sense, Stroke, Ui};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};

// ── Palette (matches the supermodel inspect overlay) ────────────────────────
const ACCENT: Color32 = Color32::from_rgb(120, 170, 255);
const PANEL_BG: Color32 = Color32::from_rgb(18, 26, 44);
const DIM: Color32 = Color32::from_rgb(150, 162, 186);

/// A region the user can anchor a bug report to.
#[derive(Clone)]
pub struct AnchorHit {
    pub key: String,
    pub rect: Rect,
    pub file: &'static str,
    pub line: u32,
}

/// A region screenshot to capture in `gpu.rs` (which owns the window HWND).
pub struct CaptureReq {
    pub rect: Rect,
    pub out: PathBuf,
}

/// A bug report being composed in the prompt window.
struct ReportDraft {
    hit: AnchorHit,
    bug_id: u32,
    text: String,
    list: Vec<String>,
    list_input: String,
    /// Saved image files (absolute or cwd-relative), already on disk.
    images: Vec<PathBuf>,
    /// Reference the clicked region's clean screenshot (captured at click time,
    /// before this dialog opened) in the saved report.
    auto_shot: bool,
    /// Path to the region screenshot captured at click time, if any.
    region_path: Option<PathBuf>,
    /// Transient status line shown under the buttons.
    status: String,
    /// Skip the dialog render for exactly one frame so the click-time region
    /// capture lands on a clean frame (no dialog, no inspect overlay).
    skip_render_once: bool,
    /// Auto-focus the text field on the first real render frame.
    focus_next: bool,
}

thread_local! {
    static INSPECT: Cell<bool> = const { Cell::new(false) };
    static FRAME: RefCell<Vec<AnchorHit>> = const { RefCell::new(Vec::new()) };
    static PENDING: RefCell<Option<AnchorHit>> = const { RefCell::new(None) };
    static DRAFT: RefCell<Option<ReportDraft>> = const { RefCell::new(None) };
    static CAPTURE: RefCell<Vec<CaptureReq>> = const { RefCell::new(Vec::new()) };
    /// Region screenshot staged at click time: (bug_id, png path).
    static STAGED_REGION: RefCell<Option<(u32, PathBuf)>> = const { RefCell::new(None) };
}

pub fn set_inspect(on: bool) { INSPECT.with(|c| c.set(on)); }
pub fn toggle_inspect() { INSPECT.with(|c| c.set(!c.get())); }
pub fn inspect() -> bool { INSPECT.with(|c| c.get()) }

/// Clear the per-frame region list. Call once before rendering the shell.
pub fn begin_frame() { FRAME.with(|f| f.borrow_mut().clear()); }

/// Take (and clear) a pending anchor capture from the last frame.
pub fn take_pending() -> Option<AnchorHit> { PENDING.with(|p| p.borrow_mut().take()) }

pub fn draft_is_open() -> bool { DRAFT.with(|d| d.borrow().is_some()) }

/// Discard the open draft (if any), deleting its staged images + region shot.
pub fn close_draft() {
    if let Some(draft) = DRAFT.with(|d| d.borrow_mut().take()) {
        for p in &draft.images { let _ = std::fs::remove_file(p); }
        if let Some(p) = &draft.region_path { let _ = std::fs::remove_file(p); }
    }
}

/// Drain queued region-screenshot requests (fulfilled by `gpu.rs`).
pub fn take_capture_reqs() -> Vec<CaptureReq> { CAPTURE.with(|c| std::mem::take(&mut *c.borrow_mut())) }

/// Register an instrumented region. No-op unless inspect mode is on. Draws a
/// faint outline so the user can see what is addressable; the bright highlight
/// for the hovered region is painted in [`resolve_frame`].
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

/// `anchor!(ui, "area/section/control", rect)` — captures source location.
#[macro_export]
macro_rules! bug_anchor {
    ($ui:expr, $key:expr, $rect:expr) => {
        $crate::chart_renderer::bug_anchor::anchor($ui, $key, $rect, file!(), line!());
    };
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

/// Public accessor for trimming a source path to its `src/...` tail.
pub fn short(f: &str) -> &str { short_file(f) }

fn short_file(f: &str) -> &str {
    if let Some(idx) = f.rfind("src/").or_else(|| f.rfind("src\\")) {
        &f[idx..]
    } else {
        f
    }
}

// ── Inspect overlay ─────────────────────────────────────────────────────────

/// After all UI is drawn, highlight the most-specific anchored region under the
/// pointer, label it with its key + source, and capture a click as a PENDING
/// hit. A full-window catch layer eats the click so the underlying widget does
/// not also fire. Skipped when `note_open` (so the report prompt stays
/// interactive).
pub fn resolve_frame(ctx: &egui::Context, note_open: bool) {
    if !inspect() {
        return;
    }

    let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("bug_inspect_overlay"));
    let painter = ctx.layer_painter(layer);
    let screen = ctx.screen_rect();

    // Banner — a filled pill placed below the toolbar so it is not occluded.
    let paint_banner = || {
        let banner = "● BUG INSPECT — click a region to report it · hold Alt for the containing region · Ctrl+Shift+I to exit";
        let font = FontId::proportional(12.5);
        let center = egui::pos2(screen.center().x, screen.top() + 52.0);
        let galley = painter.layout_no_wrap(banner.to_string(), font, Color32::WHITE);
        let pill = Rect::from_center_size(center, galley.size() + egui::vec2(24.0, 12.0));
        painter.rect_filled(pill, 14.0, Color32::from_rgb(40, 70, 130));
        painter.rect_stroke(pill, 14.0, Stroke::new(1.0, Color32::from_rgb(150, 190, 255)), egui::StrokeKind::Inside);
        painter.galley(pill.center() - galley.size() / 2.0, galley, Color32::WHITE);
    };

    if note_open {
        paint_banner();
        return; // let the report window take input
    }

    // Resolve the region under the pointer (default = smallest; Alt = parent).
    let hit = ctx.pointer_hover_pos().and_then(|p| {
        let want_parent = ctx.input(|i| i.modifiers.alt);
        let by_area = FRAME.with(|f| {
            let mut containing: Vec<AnchorHit> =
                f.borrow().iter().filter(|a| a.rect.contains(p)).cloned().collect();
            containing.sort_by(|a, b| a.rect.area().partial_cmp(&b.rect.area()).unwrap_or(std::cmp::Ordering::Equal));
            if want_parent { containing.into_iter().nth(1) } else { containing.into_iter().next() }
        });
        by_area.or_else(|| FRAME.with(|f| {
            f.borrow().iter().filter(|a| a.rect.contains(p))
                .min_by(|a, b| a.rect.area().partial_cmp(&b.rect.area()).unwrap_or(std::cmp::Ordering::Equal))
                .cloned()
        }))
    });

    // Full-window catch layer so the click captures the anchor instead of
    // activating the widget underneath. It paints nothing, so it never pollutes
    // a capture frame.
    let _catch = egui::Area::new(egui::Id::new("bug_inspect_catch"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .interactable(true)
        .show(ctx, |ui| ui.allocate_rect(screen, Sense::click()))
        .inner;
    ctx.set_cursor_icon(egui::CursorIcon::Crosshair);

    // On click: stage a CLEAN region screenshot (this frame paints no banner,
    // no highlight, and the dialog is held back one frame — see prompt's
    // skip_render_once), then record the pending hit and return early so the
    // capture lands on an un-occluded frame.
    if ctx.input(|i| i.pointer.primary_clicked()) {
        if let Some(hit) = hit {
            stage_region_capture(&hit);
            PENDING.with(|pp| *pp.borrow_mut() = Some(hit));
        }
        return;
    }

    // No click this frame → paint the banner + the hovered region highlight.
    paint_banner();
    let Some(hit) = hit else { return };

    painter.rect_filled(hit.rect, 2.0, Color32::from_rgba_unmultiplied(120, 170, 255, 30));
    painter.rect_stroke(hit.rect, 2.0, Stroke::new(2.0, ACCENT), egui::StrokeKind::Inside);

    // Label: key + source, placed above the region (or below if near the top).
    let label = format!("{}\n{}:{}", hit.key, short_file(hit.file), hit.line);
    let galley = painter.layout(label, FontId::monospace(11.0), Color32::WHITE, 480.0);
    let pad = egui::vec2(8.0, 5.0);
    let box_size = galley.size() + pad * 2.0;
    let above = hit.rect.top() - box_size.y - 4.0 > screen.top() + 18.0;
    let origin = if above {
        egui::pos2(hit.rect.left(), hit.rect.top() - box_size.y - 4.0)
    } else {
        egui::pos2(hit.rect.left(), hit.rect.bottom() + 4.0)
    };
    let bg = Rect::from_min_size(origin, box_size);
    painter.rect_filled(bg, 3.0, PANEL_BG);
    painter.rect_stroke(bg, 3.0, Stroke::new(1.0, ACCENT), egui::StrokeKind::Inside);
    painter.galley(origin + pad, galley, Color32::WHITE);
}

/// Queue a clean region screenshot at click time + stash its path for the draft.
fn stage_region_capture(hit: &AnchorHit) {
    let bug_id = next_bug_id();
    let path = images_dir().join(format!("bug-{:04}-region.png", bug_id));
    let _ = std::fs::create_dir_all(images_dir());
    CAPTURE.with(|c| c.borrow_mut().push(CaptureReq { rect: hit.rect, out: path.clone() }));
    STAGED_REGION.with(|s| *s.borrow_mut() = Some((bug_id, path)));
}

// ── Report draft ────────────────────────────────────────────────────────────

/// Open the report prompt for a freshly clicked anchor. Consumes the region
/// screenshot staged at click time (captured on a clean frame).
pub fn open_draft(hit: AnchorHit) {
    let (bug_id, region_path) = match STAGED_REGION.with(|s| s.borrow_mut().take()) {
        Some((id, path)) => (id, Some(path)),
        None => (next_bug_id(), None),
    };
    DRAFT.with(|d| {
        *d.borrow_mut() = Some(ReportDraft {
            hit,
            bug_id,
            text: String::new(),
            list: Vec::new(),
            list_input: String::new(),
            images: Vec::new(),
            auto_shot: region_path.is_some(),
            region_path,
            status: String::new(),
            skip_render_once: true,
            focus_next: false,
        });
    });
}

/// Render the "Report a bug" window. No-op when no draft is open.
pub fn prompt(ctx: &egui::Context) {
    // Take the draft out so we don't hold a RefCell borrow across the egui
    // closure; put it back unless it was saved/cancelled.
    let Some(mut draft) = DRAFT.with(|d| d.borrow_mut().take()) else { return };

    // Hold the dialog back for exactly one frame after the click so the
    // click-time region screenshot lands on a clean frame (no dialog visible).
    if draft.skip_render_once {
        draft.skip_render_once = false;
        draft.focus_next = true;
        DRAFT.with(|d| *d.borrow_mut() = Some(draft));
        return;
    }

    let key = draft.hit.key.clone();
    let src = format!("{}:{}", short_file(draft.hit.file), draft.hit.line);
    let mut save = false;
    let mut close = false;

    // Escape cancels.
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close = true;
    }

    egui::Window::new("Report a bug")
        .order(egui::Order::Tooltip) // above the inspect catch layer
        .collapsible(false)
        .resizable(true)
        .anchor(Align2::CENTER_CENTER, [0.0, -20.0])
        .default_width(480.0)
        .frame(egui::Frame::popup(&ctx.style()).fill(PANEL_BG).stroke(Stroke::new(1.0, ACCENT)))
        .show(ctx, |ui| {
            ui.set_width(480.0);
            ui.label(egui::RichText::new(format!("@anchor {key}")).strong().monospace().color(ACCENT));
            ui.label(egui::RichText::new(&src).monospace().size(10.5).color(DIM));
            ui.add_space(8.0);

            // ── Description ──────────────────────────────────────────────
            ui.label(egui::RichText::new("What's wrong / what should it do?").size(11.5).color(DIM));
            ui.add_space(3.0);
            let resp = ui.add(
                egui::TextEdit::multiline(&mut draft.text)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY)
                    .hint_text("e.g. % column shows wrong value when market is closed"),
            );
            if draft.focus_next {
                resp.request_focus();
                draft.focus_next = false;
            }
            if ui.input(|i| (i.modifiers.command || i.modifiers.ctrl) && i.key_pressed(egui::Key::Enter)) {
                save = true;
            }

            // ── Checklist ────────────────────────────────────────────────
            ui.add_space(10.0);
            ui.label(egui::RichText::new("Checklist (optional)").size(11.5).color(DIM));
            ui.add_space(3.0);
            let mut remove_item: Option<usize> = None;
            for (i, item) in draft.list.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("☐").color(DIM));
                    ui.label(egui::RichText::new(item).size(12.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("✕").clicked() { remove_item = Some(i); }
                    });
                });
            }
            if let Some(i) = remove_item { draft.list.remove(i); }
            ui.horizontal(|ui| {
                let add_resp = ui.add(
                    egui::TextEdit::singleline(&mut draft.list_input)
                        .desired_width(360.0)
                        .hint_text("add a checklist item, press Enter"),
                );
                let enter = add_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (ui.button("Add").clicked() || enter) && !draft.list_input.trim().is_empty() {
                    draft.list.push(draft.list_input.trim().to_string());
                    draft.list_input.clear();
                    add_resp.request_focus();
                }
            });

            // ── Images ───────────────────────────────────────────────────
            ui.add_space(10.0);
            ui.label(egui::RichText::new("Images").size(11.5).color(DIM));
            ui.add_space(3.0);
            let chip = |ui: &mut egui::Ui, text: String| -> bool {
                let mut removed = false;
                egui::Frame::none()
                    .fill(Color32::from_rgb(28, 38, 60))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(60, 80, 120)))
                    .inner_margin(egui::Margin::symmetric(6, 3))
                    .corner_radius(egui::CornerRadius::same(3))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(text).size(10.5).color(Color32::WHITE));
                        if ui.small_button("✕").clicked() { removed = true; }
                    });
                removed
            };
            let mut remove_img: Option<usize> = None;
            ui.horizontal_wrapped(|ui| {
                // Region screenshot (captured at click time) shown as a togglable chip.
                if draft.region_path.is_some() {
                    let label = if draft.auto_shot { "🖼 region screenshot ✓" } else { "🖼 region screenshot (off)" };
                    if chip(ui, label.to_string()) { draft.auto_shot = false; }
                }
                for (i, p) in draft.images.iter().enumerate() {
                    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    if chip(ui, format!("🖼 {name}")) { remove_img = Some(i); }
                }
            });
            if let Some(i) = remove_img {
                let p = draft.images.remove(i);
                let _ = std::fs::remove_file(p); // best-effort: drop the staged file
            }

            ui.add_space(4.0);
            if draft.region_path.is_some() {
                ui.checkbox(&mut draft.auto_shot, "Include the clicked-region screenshot");
            }

            ui.horizontal(|ui| {
                if ui.button("📎 Attach image…").clicked() {
                    match attach_file(draft.bug_id, draft.images.len()) {
                        Ok(Some(p)) => draft.images.push(p),
                        Ok(None) => {}
                        Err(e) => draft.status = format!("Attach failed: {e}"),
                    }
                }
                if ui.button("📋 Paste from clipboard").clicked() {
                    match paste_clipboard_image(draft.bug_id, draft.images.len()) {
                        Ok(Some(p)) => draft.images.push(p),
                        Ok(None) => draft.status = "No image on the clipboard".to_string(),
                        Err(e) => draft.status = format!("Paste failed: {e}"),
                    }
                }
            });
            // Ctrl+V anywhere in the window also pastes an image.
            if ui.input(|i| (i.modifiers.command || i.modifiers.ctrl) && i.key_pressed(egui::Key::V)) {
                match paste_clipboard_image(draft.bug_id, draft.images.len()) {
                    Ok(Some(p)) => draft.images.push(p),
                    Ok(None) => {}
                    Err(e) => draft.status = format!("Paste failed: {e}"),
                }
            }

            // ── Footer ───────────────────────────────────────────────────
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(egui::RichText::new("Log bug").color(Color32::WHITE).size(12.0)).fill(ACCENT)).clicked() {
                    save = true;
                }
                if ui.button(egui::RichText::new("Cancel").size(12.0)).clicked() {
                    close = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("Ctrl+Enter to save · Esc to cancel").size(10.0).color(DIM));
                });
            });
            if !draft.status.is_empty() {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(&draft.status).size(10.5).color(Color32::from_rgb(255, 180, 120)));
            }
        });

    if save {
        // The region screenshot was already captured (cleanly) at click time.
        // Reference it if still wanted; otherwise delete the staged file.
        let region_ref = match (&draft.region_path, draft.auto_shot) {
            (Some(p), true) => p.file_name()
                .map(|n| format!("{}/{}", images_dir_name(), n.to_string_lossy())),
            (Some(p), false) => { let _ = std::fs::remove_file(p); None }
            (None, _) => None,
        };
        match write_report(&draft, region_ref.as_deref()) {
            Ok(()) => eprintln!("[bug-anchor] Logged @anchor {} → {}", draft.hit.key, reports_path().display()),
            Err(e) => eprintln!("[bug-anchor] Failed to write {}: {e}", reports_path().display()),
        }
        // Saved → don't put the draft back (window closes).
    } else if close {
        // Cancelled → drop staged images + the region screenshot.
        for p in &draft.images { let _ = std::fs::remove_file(p); }
        if let Some(p) = &draft.region_path { let _ = std::fs::remove_file(p); }
        // Draft dropped (window closes).
    } else {
        // Still composing → put it back.
        DRAFT.with(|d| *d.borrow_mut() = Some(draft));
    }
}

// ── Markdown output ─────────────────────────────────────────────────────────

fn reports_path() -> PathBuf { PathBuf::from("BUG_REPORTS.md") }
fn images_dir_name() -> &'static str { "BUG_REPORTS" }
fn images_dir() -> PathBuf { PathBuf::from(images_dir_name()) }

/// Next sequential bug id = count of existing `## ` headers + 1.
fn next_bug_id() -> u32 {
    let content = std::fs::read_to_string(reports_path()).unwrap_or_default();
    let count = content.lines().filter(|l| l.starts_with("## ")).count();
    count as u32 + 1
}

fn now_stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    chrono::DateTime::from_timestamp(secs as i64, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| format!("epoch+{secs}s"))
}

/// Append an anchored bug report to `BUG_REPORTS.md`, creating it with a header
/// if missing.
fn write_report(draft: &ReportDraft, region_ref: Option<&str>) -> std::io::Result<()> {
    use std::io::Write;
    let path = reports_path();
    let new = !path.exists();
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
    if new {
        writeln!(
            f,
            "# Bug Reports\n\nAnchored bug reports captured from the running app (Ctrl+Shift+I Bug Inspect mode).\nEach report header: `## [ ] @anchor <key>  (<file>:<line>)`. Check the box when fixed.\n"
        )?;
    }
    writeln!(
        f,
        "## [ ] @anchor {}  ({}:{})",
        draft.hit.key,
        short_file(draft.hit.file),
        draft.hit.line
    )?;
    writeln!(f, "_{}_  ·  bug-{:04}\n", now_stamp(), draft.bug_id)?;

    let text = draft.text.trim();
    if !text.is_empty() {
        writeln!(f, "{text}\n")?;
    }
    if !draft.list.is_empty() {
        for item in &draft.list {
            writeln!(f, "- [ ] {item}")?;
        }
        writeln!(f)?;
    }
    if let Some(r) = region_ref {
        writeln!(f, "![region]({r})")?;
    }
    for p in &draft.images {
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        writeln!(f, "![{name}]({}/{name})", images_dir_name())?;
    }
    writeln!(f, "\n---\n")?;
    Ok(())
}

// ── Image attachment: file picker ───────────────────────────────────────────

/// Open a file dialog and copy the chosen image into `BUG_REPORTS/`. Returns the
/// staged path, or `None` if the user cancelled.
fn attach_file(bug_id: u32, idx: usize) -> std::io::Result<Option<PathBuf>> {
    let Some(src) = rfd::FileDialog::new()
        .add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp", "bmp"])
        .set_title("Attach an image to the bug report")
        .pick_file()
    else {
        return Ok(None);
    };
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("png").to_lowercase();
    std::fs::create_dir_all(images_dir())?;
    let dest = images_dir().join(format!("bug-{:04}-img{}.{}", bug_id, idx + 1, ext));
    std::fs::copy(&src, &dest)?;
    Ok(Some(dest))
}

// ── Image attachment: clipboard paste (Windows DIB) ─────────────────────────

#[cfg(target_os = "windows")]
fn paste_clipboard_image(bug_id: u32, idx: usize) -> std::io::Result<Option<PathBuf>> {
    use std::io::{Error, ErrorKind};
    use windows_sys::Win32::System::DataExchange::{CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard};
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
    const CF_DIB: u32 = 8;

    unsafe {
        if IsClipboardFormatAvailable(CF_DIB) == 0 {
            return Ok(None);
        }
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return Err(Error::new(ErrorKind::Other, "OpenClipboard failed"));
        }
        // Ensure the clipboard is closed on every path.
        let result = (|| {
            let h = GetClipboardData(CF_DIB);
            if h.is_null() {
                return Ok(None);
            }
            let size = GlobalSize(h) as usize;
            let ptr = GlobalLock(h) as *const u8;
            if ptr.is_null() || size < 40 {
                return Err(Error::new(ErrorKind::Other, "clipboard DIB lock failed"));
            }
            let bytes = std::slice::from_raw_parts(ptr, size).to_vec();
            GlobalUnlock(h);
            let img = dib_to_rgba(&bytes)
                .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
            std::fs::create_dir_all(images_dir())?;
            let dest = images_dir().join(format!("bug-{:04}-img{}.png", bug_id, idx + 1));
            img.save(&dest).map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;
            Ok(Some(dest))
        })();
        CloseClipboard();
        result
    }
}

#[cfg(not(target_os = "windows"))]
fn paste_clipboard_image(_bug_id: u32, _idx: usize) -> std::io::Result<Option<PathBuf>> {
    Ok(None)
}

/// Parse a packed DIB (BITMAPINFOHEADER + optional masks/palette + pixels) into
/// an RGBA image. Handles 24- and 32-bit BI_RGB / BI_BITFIELDS — the formats
/// Win+Shift+S and most apps put on the clipboard.
#[cfg(target_os = "windows")]
fn dib_to_rgba(b: &[u8]) -> Result<image::RgbaImage, String> {
    let rd_u32 = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
    let rd_i32 = |o: usize| i32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
    let rd_u16 = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);

    let header_size = rd_u32(0) as usize;
    let width = rd_i32(4);
    let height_raw = rd_i32(8);
    let bit_count = rd_u16(14) as usize;
    let compression = rd_u32(16);
    let clr_used = rd_u32(32) as usize;

    if width <= 0 || height_raw == 0 {
        return Err("invalid DIB dimensions".into());
    }
    if bit_count != 24 && bit_count != 32 {
        return Err(format!("unsupported DIB bit depth: {bit_count}"));
    }
    // BI_RGB = 0, BI_BITFIELDS = 3 (treated as straight BGRA for 32-bit).
    if compression != 0 && compression != 3 {
        return Err(format!("unsupported DIB compression: {compression}"));
    }

    let w = width as usize;
    let top_down = height_raw < 0;
    let h = height_raw.unsigned_abs() as usize;

    // Pixel data offset: header + colour masks (BI_BITFIELDS) + palette.
    let masks = if compression == 3 { 12 } else { 0 };
    let palette = clr_used * 4;
    let data_off = header_size + masks + palette;
    let bytes_pp = bit_count / 8;
    let row_stride = ((w * bytes_pp + 3) / 4) * 4; // DWORD-aligned rows
    if data_off + row_stride * h > b.len() {
        return Err("DIB pixel data truncated".into());
    }

    let mut img = image::RgbaImage::new(w as u32, h as u32);
    for y in 0..h {
        // Bottom-up DIBs store the last row first.
        let src_row = if top_down { y } else { h - 1 - y };
        let row = data_off + src_row * row_stride;
        for x in 0..w {
            let px = row + x * bytes_pp;
            let bch = b[px];
            let g = b[px + 1];
            let r = b[px + 2];
            let a = if bytes_pp == 4 { b[px + 3] } else { 255 };
            // 32-bit clipboard DIBs often carry a zero alpha — treat as opaque.
            let a = if bytes_pp == 4 && a == 0 { 255 } else { a };
            img.put_pixel(x as u32, y as u32, image::Rgba([r, g, bch, a]));
        }
    }
    Ok(img)
}

// ── Region screenshot: native window capture (called from gpu.rs) ───────────

/// Capture the on-screen pixels of `rect` (egui points) from the window's client
/// area and save a PNG to `out`. Reads the composited desktop via GDI BitBlt —
/// it captures exactly what the user sees (incl. the GPU chart) and never touches
/// the render pipeline. `scale` is the window's pixels-per-point.
#[cfg(target_os = "windows")]
pub fn capture_window_region(
    hwnd: *mut std::ffi::c_void,
    scale: f32,
    rect: Rect,
    out: &Path,
) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind};
    use windows_sys::Win32::Foundation::{POINT, RECT};
    use windows_sys::Win32::Graphics::Gdi::{
        BitBlt, ClientToScreen, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject,
        GetDC, GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        DIB_RGB_COLORS, SRCCOPY,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect;

    unsafe {
        let mut cr: RECT = std::mem::zeroed();
        if GetClientRect(hwnd, &mut cr) == 0 {
            return Err(Error::new(ErrorKind::Other, "GetClientRect failed"));
        }
        let cw = (cr.right - cr.left).max(0);
        let chh = (cr.bottom - cr.top).max(0);
        if cw == 0 || chh == 0 {
            return Err(Error::new(ErrorKind::Other, "empty client rect"));
        }

        // Region in client pixels, clamped to the client area.
        let rx = ((rect.left() * scale).round() as i32).clamp(0, cw);
        let ry = ((rect.top() * scale).round() as i32).clamp(0, chh);
        let rw = ((rect.width() * scale).round() as i32).clamp(1, cw - rx);
        let rh = ((rect.height() * scale).round() as i32).clamp(1, chh - ry);

        // Client origin in screen coordinates.
        let mut origin = POINT { x: rx, y: ry };
        ClientToScreen(hwnd, &mut origin);

        let screen_dc = GetDC(std::ptr::null_mut());
        if screen_dc.is_null() {
            return Err(Error::new(ErrorKind::Other, "GetDC(screen) failed"));
        }
        let mem_dc = CreateCompatibleDC(screen_dc);
        let hbm = CreateCompatibleBitmap(screen_dc, rw, rh);
        let old = SelectObject(mem_dc, hbm as _);

        let blt_ok = BitBlt(mem_dc, 0, 0, rw, rh, screen_dc, origin.x, origin.y, SRCCOPY) != 0;

        // Read back as top-down 32-bit BGRA.
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = rw;
        bmi.bmiHeader.biHeight = -rh; // negative = top-down
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB as u32;

        let mut buf = vec![0u8; (rw * rh * 4) as usize];
        let scan = GetDIBits(
            mem_dc,
            hbm,
            0,
            rh as u32,
            buf.as_mut_ptr() as *mut _,
            &mut bmi,
            DIB_RGB_COLORS,
        );

        SelectObject(mem_dc, old);
        DeleteObject(hbm as _);
        DeleteDC(mem_dc);
        ReleaseDC(std::ptr::null_mut(), screen_dc);

        if !blt_ok || scan == 0 {
            return Err(Error::new(ErrorKind::Other, "BitBlt/GetDIBits failed"));
        }

        // BGRA → RGBA, force opaque alpha (GDI leaves it zero).
        let mut img = image::RgbaImage::new(rw as u32, rh as u32);
        for y in 0..rh as u32 {
            for x in 0..rw as u32 {
                let i = ((y * rw as u32 + x) * 4) as usize;
                img.put_pixel(x, y, image::Rgba([buf[i + 2], buf[i + 1], buf[i], 255]));
            }
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        img.save(out).map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
pub fn capture_window_region(
    _hwnd: *mut std::ffi::c_void,
    _scale: f32,
    _rect: Rect,
    _out: &Path,
) -> std::io::Result<()> {
    Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "region capture is Windows-only"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_file_trims_to_src() {
        assert_eq!(short_file("C:\\x\\apex\\src\\chart\\core.rs"), "src\\chart\\core.rs");
        assert_eq!(short_file("/home/u/apex/src/chart/core.rs"), "src/chart/core.rs");
        assert_eq!(short_file("weird"), "weird");
    }

    #[test]
    fn slug_normalizes() {
        assert_eq!(slug("Options Chain"), "options-chain");
        assert_eq!(slug("  GEX!! levels  "), "gex-levels");
        assert_eq!(slug("///"), "unnamed");
    }

    #[test]
    fn pending_round_trips() {
        set_inspect(true);
        assert!(inspect());
        PENDING.with(|p| *p.borrow_mut() = Some(AnchorHit {
            key: "a/b".into(),
            rect: Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(10.0, 10.0)),
            file: "src/x.rs",
            line: 1,
        }));
        assert!(take_pending().is_some());
        assert!(take_pending().is_none());
        set_inspect(false);
    }
}
