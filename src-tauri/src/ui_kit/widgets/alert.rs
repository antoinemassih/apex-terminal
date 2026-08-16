//! Alert — inline status banner. Different from Toast (transient overlay)
//! and Tooltip (hover-triggered). Alert lives in document flow.
//!
//! API:
//!   ui.add(Alert::info("New version available."));
//!   ui.add(Alert::error("Order rejected: insufficient buying power."));
//!
//!   Alert::warn("Market is closing in 5 minutes")
//!     .title("Market Close")
//!     .closable(true)
//!     .show(ui, theme);

use egui::{Color32, FontId, Pos2, Response, Sense, Ui, Vec2, Widget};

use super::theme::ComponentTheme;
use crate::ui_kit::layout::{Align as FlexAlign, Flex, Item};
use crate::ui_kit::cascade::El;
use crate::ui_kit::sx::{palette_ct, Sx, Tone};
use crate::ui_kit::tokens as st;
use crate::ui_kit::icons::Icon;
use crate::ui_kit::text_style::TextStyle;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlertVariant {
    #[default] Info,
    Success,
    Warning,
    Error,
}

#[must_use = "Alert does nothing until `.show(ui, theme)` or `ui.add(alert)` is called"]
pub struct Alert {
    message: String,
    title: Option<String>,
    icon: Option<&'static str>,
    variant: AlertVariant,
    closable: bool,
}

pub struct AlertResponse {
    pub response: Response,
    pub closed: bool,
    pub action_clicked: bool,
}

impl Alert {
    pub fn info(message: impl Into<String>) -> Self { Self::new(message, AlertVariant::Info) }
    pub fn success(message: impl Into<String>) -> Self { Self::new(message, AlertVariant::Success) }
    pub fn warn(message: impl Into<String>) -> Self { Self::new(message, AlertVariant::Warning) }
    pub fn error(message: impl Into<String>) -> Self { Self::new(message, AlertVariant::Error) }

    fn new(message: impl Into<String>, variant: AlertVariant) -> Self {
        Self {
            message: message.into(),
            title: None,
            icon: None,
            variant,
            closable: false,
        }
    }

    pub fn variant(mut self, v: AlertVariant) -> Self { self.variant = v; self }
    pub fn title(mut self, text: impl Into<String>) -> Self { self.title = Some(text.into()); self }
    pub fn icon(mut self, icon: &'static str) -> Self { self.icon = Some(icon); self }
    pub fn closable(mut self, v: bool) -> Self { self.closable = v; self }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> AlertResponse {
        // Build the ctx from the UI so it carries the AMBIENT RecipeSet.
        // `StyleCtx::from_theme` would hand this widget an empty set — see
        // `ctx.rs` for why that shim must never be used inside a `show`.
        let sctx = super::ctx::StyleCtx::from_ui(theme, ui);
        self.show_ctx(ui, &sctx)
    }

    /// [`StyleCtx`](super::ctx::StyleCtx) entry point.
    ///
    /// Callers that need per-call-site token overrides or an explicit
    /// `RecipeSet` construct a `StyleCtx` and call this directly; `show`
    /// delegates here with the ambient one.
    pub fn show_ctx(self, ui: &mut Ui, sctx: &super::ctx::StyleCtx<'_>) -> AlertResponse {
        let theme = sctx.theme();
        let color = match self.variant {
            AlertVariant::Info => palette_ct(theme).base(Tone::Accent),
            AlertVariant::Success => palette_ct(theme).base(Tone::Bull),
            AlertVariant::Warning => palette_ct(theme).base(Tone::Warn),
            AlertVariant::Error => palette_ct(theme).base(Tone::Bear),
        };
        let icon = self.icon.unwrap_or_else(|| match self.variant {
            AlertVariant::Info => Icon::CIRCLE,
            AlertVariant::Success => Icon::CHECK,
            AlertVariant::Warning => Icon::SHIELD_WARNING,
            AlertVariant::Error => Icon::X,
        });

        let icon_size: f32 = 18.0;
        let pad = st::gap_sm();
        let gap = st::gap_xs();
        let close_size: f32 = 12.0;

        let avail_w = ui.available_width();

        // ── Row geometry (flexbox) ──────────────────────────────────────────
        //
        // M4.3: the row used to derive its text column by hand —
        //   `content_left_offset = pad + icon_size + gap`
        //   `close_reserve       = close_size + gap + pad`
        //   `text_max_w          = (avail_w - left - reserve).max(40)`
        // — i.e. the padding token appeared four times and the gutter twice,
        // in two subtractions that had to stay in sync with the paint anchors
        // below. That is `icon · text(grows) · ×` with `pad` inset and a `gap`
        // gutter, and the `.max(40)` is `Item::min`.
        //
        // Solved at height 0 on purpose: every child is `Align::Start`, so the
        // vertical offsets are the padding alone and do NOT depend on the row
        // height — which is what breaks the measure↔layout circularity here
        // (the text wraps at the solved column width, and the row height comes
        // from the wrapped text).
        let mut f = Flex::row()
            .padding(pad)
            .gap(gap)
            .align(FlexAlign::Start)
            .item(Item::fixed(icon_size).cross(icon_size))
            .item(Item::grow(1.0).min(40.0));
        if self.closable {
            f = f.item(Item::fixed(close_size).cross(close_size));
        }
        let slots = f.solve(Vec2::new(avail_w, 0.0));
        let icon_slot = slots[0];
        let text_slot = slots[1];
        let close_slot = if self.closable { slots.get(2).copied() } else { None };
        let text_max_w = text_slot.width();

        let title_font = TextStyle::BodySm.font_id_in(ui);
        let body_font = TextStyle::BodySm.font_id_in(ui);

        let text_color = palette_ct(theme).base(Tone::Text);
        let dim_color = palette_ct(theme).base(Tone::Dim);

        let title_galley = self.title.as_ref().map(|t| {
            ui.fonts(|f| f.layout(t.clone(), title_font.clone(), text_color, text_max_w))
        });
        let body_galley = ui.fonts(|f| {
            f.layout(self.message.clone(), body_font.clone(), dim_color, text_max_w)
        });

        let title_h = title_galley.as_ref().map(|g| g.size().y).unwrap_or(0.0);
        let body_h = body_galley.size().y;
        let title_gap = if title_galley.is_some() { 2.0 } else { 0.0 };
        let content_h = title_h + title_gap + body_h;
        let h = (content_h + pad * 2.0).max(icon_size + pad * 2.0);

        let desired = Vec2::new(avail_w, h);
        let (rect, response) = ui.allocate_exact_size(desired, Sense::hover());

        let mut closed = false;
        let action_clicked = false;

        if ui.is_rect_visible(rect) {
            // DS#4: the Alert box is DECLARED as an Sx (tinted fill + border on
            // the variant tone) and painted in one call — no hand-rolled
            // rect_filled/rect_stroke. Byte-identical to the prior code.
            let box_tone = match self.variant {
                AlertVariant::Info => Tone::Accent,
                AlertVariant::Success => Tone::Bull,
                AlertVariant::Warning => Tone::Warn,
                AlertVariant::Error => Tone::Bear,
            };
            // `alert` key. The tone stays with the widget (it encodes
            // Info/Success/Warning/Error); the recipe governs shape.
            super::theme::resolve_sx(ui.ctx(), theme, "alert",
                Sx::new()
                    .rounded_md()
                    .bg_alpha(box_tone, 32)
                    .border_alpha(box_tone, 200, st::stroke_std()),
            ).paint_box_ct(ui, rect, theme);

            let painter = ui.painter_at(rect);

            let off = rect.min.to_vec2();

            // Leading icon
            let icon_center = icon_slot.translate(off).center();
            painter.text(
                icon_center,
                egui::Align2::CENTER_CENTER,
                icon,
                crate::ui_kit::style::prop_at(icon_size),
                color,
            );

            // Title + body — a DECLARED column, not a walked `y`.
            //
            // The gap belongs BETWEEN the two, and a titleless alert must not
            // pay for it. In the walked form that was true only because
            // `y += title_h + title_gap` sat inside the `if`; here it is a
            // property of the tree, so it cannot be got wrong by moving a line.
            let text_col_rect = text_slot.translate(off);
            let text_x = text_col_rect.left();
            let stack = El::column()
                .gap(title_gap)
                .child_if(
                    title_galley.is_some(),
                    El::slot("title", egui::vec2(0.0, title_h)),
                )
                .child(El::slot("body", egui::Vec2::ZERO).grow(1.0))
                .solve_rect(text_col_rect);
            let mut y = stack.rect("body").top();
            if let Some(g) = title_galley {
                y = stack.rect("title").top();
                // Keep the egui galley for height measurement (drives
                // `title_h` and the body_y advance), but paint the
                // glyphs via cosmic-text for shaping quality.
                if let Some(title_text) = self.title.as_ref() {
                    crate::ui_kit::widgets::text_engine::paint_polished_label_at(
                        &painter,
                        Pos2::new(text_x, y),
                        title_text,
                        st::font_sm(),
                        cosmic_text::Family::SansSerif,
                        cosmic_text::Weight::SEMIBOLD,
                        text_color,
                    );
                } else {
                    painter.galley(Pos2::new(text_x, y), g, text_color);
                }
            }
            painter.galley(Pos2::new(text_x, stack.rect("body").top()), body_galley, dim_color);

            // Close button
            if let Some(slot) = close_slot {
                let close_center = slot.translate(off).center();
                let close_rect = egui::Rect::from_center_size(close_center, Vec2::splat(close_size + 6.0));
                let close_resp = ui.interact(close_rect, response.id.with("alert_close"), Sense::click());
                let col = if close_resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    text_color
                } else {
                    dim_color
                };
                painter.text(
                    close_center,
                    egui::Align2::CENTER_CENTER,
                    Icon::X,
                    crate::ui_kit::style::prop_at(close_size),
                    col,
                );
                if close_resp.clicked() { closed = true; }
            }

            let _ = Color32::TRANSPARENT;
        }

        AlertResponse { response, closed, action_clicked }
    }
}

impl Widget for Alert {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = super::theme::active_theme(ui.ctx());
        self.show(ui, &theme).response
    }
}
