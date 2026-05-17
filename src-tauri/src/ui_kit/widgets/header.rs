//! Header — the canonical panel / section / dialog header widget.
//!
//! ## When to use
//!
//! - **Panel headers** — side panels, dock panels, modal sections.
//! - **Dialog headers** — modal title rows.
//! - **Section labels** within a panel — sub-grouping inside a single
//!   panel body.
//!
//! Three preset surfaces cover the role:
//!
//! ```ignore
//! Header::panel("ALERTS").show(ui, theme);
//! Header::dialog("Edit Order").show(ui, theme);
//! Header::section("Trigger conditions").show(ui, theme);
//! ```
//!
//! Plus a builder for custom cases:
//!
//! ```ignore
//! Header::new("WATCHLIST", HeaderVariant::Panel)
//!     .subtitle("12 symbols")
//!     .leading_icon(Icon::LIST)
//!     .trailing(|ui, t| {
//!         if ui_kit::Button::icon(Icon::PLUS).show(ui, t).clicked() { ... }
//!     })
//!     .show(ui, theme);
//! ```
//!
//! ## When NOT to use
//!
//! - For pane chrome (chart pane headers with symbol + tabs), use the
//!   in-chart `PaneHeader` in `chart/renderer/ui/chrome/painter_pane.rs`
//!   — it has GPU-aligned layout requirements this widget doesn't share.
//! - For toolbar segmented controls or top-nav, those have their own
//!   primitives in `chart/renderer/ui/components/toolbar/`.
//!
//! ## Migration target
//!
//! Replaces these legacy entry points:
//! - `chart/renderer/ui/components/headers.rs::panel_header()`
//! - `chart/renderer/ui/components/headers_widget.rs::PanelHeaderWithTabs`
//!   (keep this one for now — tabs are out of scope)
//! - `chart/renderer/ui/style.rs::dialog_header()`,
//!   `dialog_header_colored()`, `section_label()`
//! - The panel-internal `chart/renderer/ui/panels/kit.rs::PanelHeader`
//!   which has a hard `Watchlist` coupling. Use this widget for new
//!   panels that don't need that coupling.

use egui::{Align, CornerRadius, FontId, Layout, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::chart_renderer::ui::style as st;
use super::theme::ComponentTheme;

/// Which surface this header sits on. Drives height, font tier, padding,
/// and whether a bottom rule is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderVariant {
    /// Side / dock panel title row. 28px high, uppercase 9px caps, faint
    /// bottom rule.
    Panel,
    /// Modal dialog title row. 36px high, mixed-case 13px, no bottom rule
    /// (modal frame draws its own).
    Dialog,
    /// Sub-section inside a panel body. No background, 9px caps, generous
    /// top padding.
    Section,
}

impl HeaderVariant {
    fn height(self) -> f32 {
        match self {
            HeaderVariant::Panel => 28.0,
            HeaderVariant::Dialog => 36.0,
            HeaderVariant::Section => 22.0,
        }
    }
    fn font_size(self) -> f32 {
        match self {
            HeaderVariant::Panel | HeaderVariant::Section => st::font_xs(),
            HeaderVariant::Dialog => st::font_md(),
        }
    }
    fn upper(self) -> bool {
        matches!(self, HeaderVariant::Panel | HeaderVariant::Section)
    }
    fn bottom_rule(self) -> bool {
        matches!(self, HeaderVariant::Panel)
    }
}

/// Output of a `Header::show(...)` call.
pub struct HeaderResponse {
    /// The full header rect.
    pub rect: egui::Rect,
    /// The header row's response (useful for tooltip/menu anchoring).
    pub response: Option<Response>,
    /// True if the close button was clicked. Only fires when
    /// `.closable(true)` was set.
    pub close_clicked: bool,
}

type Trailing<'a> = Option<Box<dyn FnOnce(&mut Ui, &dyn ComponentTheme) + 'a>>;
type Leading<'a>  = Option<Box<dyn FnOnce(&mut Ui, &dyn ComponentTheme) + 'a>>;

#[must_use = "Header must be rendered with `.show(ui, theme)`"]
pub struct Header<'a> {
    title: &'a str,
    subtitle: Option<&'a str>,
    variant: HeaderVariant,
    leading_icon: Option<&'a str>,
    closable: bool,
    leading: Leading<'a>,
    trailing: Trailing<'a>,
}

impl<'a> Header<'a> {
    /// Build a header explicitly. Most callers should prefer the named
    /// presets (`Header::panel(...)`, `Header::dialog(...)`,
    /// `Header::section(...)`).
    pub fn new(title: &'a str, variant: HeaderVariant) -> Self {
        Self {
            title,
            subtitle: None,
            variant,
            leading_icon: None,
            closable: false,
            leading: None,
            trailing: None,
        }
    }

    /// Panel header preset.
    pub fn panel(title: &'a str) -> Self { Self::new(title, HeaderVariant::Panel) }

    /// Dialog header preset.
    pub fn dialog(title: &'a str) -> Self { Self::new(title, HeaderVariant::Dialog) }

    /// Section label preset.
    pub fn section(title: &'a str) -> Self { Self::new(title, HeaderVariant::Section) }

    /// Add a muted subtitle line below the title.
    pub fn subtitle(mut self, s: &'a str) -> Self {
        self.subtitle = Some(s);
        self
    }

    /// Phosphor icon glyph rendered before the title.
    pub fn leading_icon(mut self, icon: &'a str) -> Self {
        self.leading_icon = Some(icon);
        self
    }

    /// Add a close (×) button at the trailing edge. Click state surfaces
    /// via `HeaderResponse.close_clicked`.
    pub fn closable(mut self, on: bool) -> Self {
        self.closable = on;
        self
    }

    /// Caller-drawn content immediately AFTER the title (LTR flow). Useful
    /// for status pills, counts, "(12 active)" suffixes.
    pub fn leading<F>(mut self, f: F) -> Self
    where F: FnOnce(&mut Ui, &dyn ComponentTheme) + 'a {
        self.leading = Some(Box::new(f));
        self
    }

    /// Caller-drawn content at the trailing edge, to the LEFT of the
    /// close button. Action buttons, menu triggers, etc.
    pub fn trailing<F>(mut self, f: F) -> Self
    where F: FnOnce(&mut Ui, &dyn ComponentTheme) + 'a {
        self.trailing = Some(Box::new(f));
        self
    }

    pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> HeaderResponse {
        let h = self.variant.height();
        let font_size = self.variant.font_size();
        let pad_x = st::gap_md();

        let avail_w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(avail_w, h), Sense::hover());

        let painter = ui.painter_at(rect);
        let mut out = HeaderResponse { rect, response: Some(resp), close_clicked: false };

        let mut left_cursor = rect.left() + pad_x;
        let right_edge = rect.right() - pad_x;
        let cy = rect.center().y;

        // ── Leading icon ──
        if let Some(icon) = self.leading_icon {
            let icon_font = FontId::proportional(font_size + 2.0);
            let g = painter.layout_no_wrap(icon.to_string(), icon_font.clone(), theme.dim());
            painter.text(
                egui::pos2(left_cursor, cy),
                egui::Align2::LEFT_CENTER,
                icon,
                icon_font,
                theme.text(),
            );
            left_cursor += g.size().x + st::gap_sm();
        }

        // ── Title (uppercase for Panel/Section, mixed-case for Dialog) ──
        let title_text = if self.variant.upper() {
            self.title.to_uppercase()
        } else {
            self.title.to_string()
        };
        let title_font = FontId::monospace(font_size);
        let title_color = theme.text();
        let g = painter.layout_no_wrap(title_text.clone(), title_font.clone(), title_color);
        painter.text(
            egui::pos2(left_cursor, cy),
            egui::Align2::LEFT_CENTER,
            &title_text,
            title_font,
            title_color,
        );
        left_cursor += g.size().x + st::gap_md();

        // ── Subtitle (muted, right after title) ──
        if let Some(sub) = self.subtitle {
            let sub_font = FontId::monospace(st::font_xs());
            painter.text(
                egui::pos2(left_cursor, cy),
                egui::Align2::LEFT_CENTER,
                sub,
                sub_font,
                st::color_muted(theme.dim()),
            );
        }

        // ── Trailing: caller content, then close button (RTL) ──
        let close_w = if self.closable { 22.0 } else { 0.0 };

        if self.closable {
            let close_rect = egui::Rect::from_min_size(
                egui::pos2(right_edge - close_w + 2.0, rect.top() + (h - 18.0) / 2.0),
                Vec2::new(18.0, 18.0),
            );
            let close_resp = ui.allocate_rect(close_rect, Sense::click());
            st::cursor::clickable(ui, &close_resp);
            let fg = if close_resp.hovered() { theme.text() } else { st::color_muted(theme.dim()) };
            painter.text(
                close_rect.center(),
                egui::Align2::CENTER_CENTER,
                "\u{00D7}",
                FontId::monospace(font_size + 2.0),
                fg,
            );
            if close_resp.clicked() {
                out.close_clicked = true;
            }
        }

        if let Some(f) = self.trailing {
            // Hand the caller a child UI that sits between left_cursor and
            // (right_edge - close_w), laid out RTL so they can `add` from
            // the close button leftward.
            let trailing_rect = egui::Rect::from_min_max(
                egui::pos2(left_cursor, rect.top()),
                egui::pos2(right_edge - close_w, rect.bottom()),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(trailing_rect)
                    .layout(Layout::right_to_left(Align::Center)),
            );
            f(&mut child, theme);
        }

        if let Some(f) = self.leading {
            // Re-allocate the trailing-of-title slot left-to-right.
            let leading_rect = egui::Rect::from_min_max(
                egui::pos2(left_cursor, rect.top()),
                egui::pos2(right_edge - close_w, rect.bottom()),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(leading_rect)
                    .layout(Layout::left_to_right(Align::Center)),
            );
            f(&mut child, theme);
        }

        // ── Bottom rule (Panel only) ──
        if self.variant.bottom_rule() {
            let y = rect.bottom() - 0.5;
            painter.line_segment(
                [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                Stroke::new(st::stroke_thin(), st::color_alpha(theme.border(), st::alpha_muted())),
            );
        }

        let _ = CornerRadius::default(); // silence unused import if shape paths change later
        let _ = StrokeKind::Inside;
        out
    }
}
