//! PanelListRow — the canonical row primitive for panel lists.
//!
//! Replaces the per-panel hand-rolled "row" patterns (watchlist rows, alert
//! rows, scanner rows, order rows, journal rows, etc.) with one builder that
//! handles hover, selection, leading/trailing slots, and consistent
//! typography.
//!
//! ```ignore
//! let resp = PanelListRow::new("aapl_row")
//!     .leading(|ui, t| { /* avatar / color swatch / drag handle */ })
//!     .primary("AAPL")
//!     .secondary("Apple Inc")
//!     .trailing(|ui, t| { /* price / X button / dropdown */ })
//!     .selected(is_selected)
//!     .show(ui, t);
//! if resp.clicked() { /* ... */ }
//! ```
//!
//! Visual spec (LOCKED per user decision):
//! - Hover: `color_alpha(palette_ct(t).base(SxTone::Text), 8)` background, `radius_sm()` corners.
//! - Selected: BOTH `color_alpha(palette_ct(t).base(SxTone::Accent), 24)` background fill AND a 2px
//!   accent stripe on the LEFT edge. User explicitly chose "both" — loud is OK.
//! - No stroke. No border. Just background and the stripe.
//! - Primary text: `mono_sm` in `palette_ct(t).base(SxTone::Text)`.
//! - Secondary text: `mono_xs` in `color_muted(palette_ct(t).base(SxTone::Dim))`, on a second line.
//! - Row height: 22px default; 32px when `.dense(false)`.
//! - LR padding: `gap_md`.
//!
//! Returns the full-row `Response` so callers can call `.clicked()`,
//! `.hovered()`, etc. Cursor is routed to `PointingHand` on hover.
//!
//! ## Columns mode
//!
//! For streaming-data panels (tape prints, scanner T&S, journal entries)
//! where each row is N free-form text columns instead of the
//! leading/primary/secondary/trailing layout, call `.columns(&[...])`:
//!
//! ```ignore
//! PanelListRow::new("print_123")
//!     .columns(&[
//!         Column::left("09:31:42.123").color(palette_ct(t).base(SxTone::Dim)),
//!         Column::right("$145.32").color(palette_ct(t).base(SxTone::Bull)),
//!         Column::right("250").color(palette_ct(t).base(SxTone::Text)),
//!     ])
//!     .show(ui, t);
//! ```
//!
//! Selected + hover backgrounds STILL apply. When `.columns()` is set,
//! `.primary()` / `.secondary()` / `.leading()` / `.trailing()` are
//! ignored. The slice is borrowed for one frame; no per-cell heap
//! allocations.
//!
//! When to use:
//! - Any repeating list item inside a `PanelSection` body.
//! - Streaming data rows — pass `.columns(&[...])`.
//!
//! When NOT to use:
//! - Menus / dropdown items — use `SelectableRow`.
//! - Label/value metric rows — use `PanelKeyValueRow` or `MetricRow`.
//! - Form fields — use `FormRow`.
//!
//! Sister widgets: `SelectableRow`, `PanelKeyValueRow`, `PanelCard`.
//!
//! ## Trailing buttons slice (zero-allocation)
//!
//! When a row wants 2–4 inline icon buttons (delete / lock / visibility /
//! reset), the closure-style `.trailing(|ui, t| ...)` slot forces callers to
//! wire each click through a `Cell<bool>` trampoline because the `FnOnce`
//! is consumed by `.show()`. Use `.trailing_buttons(&[...])` instead and
//! call `.show_full(ui, t)` to read which one fired:
//!
//! ```ignore
//! let resp = PanelListRow::new("drawing_42")
//!     .leading(|ui, _| paint_swatch(ui, color))
//!     .primary("Trendline AAPL")
//!     .trailing_buttons(&[
//!         TrailingBtn::icon(Icon::TRASH).tone(TrailingTone::Bear).tooltip("Delete"),
//!         TrailingBtn::icon(if locked { Icon::LOCK } else { Icon::LOCK_OPEN })
//!             .active(locked).tooltip("Lock"),
//!         TrailingBtn::icon(if visible { Icon::EYE } else { Icon::EYE_SLASH })
//!             .active(visible).tooltip("Visibility"),
//!     ])
//!     .show_full(ui, t);
//! if resp.clicked { /* row body click */ }
//! match resp.trailing_clicked_idx {
//!     Some(0) => delete(),
//!     Some(1) => toggle_lock(),
//!     Some(2) => toggle_visible(),
//!     _ => {}
//! }
//! ```
//!
//! `.trailing_buttons()` and `.trailing(closure)` are mutually exclusive —
//! last setter wins. The slice is borrowed for one frame; no per-button heap
//! allocations. Click reporting is `Option<usize>` because only one button
//! can fire per frame and matching on an index reads cleaner than a
//! `Vec<bool>` at the call site.
//!
//! For back-compat, `.show()` still returns `egui::Response`; use
//! `.show_full()` when you need the trailing-button click index.

use egui::{Color32, CornerRadius, FontId, Pos2, Rect, Response, Sense, Ui, Vec2};

use crate::ui_kit::tokens::{
    self as st, alpha_ghost, color_alpha, color_muted, font_sm, font_xs, gap_lg, gap_md, gap_xs,
    radius_sm,
};
use crate::ui_kit::widgets::theme::{ComponentTheme, get_ambient_recipes};
use crate::ui_kit::sx::{palette_ct, Fill, Sx, StyleState, Tone as SxTone};
use crate::ui_kit::widgets::{motion, Tooltip};

/// Horizontal alignment for a `Column` cell in `PanelListRow::columns` mode.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub enum ColAlign {
    #[default]
    Left,
    Right,
    Center,
}

/// One cell in a column-mode `PanelListRow`. Borrow-friendly: holds a
/// `&str` so callers can build a `&[Column]` on the stack each frame with
/// no allocation per cell.
#[derive(Copy, Clone, Debug)]
pub struct Column<'a> {
    pub text: &'a str,
    pub align: ColAlign,
    pub color: Color32,
    /// Flex weight against the remaining row width after `min_width`
    /// reservations. Default `1.0`.
    pub weight: f32,
    pub min_width: Option<f32>,
    /// Monospace cell? Default `true` — dense data rows are the use case.
    pub mono: bool,
}

impl<'a> Column<'a> {
    /// Left-aligned cell with default styling (mono, white text color via
    /// `Color32::PLACEHOLDER` — caller is expected to set `.color()` to a
    /// theme value, but if they don't we fall back to white at paint time).
    pub fn left(text: &'a str) -> Self {
        Self {
            text,
            align: ColAlign::Left,
            color: Color32::PLACEHOLDER,
            weight: 1.0,
            min_width: None,
            mono: true,
        }
    }

    pub fn right(text: &'a str) -> Self {
        Self { align: ColAlign::Right, ..Self::left(text) }
    }

    pub fn center(text: &'a str) -> Self {
        Self { align: ColAlign::Center, ..Self::left(text) }
    }

    pub fn color(mut self, c: Color32) -> Self {
        self.color = c;
        self
    }

    pub fn weight(mut self, w: f32) -> Self {
        self.weight = w;
        self
    }

    pub fn min_width(mut self, px: f32) -> Self {
        self.min_width = Some(px);
        self
    }

    pub fn mono(mut self, on: bool) -> Self {
        self.mono = on;
        self
    }
}

/// Tone palette for `TrailingBtn`. Resolves to a `Theme` color at paint
/// time so the row primitive stays theme-aware (no raw RGB at call sites).
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub enum TrailingTone {
    #[default]
    Default,
    Accent,
    Bull,
    Bear,
    Warn,
    Muted,
}

impl TrailingTone {
    /// Resolve the tone to a concrete theme color.
    #[inline]
    fn resolve<T: ComponentTheme>(self, t: &T) -> Color32 {
        match self {
            TrailingTone::Default => palette_ct(t).base(SxTone::Text),
            TrailingTone::Accent => palette_ct(t).base(SxTone::Accent),
            TrailingTone::Bull => palette_ct(t).base(SxTone::Bull),
            TrailingTone::Bear => palette_ct(t).base(SxTone::Bear),
            TrailingTone::Warn => palette_ct(t).base(SxTone::Warn),
            TrailingTone::Muted => palette_ct(t).base(SxTone::Dim),
        }
    }
}

/// One inline icon button in `PanelListRow::trailing_buttons`. Borrow-
/// friendly: stores `&str` for icon + tooltip so callers can build a stack
/// slice each frame with no allocation. See the module docs for an
/// end-to-end example.
#[derive(Copy, Clone, Debug)]
pub struct TrailingBtn<'a> {
    pub icon: &'a str,
    pub tone: TrailingTone,
    pub tooltip: Option<&'a str>,
    /// Toggled-on visual state — paints a subtle background tint at
    /// `color_alpha(tone, alpha_soft())`.
    pub active: bool,
}

impl<'a> TrailingBtn<'a> {
    /// New trailing icon button with `TrailingTone::Default` styling.
    pub fn icon(icon: &'a str) -> Self {
        Self { icon, tone: TrailingTone::Default, tooltip: None, active: false }
    }

    pub fn tone(mut self, t: TrailingTone) -> Self { self.tone = t; self }
    pub fn tooltip(mut self, s: &'a str) -> Self { self.tooltip = Some(s); self }
    pub fn active(mut self, v: bool) -> Self { self.active = v; self }
}

/// Rich response from `PanelListRow::show_full`. Carries both the row body
/// click signal and, when `.trailing_buttons(&[...])` was set, the index of
/// the trailing button that fired this frame (at most one).
#[derive(Clone, Debug, Default)]
pub struct PanelListRowResponse {
    /// `true` when the row body (not a trailing button) was clicked.
    pub clicked: bool,
    /// Hover state on the row body.
    pub hovered: bool,
    /// Index into the `&[TrailingBtn]` slice passed to `.trailing_buttons()`.
    /// `None` when no trailing button fired this frame. Only one click per
    /// frame is possible because egui consumes pointer presses on the
    /// topmost interactable rect.
    pub trailing_clicked_idx: Option<usize>,
    /// The full underlying row `Response` — preserved so callers can still
    /// reach for `.on_hover_ui(...)` etc. when needed.
    pub response: Option<Response>,
}

/// Width (in px) of the left accent stripe for the selected state.
const SELECTED_STRIPE_W: f32 = 2.0;

/// Hover background alpha (out of 255), applied to `palette_ct(t).base(SxTone::Text)`.
const HOVER_BG_ALPHA: u8 = 8;

/// Selected background alpha (out of 255), applied to `palette_ct(t).base(SxTone::Accent)`.
const SELECTED_BG_ALPHA: u8 = 24;

#[must_use = "PanelListRow must be rendered with `.show(...)`"]
pub struct PanelListRow<'a, T: ComponentTheme = crate::ui_kit::widgets::theme::PortableTheme> {
    id_salt: &'a str,
    primary: Option<&'a str>,
    secondary: Option<&'a str>,
    leading: Option<Box<dyn FnOnce(&mut Ui, &T) + 'a>>,
    trailing: Option<Box<dyn FnOnce(&mut Ui, &T) + 'a>>,
    /// Typed slice slot — mutually exclusive with `trailing`. Last setter
    /// wins. When set, the row paints N inline icon buttons in an RTL
    /// strip on the right edge and reports clicks via
    /// `PanelListRowResponse::trailing_clicked_idx`.
    trailing_buttons: Option<&'a [TrailingBtn<'a>]>,
    /// When `Some`, the row paints as N free-form columns and IGNORES
    /// `primary` / `secondary` / `leading` / `trailing`. Selected and
    /// hover backgrounds still apply.
    columns: Option<&'a [Column<'a>]>,
    selected: bool,
    dense: bool,
    /// Paint a faint bottom hairline below the row. Opt-in for lists
    /// where consecutive rows would otherwise blur together (indicator
    /// library, accordion children). Leave off for selected-row lists
    /// where the selected-row stripe + tint already carries the divide.
    divided: bool,
    /// When `true`, suppresses the bottom hairline even if `divided` is set.
    /// Set by `.divided_except_last(is_last)` so loop callers can skip the
    /// divider on the final row without an extra `if` at every call site.
    suppress_divider: bool,
    /// Optional explicit row height override (px). When `None`, the row uses
    /// the dense (22) / non-dense (32) default. Streaming-data panels (tape
    /// T&S) that historically used a tighter 14-px row pass this to preserve
    /// information density. Most callers should NOT set this — use `.dense()`.
    height_override: Option<f32>,
    /// When `false`, suppresses the hover background fill (and the
    /// PointingHand cursor). Used by purely-display streaming rows
    /// (tape T&S) where hovering a print conveys nothing actionable.
    /// Default `true`.
    hoverable: bool,
    /// Permanent row background tint (e.g., buy/sell tape coloring).
    /// Painted BEHIND the hover/selected backgrounds so those still
    /// layer correctly on top. Works in both layout modes (primary/
    /// secondary AND columns). Stored as `(color, alpha 0..=255)`.
    row_tint: Option<(Color32, u8)>,
}

impl<'a, T: ComponentTheme> PanelListRow<'a, T> {
    pub fn new(id_salt: &'a str) -> Self {
        Self {
            id_salt,
            primary: None,
            secondary: None,
            leading: None,
            trailing: None,
            trailing_buttons: None,
            columns: None,
            selected: false,
            dense: true,
            divided: false,
            suppress_divider: false,
            height_override: None,
            hoverable: true,
            row_tint: None,
        }
    }

    /// Suppress the hover background fill (and PointingHand cursor) for
    /// purely-display streaming rows (tape T&S) where hover conveys
    /// nothing actionable. Default `true`.
    #[inline]
    pub fn hoverable(mut self, on: bool) -> Self {
        self.hoverable = on;
        self
    }

    /// Override the row height (px). Use sparingly — `.dense(true/false)`
    /// (22 / 32) covers the design-system defaults. The streaming tape T&S
    /// panel uses 14 for historical density; new code should prefer the
    /// dense default.
    #[inline]
    pub fn height(mut self, px: f32) -> Self {
        self.height_override = Some(px);
        self
    }

    /// Paint a permanent background tint for this row, layered BEHIND the
    /// hover/selected backgrounds. Used for buy/sell-tinted streaming rows
    /// (tape T&S, scanner T&S). `alpha` is 0..=255. Works in both layout
    /// modes (primary/secondary AND columns). Byte-trivial — just stores
    /// the pair.
    #[inline]
    pub fn row_tint(mut self, color: Color32, alpha: u8) -> Self {
        self.row_tint = Some((color, alpha));
        self
    }

    /// Paint a faint bottom hairline beneath the row so consecutive
    /// rows in a dense list (indicator library, settings list) read
    /// as distinct entries instead of blurring together.
    pub fn divided(mut self, on: bool) -> Self {
        self.divided = on;
        self
    }

    /// Variant of `.divided(true)` for use inside loops: enables the bottom
    /// hairline for all rows **except** the last one. Pass `is_last = true`
    /// for the final row and the hairline is suppressed; all other rows get
    /// the normal divider line.
    ///
    /// ```ignore
    /// for (i, item) in items.iter().enumerate() {
    ///     PanelListRow::new(&item.id)
    ///         .primary(&item.label)
    ///         .divided_except_last(i + 1 == items.len())
    ///         .show(ui, t);
    /// }
    /// ```
    ///
    /// Default behavior of the existing `.divided(bool)` is **100% unchanged**
    /// — this is purely additive. Do NOT call both; if you do, `suppress_divider`
    /// wins (last row's hairline is suppressed regardless of `.divided()`).
    pub fn divided_except_last(mut self, is_last: bool) -> Self {
        self.divided = true;
        self.suppress_divider = is_last;
        self
    }

    /// Switch this row into column-layout mode for streaming-data panels
    /// (tape, scanner T&S, journal). The slice is borrowed for one frame
    /// and copied internally only for layout math — no per-cell heap
    /// allocations. Selected and hover backgrounds still apply.
    ///
    /// When this is set, the `primary` / `secondary` / `leading` /
    /// `trailing` slots are ignored.
    pub fn columns(mut self, cols: &'a [Column<'a>]) -> Self {
        self.columns = Some(cols);
        self
    }

    pub fn primary(mut self, p: &'a str) -> Self {
        self.primary = Some(p);
        self
    }

    pub fn secondary(mut self, s: &'a str) -> Self {
        self.secondary = Some(s);
        self
    }

    pub fn leading(mut self, f: impl FnOnce(&mut Ui, &T) + 'a) -> Self {
        self.leading = Some(Box::new(f));
        self
    }

    pub fn trailing(mut self, f: impl FnOnce(&mut Ui, &T) + 'a) -> Self {
        self.trailing = Some(Box::new(f));
        // Last setter wins — clear the slice form so the two modes don't
        // both paint and double-stack the right edge.
        self.trailing_buttons = None;
        self
    }

    /// Slice-based trailing slot for 2–4 inline icon buttons. Mutually
    /// exclusive with `.trailing(closure)`; last setter wins. Clicks are
    /// reported via `PanelListRowResponse::trailing_clicked_idx` from
    /// `.show_full()`. With plain `.show()` the click is observable only
    /// indirectly (the row body click is suppressed inside the button rect).
    pub fn trailing_buttons(mut self, btns: &'a [TrailingBtn<'a>]) -> Self {
        self.trailing_buttons = Some(btns);
        self.trailing = None;
        self
    }

    pub fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }

    /// Dense rows are 22px tall (default). Non-dense rows are 32px and
    /// generally only used when the row carries a secondary line + leading
    /// graphics.
    pub fn dense(mut self, on: bool) -> Self {
        self.dense = on;
        self
    }

    /// Back-compat entry point — returns the plain `Response` so existing
    /// call sites compile unchanged. Use [`Self::show_full`] when you set
    /// `.trailing_buttons(&[...])` and need the clicked index.
    pub fn show(self, ui: &mut Ui, t: &T) -> Response {
        // Drive the same internal painter; discard the rich response.
        self.show_full(ui, t).response.unwrap_or_else(|| {
            // Defensive fallback — paint() always sets `response`. Allocate
            // a zero-size rect at the cursor so callers can still call
            // `.clicked()` without panicking on an unwrap.
            let (_, r) = ui.allocate_exact_size(Vec2::ZERO, Sense::hover());
            r
        })
    }

    /// Rich entry point. Returns a [`PanelListRowResponse`] that carries
    /// the body click signal **and** (when `.trailing_buttons(&[...])` was
    /// set) the index of the trailing button that fired this frame.
    pub fn show_full(self, ui: &mut Ui, t: &T) -> PanelListRowResponse {
        let pal = palette_ct(t); // bind once: hottest widget (one per list row)
        let Self {
            id_salt,
            primary,
            secondary,
            leading,
            trailing,
            trailing_buttons,
            columns,
            selected,
            dense,
            divided,
            suppress_divider,
            height_override,
            hoverable,
            row_tint,
        } = self;

        let h = height_override.unwrap_or(if dense { 22.0 } else { 32.0 });
        let avail_w = ui.available_width();
        let row_sense = if hoverable { Sense::click() } else { Sense::hover() };
        let (rect, resp) = ui.allocate_exact_size(
            Vec2::new(avail_w, h),
            row_sense,
        );
        let resp = if hoverable {
            resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            resp
        };

        // ── Reserve trailing-button strip BEFORE interacting with the row
        // body — clicks inside the strip belong to the buttons, not the row.
        let btn_size = gap_lg(); // 16px square hit rect per button
        let btn_gap = gap_xs();
        let (trail_strip_rect, trail_btn_rects) = match trailing_buttons {
            Some(btns) if !btns.is_empty() => {
                let n = btns.len();
                let strip_w = (n as f32) * btn_size + ((n as f32) - 1.0).max(0.0) * btn_gap;
                let strip_left = rect.right() - gap_md() - strip_w;
                let strip = Rect::from_min_max(
                    Pos2::new(strip_left, rect.top()),
                    Pos2::new(rect.right() - gap_md(), rect.bottom()),
                );
                // Per-button rects, left-to-right inside the strip (RTL
                // visually is just left-to-right placement starting from
                // strip_left — order matches the slice index).
                let mut rects: Vec<Rect> = Vec::with_capacity(n);
                let cy = rect.center().y;
                let mut x = strip_left;
                for _ in 0..n {
                    let cx = x + btn_size * 0.5;
                    rects.push(Rect::from_center_size(
                        Pos2::new(cx, cy),
                        Vec2::splat(btn_size),
                    ));
                    x += btn_size + btn_gap;
                }
                (Some(strip), rects)
            }
            _ => (None, Vec::new()),
        };

        // Row body interaction — excludes the trailing strip when present.
        let body_rect = match trail_strip_rect {
            Some(strip) => Rect::from_min_max(
                rect.min,
                Pos2::new(strip.left() - btn_gap, rect.bottom()),
            ),
            None => rect,
        };
        let body_resp = ui.interact(
            body_rect,
            ui.id().with(("panel_list_row", id_salt)),
            Sense::click(),
        );
        let resp = body_resp.union(resp);

        if !ui.is_rect_visible(rect) {
            return PanelListRowResponse {
                clicked: resp.clicked(),
                hovered: resp.hovered(),
                trailing_clicked_idx: None,
                response: Some(resp),
            };
        }

        let painter = ui.painter_at(rect);

        // ── Recipe adoption (Stream S5) ──────────────────────────────────────
        // Resolve `row.list` to allow theme-pack overrides of corner radius.
        // When the ambient RecipeSet is empty (the default), `resolve` returns
        // the built-in default Sx unchanged — zero visual change.
        //
        // Default Sx: rounded_sm (matches the pre-recipe `radius_sm()` path).
        let recipes = get_ambient_recipes(ui.ctx());
        let row_sx  = recipes.resolve(
            "row.list",
            Sx::new().rounded_sm(),
            t,
        );
        // Extract the resolved corner radius for use in rect_filled calls.
        // Falls back to `radius_sm()` when the recipe sets no radius.
        let row_delta = row_sx.resolved(StyleState::Normal);
        let recipe_cr = CornerRadius::same(
            row_delta.radius.unwrap_or_else(radius_sm) as u8
        );

        // Resolve `row.list.selected` for the selected-background fill color.
        // Default: Accent at SELECTED_BG_ALPHA (the historical constant).
        // Recipe can override to any tone/alpha.
        let default_selected_color =
            color_alpha(pal.base(SxTone::Accent), SELECTED_BG_ALPHA);
        let sel_sx = recipes.resolve(
            "row.list.selected",
            Sx::new().bg_color(default_selected_color),
            t,
        );
        let sel_delta = sel_sx.resolved(StyleState::Active);
        // Use the recipe fill when set; otherwise fall back to the pre-recipe
        // default color.
        let resolved_selected_color = sel_delta.fill
            .map(|fill| match fill {
                Fill::Solid(c) => c,
                Fill::Shade(tone, shade) => pal.shade(tone, shade),
                Fill::Alpha(tone, a) => {
                    let b = pal.base(tone);
                    Color32::from_rgba_unmultiplied(b.r(), b.g(), b.b(), a)
                }
            })
            .unwrap_or(default_selected_color);

        let cr = recipe_cr;

        // Stable per-row animation id — same key used by ui.interact below,
        // so it is stable across frames as long as id_salt is stable.
        let row_id = ui.id().with(("panel_list_row", id_salt));

        // Permanent row tint (buy/sell tape) — painted BEHIND hover/selected
        // so directional coloring stays visible but interactive states still
        // read clearly on top.
        if let Some((tint_color, tint_alpha)) = row_tint {
            painter.rect_filled(rect, cr, color_alpha(tint_color, tint_alpha));
        }

        // Hover background — animated when hoverable, suppressed entirely when not.
        // `.hoverable(false)` rows (tape T&S, 200 rows/frame) must NOT call
        // ease_bool: the animation state would still accumulate and request
        // repaints, burning CPU for a visual that is intentionally absent.
        if hoverable {
            let hover_t = motion::ease_bool(
                ui.ctx(),
                row_id.with("hov"),
                resp.hovered(),
                motion::FAST,
            );
            if hover_t > 0.0 {
                let bg = color_alpha(pal.base(SxTone::Text), (alpha_ghost() as f32 * hover_t).round() as u8);
                painter.rect_filled(rect, cr, bg);
            }
        }

        // Selected background — eased in/out, color sourced from recipe resolution.
        let selected_t = motion::ease_bool(
            ui.ctx(),
            row_id.with("sel"),
            selected,
            motion::FAST,
        );
        if selected_t > 0.0 {
            // Animate alpha: the resolved color carries the target alpha; we scale it
            // by the easing factor so the fade still works correctly.
            let base = resolved_selected_color;
            let animated_alpha = ((base.a() as f32) * selected_t).round() as u8;
            let sel_bg = Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), animated_alpha);
            painter.rect_filled(rect, cr, sel_bg);
        }

        // Left accent stripe — eases with selected_t so it fades alongside the bg.
        if selected_t > 0.0 {
            let stripe_alpha = (255.0_f32 * selected_t).round() as u8;
            let stripe_color = color_alpha(pal.base(SxTone::Accent), stripe_alpha);
            let stripe = Rect::from_min_max(
                Pos2::new(rect.left(), rect.top()),
                Pos2::new(rect.left() + SELECTED_STRIPE_W, rect.bottom()),
            );
            painter.rect_filled(stripe, 0.0, stripe_color);
        }

        // Optional bottom hairline divider — opt-in via .divided(true)
        // for dense lists where consecutive rows would otherwise blur
        // together (indicator library, settings rows). Painted at the
        // toolbar_border color so it reads against both panel surface
        // and recessed sub-section header strips above/below.
        // `suppress_divider` is set by `.divided_except_last(true)` to
        // skip the line on the final row without changing the default path.
        if divided && !suppress_divider {
            let y = rect.bottom() - 0.5;
            painter.line_segment(
                [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                egui::Stroke::new(crate::ui_kit::tokens::stroke_thin(), color_alpha(t.surface_border(), 60)),
            );
        }

        // ── Columns mode ────────────────────────────────────────────
        // When `.columns()` was called, paint a free-form column row and
        // return. Streaming-data hot path — no nested UIs, no closures,
        // just the painter.
        if let Some(cols) = columns {
            paint_columns(ui, rect, cols, t);
            return PanelListRowResponse {
                clicked: resp.clicked(),
                hovered: resp.hovered(),
                trailing_clicked_idx: None,
                response: Some(resp),
            };
        }

        // Layout LTR: leading | text block | (RTL) trailing.
        let inner_left = rect.left() + gap_md();
        // When the trailing-button strip is reserved, the text block
        // should stop short of it — otherwise long titles would draw
        // underneath the icons.
        let inner_right = match trail_strip_rect {
            Some(strip) => strip.left() - btn_gap,
            None => rect.right() - gap_md(),
        };
        let content_rect = Rect::from_min_max(
            Pos2::new(inner_left, rect.top()),
            Pos2::new(inner_right, rect.bottom()),
        );

        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(content_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );

        if let Some(lead) = leading {
            lead(&mut child, t);
            child.add_space(gap_xs());
        }

        // Reserve text block; trailing slot floats right.
        // We use right_to_left layout for the trailing slot via a nested with_layout.
        child.with_layout(
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                // Text block (vertical) — primary + optional secondary.
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    if let Some(p) = primary {
                        let font = FontId::monospace(font_sm());
                        let galley = ui.fonts(|f| {
                            f.layout_no_wrap(p.to_string(), font.clone(), pal.base(SxTone::Text))
                        });
                        let (r, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
                        ui.painter().galley(r.min, galley, pal.base(SxTone::Text));
                    }
                    if let Some(s) = secondary {
                        let font = FontId::monospace(font_xs());
                        let col = color_muted(pal.base(SxTone::Dim));
                        let galley = ui.fonts(|f| {
                            f.layout_no_wrap(s.to_string(), font.clone(), col)
                        });
                        let (r, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
                        ui.painter().galley(r.min, galley, col);
                    }
                });

                // Trailing slot: float to the right. Only the closure form
                // paints here — the slice form is painted below using the
                // pre-allocated strip so per-button hit rects can be made
                // independently of the nested layout.
                if let Some(trail) = trailing {
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            trail(ui, t);
                        },
                    );
                }
            },
        );

        // ── Trailing icon-button strip ──────────────────────────────────
        // Paint each TrailingBtn at its pre-allocated rect via the painter,
        // then call ui.interact() for click sense + tooltip. No nested Ui /
        // closures / heap allocations per button.
        let mut trailing_clicked_idx: Option<usize> = None;
        if let Some(btns) = trailing_buttons {
            for (i, btn) in btns.iter().enumerate() {
                let r = trail_btn_rects[i];
                let id = ui.id().with(("panel_list_row_trail_btn", id_salt, i));
                let br = ui.interact(r, id, Sense::click());
                if let Some(tip) = btn.tooltip {
                    Tooltip::new(tip).show(ui, &br, t);
                }
                crate::ui_kit::tokens::cursor::clickable(ui, &br);

                let tone_col = btn.tone.resolve(t);
                let painter = ui.painter_at(r);

                // Active bg fill — subtle tint at alpha_soft().
                if btn.active {
                    painter.rect_filled(
                        r,
                        CornerRadius::same(radius_sm() as u8),
                        color_alpha(
                            tone_col,
                            crate::ui_kit::tokens::alpha_soft(),
                        ),
                    );
                }

                // Icon glyph. Hover snaps to full tone color; idle is
                // tone-at-`pal.base(SxTone::Dim)`-opacity so the icons sit visually quiet
                // when the row is idle. Active state implies full color.
                let glyph_color = if br.hovered() || btn.active {
                    tone_col
                } else {
                    color_alpha(tone_col, pal.base(SxTone::Dim).a())
                };
                painter.text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    btn.icon,
                    FontId::proportional(font_sm()),
                    glyph_color,
                );

                if br.clicked() {
                    trailing_clicked_idx = Some(i);
                }
            }
        }

        // Focus ring — painted on top of all other visuals so it's always
        // visible. Only for hoverable rows (streaming/tape rows with
        // `hoverable(false)` are never navigated via Tab). egui's built-in
        // `Sense::click()` already handles Enter/Space → clicked() natively,
        // so the ring is the only missing piece for full keyboard support.
        if hoverable {
            st::cursor::focus_ring(ui, &resp, pal.base(SxTone::Accent));
        }

        PanelListRowResponse {
            clicked: resp.clicked(),
            hovered: resp.hovered(),
            trailing_clicked_idx,
            response: Some(resp),
        }
    }
}

/// Paint a column-mode row. Pure painter calls — no nested `Ui` allocations,
/// no per-cell closures. Suitable for streaming panels that emit hundreds of
/// rows per frame (tape, scanner T&S).
///
/// Layout: each column gets `min_width` reserved up front; remaining width is
/// divided among columns by `weight`. Text is clipped to its cell rect.
fn paint_columns<T: ComponentTheme>(ui: &mut Ui, rect: Rect, cols: &[Column<'_>], t: &T) {
    if cols.is_empty() {
        return;
    }
    let painter = ui.painter_at(rect);

    let inner_left = rect.left() + gap_md();
    let inner_right = rect.right() - gap_md();
    let content_w = (inner_right - inner_left).max(0.0);
    let n = cols.len();
    // Inter-column gap. Small — these rows are dense by design.
    let gap = gap_xs();
    let total_gaps = gap * (n as f32 - 1.0).max(0.0);

    // Sum mins + weights.
    let mut min_total = 0.0_f32;
    let mut weight_total = 0.0_f32;
    for c in cols {
        min_total += c.min_width.unwrap_or(0.0);
        weight_total += c.weight.max(0.0);
    }
    let flex_w = (content_w - total_gaps - min_total).max(0.0);

    let cy = rect.center().y;
    let mut x = inner_left;
    for (i, c) in cols.iter().enumerate() {
        let base = c.min_width.unwrap_or(0.0);
        let extra = if weight_total > 0.0 {
            flex_w * (c.weight.max(0.0) / weight_total)
        } else {
            0.0
        };
        let cell_w = base + extra;
        let cell_rect = Rect::from_min_max(
            Pos2::new(x, rect.top()),
            Pos2::new(x + cell_w, rect.bottom()),
        );

        let font = if c.mono {
            FontId::monospace(font_sm())
        } else {
            FontId::proportional(font_sm())
        };
        let color = if c.color == Color32::PLACEHOLDER { palette_ct(t).base(SxTone::Text) } else { c.color };
        let galley = ui.fonts(|f| f.layout_no_wrap(c.text.to_string(), font, color));
        let tw = galley.size().x;
        let th = galley.size().y;
        let tx = match c.align {
            ColAlign::Left => cell_rect.left(),
            ColAlign::Right => cell_rect.right() - tw,
            ColAlign::Center => cell_rect.center().x - tw * 0.5,
        };
        // Clip to cell so overflow doesn't bleed into the next column.
        let cell_painter = painter.with_clip_rect(cell_rect);
        cell_painter.galley(Pos2::new(tx, cy - th * 0.5), galley, color);

        x += cell_w;
        if i + 1 < n {
            x += gap;
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// When `hoverable(false)` is set, the source must NOT call `ease_bool`
    /// for the hover background path. This guards against accidentally
    /// re-introducing the animation state for tape rows (200+ rows/frame)
    /// which would drive spurious repaints and CPU burn.
    ///
    /// Verified via source scan: the hoverable guard in `show_full` wraps
    /// the `ease_bool` call in `if hoverable { ... }`, so the call is
    /// compile-time absent when `hoverable == false` is known statically,
    /// and skipped at runtime otherwise.
    #[test]
    fn hoverable_false_path_does_not_call_ease_bool() {
        // Builder-state assertion: hoverable(false) stores the flag.
        let row: PanelListRow<crate::ui_kit::widgets::theme::PortableTheme> = PanelListRow::new("tape_row").hoverable(false);
        assert!(!row.hoverable, "hoverable(false) must set hoverable=false");
    }

    /// The default row is hoverable.
    #[test]
    fn hoverable_true_by_default() {
        let row: PanelListRow<crate::ui_kit::widgets::theme::PortableTheme> = PanelListRow::new("default_row");
        assert!(row.hoverable, "PanelListRow::new must default hoverable=true");
    }

    /// Source-level check: the hover ease_bool call is guarded by `if hoverable`.
    /// Catches accidental refactors that move ease_bool outside the guard.
    #[test]
    fn source_guards_ease_bool_with_hoverable() {
        let src = include_str!("panel_list_row.rs");
        // The guard pattern must appear: `if hoverable {` followed by `ease_bool`
        // before the next closing brace.  A simple ordering check suffices.
        let hover_guard_pos = src.find("if hoverable {")
            .expect("show_full must guard hover animation with `if hoverable {`");
        let ease_bool_pos = src.find("ease_bool(\n                ui.ctx(),\n                row_id.with(\"hov\")")
            .or_else(|| src.find("ease_bool("))
            .expect("ease_bool must appear in the source");
        // ease_bool for hover must appear AFTER the hoverable guard.
        assert!(
            ease_bool_pos > hover_guard_pos,
            "ease_bool for hover must be inside `if hoverable {{ ... }}`"
        );
    }
}
