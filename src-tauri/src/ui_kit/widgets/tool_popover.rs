//! `ToolPopover` — anchored popover companion to `ToolOverlay`.
//!
//! Same chrome principles (themed border, consistent body padding, optional
//! header), different positioning + dismissal model:
//!
//! | | ToolOverlay | ToolPopover |
//! | --- | --- | --- |
//! | Position | Draggable window (egui or host-managed) | Anchored at a fixed `Pos2` |
//! | Dismiss | Close button in header | Click-outside or Esc |
//! | Scrim | None | None (light, transient) |
//! | Use case | Long-running editors (indicator editor, order ticket) | Quick pickers (template popup, option quick-pick, search) |
//!
//! ## Quick start
//!
//! ```ignore
//! let resp = ToolPopover::new()
//!     .id("template_picker")
//!     .width(220.0)
//!     .pos(anchor_pos)                  // host-controlled anchor
//!     .title("TEMPLATES")               // optional
//!     .show(ctx, &theme, |ui| {
//!         // popover body
//!     });
//! if resp.dismissed { /* host closes */ }
//! ```

use egui::{Color32, Context, CornerRadius, Pos2, Sense, Stroke, Ui};

use super::theme::ComponentTheme;
use crate::ui_kit::sx::{palette_ct, Tone};
use crate::ui_kit::tokens as st;
use crate::ui_kit::text_style::TextStyle;

#[derive(Default)]
pub struct ToolPopoverResponse {
    /// `true` when the user clicked outside the popover or pressed Esc.
    /// Host should clear its "popover open" flag in response.
    pub dismissed: bool,
}

#[must_use = "ToolPopover does nothing until `.show(ctx, theme, body)` is called"]
pub struct ToolPopover<'a> {
    id:           &'a str,
    width:        f32,
    pos:          Pos2,
    title:        Option<&'a str>,
    body_pad_x:   f32,
    body_pad_y:   f32,
    dismiss_on_outside: bool,
    dismiss_on_esc: bool,
    footer:       Option<Box<dyn FnOnce(&mut Ui) + 'a>>,
}

impl<'a> ToolPopover<'a> {
    pub fn new() -> Self {
        Self {
            id: "tool_popover",
            width: 240.0,
            pos: Pos2::new(0.0, 0.0),
            title: None,
            body_pad_x: 12.0,
            body_pad_y: 8.0,
            dismiss_on_outside: true,
            dismiss_on_esc: true,
            footer: None,
        }
    }

    /// Stable egui id — required if multiple popovers can coexist.
    pub fn id(mut self, id: &'a str) -> Self { self.id = id; self }

    /// Popover width. Default 240 px. Height auto-fits content.
    pub fn width(mut self, w: f32) -> Self { self.width = w; self }

    /// Anchor position — the popover's top-left corner. Caller computes
    /// (typically based on the triggering button's rect).
    pub fn pos(mut self, p: Pos2) -> Self { self.pos = p; self }

    /// Optional title rendered as a small section-label header (no close
    /// button — popover is dismissed via click-outside or Esc).
    pub fn title(mut self, t: &'a str) -> Self { self.title = Some(t); self }

    /// Override body padding. Defaults: 12 H / 8 V (tighter than
    /// ToolOverlay since popovers are typically more compact).
    pub fn body_padding(mut self, x: f32, y: f32) -> Self {
        self.body_pad_x = x; self.body_pad_y = y; self
    }

    /// Disable the click-outside-dismisses behavior. Default: enabled.
    /// Use when you want explicit close (e.g. via an action inside the body).
    pub fn dismiss_on_outside(mut self, on: bool) -> Self {
        self.dismiss_on_outside = on; self
    }

    /// Disable Esc-to-dismiss. Default: enabled.
    pub fn dismiss_on_esc(mut self, on: bool) -> Self {
        self.dismiss_on_esc = on; self
    }

    /// Footer slot with a hairline divider above. Used for action rows
    /// (Save / Cancel) or summary captions.
    pub fn footer(mut self, body: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.footer = Some(Box::new(body));
        self
    }

    /// Render. Returns whether the popover was dismissed this frame.
    pub fn show<T: ComponentTheme>(
        self,
        ctx: &Context,
        theme: &T,
        body: impl FnOnce(&mut Ui),
    ) -> ToolPopoverResponse {
        let mut response = ToolPopoverResponse::default();
        let radius = st::r_sm_cr();
        let bg = palette_ct(theme).base(Tone::Surface);
        let border = palette_ct(theme).base(Tone::Border);
        let title  = self.title;
        let body_pad_x = self.body_pad_x;
        let body_pad_y = self.body_pad_y;
        let mut footer = self.footer;
        let text_color = palette_ct(theme).base(Tone::Text);
        let dim_color  = palette_ct(theme).base(Tone::Dim);
        let border_col = border;

        let area = egui::Area::new(egui::Id::new(self.id))
            .fixed_pos(self.pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(bg)
                    .stroke(Stroke::new(st::stroke_std(), border))
                    .corner_radius(radius)
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 14,
                        spread: 0,
                        color: color_alpha(theme.shadow_color(), crate::ui_kit::style::alpha_strong()),
                    })
                    .inner_margin(egui::Margin::ZERO)
                    .show(ui, |ui| {
                        ui.set_min_width(self.width);

                        // ── Optional title strip ─────────────────────────
                        if let Some(t) = title {
                            const TITLE_H: f32 = 24.0;
                            let avail_w = ui.available_width();
                            let (title_rect, _resp) = ui.allocate_exact_size(
                                egui::vec2(avail_w, TITLE_H),
                                Sense::hover(),
                            );
                            // REUSES `section.header` for the FILL only.
                            //
                            // The radius stays asymmetric on purpose — this
                            // band sits on top of a rounded panel, so its top
                            // corners follow the panel and its bottom corners
                            // must stay square. `resolve_control_chrome`
                            // returns a symmetric radius, so taking it here
                            // would round the bottom edge into the body.
                            let hdr_default = color_alpha(border_col, st::alpha_tint());
                            let (_, hdr_fill, _) =
                                crate::ui_kit::widgets::theme::resolve_control_chrome(
                                    ui.ctx(), theme, "section.header",
                                    0.0, hdr_default, hdr_default, 0.0,
                                );
                            ui.painter().rect_filled(
                                title_rect,
                                CornerRadius { nw: radius.nw, ne: radius.ne, sw: 0, se: 0 },
                                hdr_fill,
                            );
                            ui.painter().hline(
                                title_rect.x_range(),
                                title_rect.bottom(),
                                Stroke::new(st::stroke_thin(), color_alpha(border_col, st::alpha_muted())),
                            );
                            ui.painter().text(
                                egui::pos2(title_rect.left() + st::gap_md(), title_rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                t,
                                TextStyle::MonoSm.font_id_in(ui),
                                text_color,
                            );
                        }

                        // ── Body frame ───────────────────────────────────
                        egui::Frame::NONE
                            .inner_margin(egui::Margin {
                                left:   body_pad_x as i8,
                                right:  body_pad_x as i8,
                                top:    body_pad_y as i8,
                                bottom: body_pad_y as i8,
                            })
                            .show(ui, body);

                        // ── Optional footer ──────────────────────────────
                        if let Some(f) = footer.take() {
                            let avail = ui.available_rect_before_wrap();
                            ui.painter().hline(
                                avail.x_range(),
                                avail.top(),
                                Stroke::new(st::stroke_thin(), color_alpha(border_col, st::alpha_muted())),
                            );
                            egui::Frame::NONE
                                .inner_margin(egui::Margin {
                                    left:   body_pad_x as i8,
                                    right:  body_pad_x as i8,
                                    top:    st::gap_xs() as i8,
                                    bottom: st::gap_xs() as i8,
                                })
                                .show(ui, f);
                        }

                        let _ = dim_color;
                    });
            });

        // ── Dismissal detection ──────────────────────────────────────────
        let popover_rect = area.response.rect;
        if self.dismiss_on_outside {
            if ctx.input(|i| i.pointer.any_pressed()) {
                if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
                    if !popover_rect.contains(p) {
                        response.dismissed = true;
                    }
                }
            }
        }
        if self.dismiss_on_esc && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            response.dismissed = true;
        }

        response
    }
}

impl<'a> Default for ToolPopover<'a> {
    fn default() -> Self { Self::new() }
}

#[inline]
fn color_alpha(c: Color32, a: u8) -> Color32 {
    crate::ui_kit::style::color_alpha(c, a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_kit::widgets::theme::PortableTheme;

    #[test]
    fn builder_defaults() {
        let p = ToolPopover::new();
        assert_eq!(p.width, 240.0);
        assert!(p.title.is_none());
        assert!(p.dismiss_on_outside);
        assert!(p.dismiss_on_esc);
    }

    #[test]
    fn builder_chain() {
        let p = ToolPopover::new()
            .id("the_id")
            .width(300.0)
            .pos(Pos2::new(100.0, 100.0))
            .title("Hello")
            .dismiss_on_outside(false)
            .dismiss_on_esc(false);
        assert_eq!(p.id, "the_id");
        assert_eq!(p.width, 300.0);
        assert_eq!(p.title, Some("Hello"));
        assert!(!p.dismiss_on_outside);
        assert!(!p.dismiss_on_esc);
    }

    #[test]
    fn show_returns_default_response_on_first_frame() {
        let ctx = Context::default();
        let theme = PortableTheme::default();
        let mut response = ToolPopoverResponse::default();
        let _ = ctx.run(Default::default(), |ctx| {
            response = ToolPopover::new()
                .id("test_popover")
                .pos(Pos2::new(100.0, 100.0))
                .show(ctx, &theme, |ui| {
                    ui.label("hello");
                });
        });
        // No clicks fired, so popover is not dismissed on first frame.
        assert!(!response.dismissed);
    }
}
