//! ThemePreviewCard — interactive theme preview tile.
//!
//! Renders a tiny mock editor (background bar + colored line strokes
//! representing code) inside a clickable card. Selected state swaps
//! the always-reserved 2px border from transparent to accent.
//! Zero layout shift on selection — pure color swap.
//!
//! API:
//!   ThemePreviewCard::new("Gruvbox", &gruvbox_theme)
//!     .selected(active_theme_idx == 2)
//!     .show(ui, theme);

use egui::{Color32, CornerRadius, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

use super::label::Label;
use super::theme::ComponentTheme;
use super::tokens::Size;
use crate::ui_kit::layout::{Align as FlexAlign, Flex, Item};
use crate::ui_kit::tokens as st;
// The card chrome (border/selection) uses the ACTIVE theme via Sx; the swatch
// it previews deliberately keeps raw `preview_theme.*` reads (it's showing a
// *different* theme's colors, so it must not route through the active palette).
use crate::ui_kit::sx::{palette_ct, Tone};
use crate::ui_kit::interaction::{
    apply_interaction, HoverTreatment, InteractionState, InteractionTokens,
};

const DEFAULT_SIZE: Vec2 = Vec2::new(132.0, 80.0);
const BORDER_WIDTH: f32 = 2.0;
const CARD_RADIUS: f32 = 6.0;

/// Selects which mock content is rendered inside a `ThemePreviewCard`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PreviewKind {
    /// 5 horizontal lines representing code (default).
    #[default]
    Code,
    /// Mock candlesticks for trading-app theme pickers.
    Chart,
}

#[must_use = "ThemePreviewCard does nothing until `.show(ui, theme)` is called"]
pub struct ThemePreviewCard<'a> {
    label: String,
    preview_theme: &'a dyn ComponentTheme,
    selected: bool,
    size: Vec2,
    preview_kind: PreviewKind,
}

impl<'a> ThemePreviewCard<'a> {
    pub fn new(label: impl Into<String>, preview_theme: &'a dyn ComponentTheme) -> Self {
        Self {
            label: label.into(),
            preview_theme,
            selected: false,
            size: DEFAULT_SIZE,
            preview_kind: PreviewKind::Code,
        }
    }

    pub fn selected(mut self, v: bool) -> Self {
        self.selected = v;
        self
    }

    pub fn size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }

    /// Override the mock content rendered inside the card.
    /// Default: `PreviewKind::Code`. For trading apps, prefer `Chart`.
    pub fn preview_kind(mut self, kind: PreviewKind) -> Self {
        self.preview_kind = kind;
        self
    }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response {
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
    pub fn show_ctx(self, ui: &mut Ui, sctx: &super::ctx::StyleCtx<'_>) -> Response {
        let theme = sctx.theme();
        let Self { label, preview_theme, selected, size, preview_kind } = self;

        // Allocate full vertical space (card + gap + label) so the layout
        // is stable whether or not the label wraps.
        let label_h = crate::ui_kit::tokens::font_xs() + 2.0;
        let gap = crate::ui_kit::tokens::gap_2xs();
        let total = Vec2::new(size.x, size.y + gap + label_h);

        let (rect, response) = ui.allocate_exact_size(total, Sense::click());
        if !ui.is_rect_visible(rect) {
            return response;
        }

        // M4.3: the outer stack was two hand-built rects — `Rect::from_min_size
        // (rect.min, size)` and `Pos2::new(rect.left(), card_rect.bottom() +
        // gap)` — which carried the gutter twice (once in the `total` height
        // above, once in the label anchor). One flex column states it once.
        let stack: Vec<Rect> = Flex::column()
            .gap(gap)
            .align(FlexAlign::Stretch)
            .item(Item::fixed(size.y))
            .item(Item::fixed(label_h))
            .solve(total)
            .into_iter()
            .map(|r| r.translate(rect.min.to_vec2()))
            .collect();
        let card_rect = stack[0];
        let painter = ui.painter_at(card_rect);

        // Card background.
        painter.rect_filled(card_rect, CornerRadius::same(CARD_RADIUS as u8), preview_theme.bg());

        // Hover overlay (subtle, on top of bg, before mock editor).
        // M3.3: routed through the ONE interaction table. The overlay colour is
        // the theme's own `element_hover()` (alpha already baked in), so the
        // treatment is `Custom` rather than an accent tint.
        let ix = apply_interaction(
            card_rect,
            InteractionState::new().hovered(response.hovered()),
            theme.element_hover(),
            &InteractionTokens::borderless()
                .treatment(HoverTreatment::Custom(theme.element_hover())),
        );
        if ix.fill != Color32::TRANSPARENT {
            painter.rect_filled(card_rect, CornerRadius::same(CARD_RADIUS as u8), ix.fill);
        }

        // Inset from card edges.
        let pad_x = 10.0;
        let pad_y = 12.0;
        let inner_left = card_rect.left() + pad_x;
        let inner_right = card_rect.right() - pad_x;
        let inner_top = card_rect.top() + pad_y;
        let inner_bottom = card_rect.bottom() - pad_y;
        let inner_w = inner_right - inner_left;
        let inner_h = inner_bottom - inner_top;

        match preview_kind {
            PreviewKind::Code => {
                // Mock editor: 5 horizontal "code" lines with varying widths and
                // mock indentation. 2px tall, ~3px gap.
                // (indent multiplier, width fraction, use_accent)
                let lines: [(f32, f32, bool); 5] = [
                    (0.00, 0.55, true),   // "fn name() {"
                    (0.18, 0.42, false),
                    (0.18, 0.62, false),
                    (0.36, 0.30, true),
                    (0.00, 0.18, false),  // "}"
                ];
                let line_h = 2.0;
                let line_gap = 4.0;
                let mut y = inner_top;
                for (indent, frac, accent) in lines {
                    let x0 = inner_left + indent * inner_w;
                    let x1 = (x0 + frac * inner_w).min(inner_right);
                    let color = if accent { preview_theme.accent() } else { preview_theme.text() };
                    // Slightly muted so the preview doesn't scream.
                    let color = crate::ui_kit::style::color_alpha(color, crate::ui_kit::style::alpha_solid());
                    let lr = Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, y + line_h));
                    painter.rect_filled(lr, CornerRadius::same(1), color); // TODO: off-token
                    y += line_h + line_gap;
                    if y + line_h > inner_bottom {
                        break;
                    }
                }
            }
            PreviewKind::Chart => {
                // Mock candlesticks: 8 candles equally spaced across inner width.
                // Each candle = body rect + wick line above/below. Bull/bear
                // colors alternate in a hardcoded pattern that looks plausible.
                const N: usize = 8;
                // (is_bull, body_height_frac, body_center_frac_from_top, wick_up, wick_dn)
                // body_center_frac: 0.0 = top of inner, 1.0 = bottom of inner.
                // Centers cluster around 0.55 so floor sits near bottom (~70% of card).
                let candles: [(bool, f32, f32, f32, f32); N] = [
                    (true,  0.30, 0.65, 4.0, 3.0),
                    (true,  0.42, 0.55, 5.0, 2.0),
                    (false, 0.36, 0.50, 3.0, 5.0),
                    (true,  0.50, 0.45, 6.0, 2.0),
                    (false, 0.32, 0.55, 2.0, 4.0),
                    (false, 0.28, 0.62, 3.0, 3.0),
                    (true,  0.46, 0.50, 5.0, 3.0),
                    (true,  0.40, 0.40, 4.0, 2.0),
                ];

                // Faint horizontal price-axis line at bottom.
                let axis_y = card_rect.bottom() - pad_y * 0.5;
                let axis_color = Color32::from_rgba_unmultiplied(
                    preview_theme.dim().r(),
                    preview_theme.dim().g(),
                    preview_theme.dim().b(),
                    60,
                );
                painter.line_segment(
                    [
                        Pos2::new(card_rect.left() + crate::ui_kit::style::gap_xs(), axis_y),
                        Pos2::new(card_rect.right() - crate::ui_kit::style::gap_xs(), axis_y),
                    ],
                    Stroke::new(st::stroke_std(), axis_color),
                );

                let body_w = 3.5_f32;
                let slot_w = inner_w / N as f32;
                for (i, (bull, h_frac, center_frac, wick_up, wick_dn)) in candles.iter().enumerate() {
                    let cx = inner_left + slot_w * (i as f32 + 0.5);
                    let body_h = (h_frac * inner_h).clamp(12.0, 24.0);
                    let center_y = inner_top + center_frac * inner_h;
                    let body_top = center_y - body_h * 0.5;
                    let body_bot = center_y + body_h * 0.5;
                    let color = if *bull { preview_theme.bull() } else { preview_theme.bear() };

                    // Wick (1px line through center).
                    painter.line_segment(
                        [
                            Pos2::new(cx, (body_top - wick_up).max(inner_top - 1.0)),
                            Pos2::new(cx, (body_bot + wick_dn).min(inner_bottom + 1.0)),
                        ],
                        Stroke::new(st::stroke_std(), color),
                    );

                    // Body rect.
                    let body_rect = Rect::from_min_max(
                        Pos2::new(cx - body_w * 0.5, body_top),
                        Pos2::new(cx + body_w * 0.5, body_bot),
                    );
                    painter.rect_filled(body_rect, CornerRadius::same(0), color);
                }

                // Subtle "current price" accent line near the last candle's close.
                let accent_y = inner_top + 0.42 * inner_h;
                painter.line_segment(
                    [
                        Pos2::new(card_rect.left() + crate::ui_kit::style::gap_xs(), accent_y),
                        Pos2::new(card_rect.right() - crate::ui_kit::style::gap_xs(), accent_y),
                    ],
                    Stroke::new(st::stroke_std(), preview_theme.accent()),
                );
            }
        }

        // Border — ALWAYS 2px, painted last so it overlays the card.
        // Color swaps on selection; width never changes (zero layout shift).
        let border_color = if selected { palette_ct(theme).base(Tone::Accent) } else { Color32::TRANSPARENT };
        ui.painter().rect_stroke(
            card_rect,
            CornerRadius::same(CARD_RADIUS as u8),
            Stroke::new(BORDER_WIDTH, border_color),
            StrokeKind::Inside,
        );

        // Label below the card.
        let label_rect = stack[1];
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(label_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        Label::new(label).size(Size::Xs).muted().truncate(true).show(&mut child, theme);

        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        // Focus ring for keyboard navigation — egui's Sense::click() already
        // fires clicked() on Enter/Space when the widget has keyboard focus.
        st::cursor::focus_ring(ui, &response, palette_ct(theme).base(Tone::Accent));

        response
    }
}

impl<'a> egui::Widget for ThemePreviewCard<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let theme = super::theme::active_theme(ui.ctx());
        self.show(ui, &theme)
    }
}
