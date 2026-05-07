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

use egui::{Color32, CornerRadius, FontId, Pos2, Response, Sense, Stroke, StrokeKind, Ui, Vec2, Widget};

use super::theme::ComponentTheme;
use crate::chart::renderer::ui::style as st;
use crate::ui_kit::icons::Icon;

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
        let color = match self.variant {
            AlertVariant::Info => theme.accent(),
            AlertVariant::Success => theme.bull(),
            AlertVariant::Warning => theme.warn(),
            AlertVariant::Error => theme.bear(),
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
        let content_left_offset = pad + icon_size + gap;
        let close_reserve = if self.closable { close_size + gap + pad } else { pad };
        let text_max_w = (avail_w - content_left_offset - close_reserve).max(40.0);

        let title_font = FontId::proportional(st::font_sm());
        let body_font = FontId::proportional(st::font_sm());

        let text_color = theme.text();
        let dim_color = theme.dim();

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
            let painter = ui.painter_at(rect);
            let cr = CornerRadius::same(6);
            painter.rect_filled(rect, cr, st::color_alpha(color, 32));
            painter.rect_stroke(rect, cr, Stroke::new(1.0, st::color_alpha(color, 200)), StrokeKind::Inside);

            // Leading icon
            let icon_center = Pos2::new(rect.left() + pad + icon_size * 0.5, rect.top() + pad + icon_size * 0.5);
            painter.text(
                icon_center,
                egui::Align2::CENTER_CENTER,
                icon,
                FontId::proportional(icon_size),
                color,
            );

            // Title + body
            let text_x = rect.left() + content_left_offset;
            let mut y = rect.top() + pad;
            if let Some(g) = title_galley {
                painter.galley(Pos2::new(text_x, y), g, text_color);
                y += title_h + title_gap;
            }
            painter.galley(Pos2::new(text_x, y), body_galley, dim_color);

            // Close button
            if self.closable {
                let close_center = Pos2::new(rect.right() - pad - close_size * 0.5, rect.top() + pad + close_size * 0.5);
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
                    FontId::proportional(close_size),
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
        let theme = &crate::chart_renderer::gpu::THEMES[0];
        self.show(ui, theme).response
    }
}
