//! Sheet (drawer) — full-height/width overlay sliding in from a screen edge.
//!
//! Use cases:
//!   - Trade ticket slide-in from right
//!   - Settings drawer from left
//!   - Bottom sheet for mobile-style action menus (rare on desktop trading
//!     terminals, but still occasionally useful)
//!
//! API:
//!   let mut open = false;
//!   if ui.add(Button::new("Open ticket")).clicked() { open = true; }
//!
//!   Sheet::new()
//!     .open(&mut open)
//!     .side(SheetSide::Right)
//!     .size(SheetSize::Fixed(420.0))
//!     .show(ui, theme, |ui| {
//!         Label::heading("Trade Ticket").show(ui, theme);
//!         // ... ticket form ...
//!     });
//!
//! Painting model:
//!   * Optional fullscreen scrim (Area, Background order) when `modal == true`.
//!     Scrim alpha animates 0 → 120 over `motion::MED`. A click on the scrim
//!     with `close_on_backdrop` triggers close.
//!   * The sheet panel itself is a foreground Area pinned to one screen edge
//!     and translated in from off-screen by `(1 - t) * resolved_size` along
//!     the perpendicular axis where `t = motion::ease_bool(...)`.
//!   * When `t < 0.001` and not open, the widget renders nothing and returns
//!     `None`, clearing per-id memory implicitly.

use egui::{Color32, Id, Key, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

use super::motion;
use super::theme::ComponentTheme;
use super::Button;
use super::tokens::{Size as KitSize, Variant};
use crate::ui_kit::icons::Icon;
use crate::chart::renderer::ui::style::{gap_md, stroke_thin};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SheetSide {
    Left,
    #[default]
    Right,
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug)]
pub enum SheetSize {
    /// Exact pixels along the side axis (width for L/R, height for T/B).
    Fixed(f32),
    /// Fraction of screen along the side axis (clamped 0.0..=1.0).
    Percent(f32),
    /// Measure content one frame, clamp to 30%..80% of the screen axis.
    Auto,
}

impl Default for SheetSize {
    fn default() -> Self {
        SheetSize::Fixed(380.0)
    }
}

#[must_use = "Sheet does nothing until `.show()` is called"]
pub struct Sheet<'a> {
    open: Option<&'a mut bool>,
    side: SheetSide,
    size: SheetSize,
    close_on_backdrop: bool,
    close_on_escape: bool,
    modal: bool,
    title: Option<String>,
    id: Option<Id>,
}

impl<'a> Sheet<'a> {
    pub fn new() -> Self {
        Self {
            open: None,
            side: SheetSide::default(),
            size: SheetSize::default(),
            close_on_backdrop: true,
            close_on_escape: true,
            modal: true,
            title: None,
            id: None,
        }
    }

    pub fn open(mut self, state: &'a mut bool) -> Self {
        self.open = Some(state);
        self
    }
    pub fn side(mut self, side: SheetSide) -> Self {
        self.side = side;
        self
    }
    pub fn size(mut self, s: SheetSize) -> Self {
        self.size = s;
        self
    }
    pub fn close_on_backdrop(mut self, v: bool) -> Self {
        self.close_on_backdrop = v;
        self
    }
    pub fn close_on_escape(mut self, v: bool) -> Self {
        self.close_on_escape = v;
        self
    }
    pub fn modal(mut self, v: bool) -> Self {
        self.modal = v;
        self
    }
    pub fn title(mut self, text: impl Into<String>) -> Self {
        self.title = Some(text.into());
        self
    }
    pub fn id(mut self, id: impl std::hash::Hash) -> Self {
        self.id = Some(Id::new(id));
        self
    }

    pub fn show<R>(
        self,
        ui: &mut Ui,
        theme: &dyn ComponentTheme,
        add_contents: impl FnOnce(&mut Ui) -> R,
    ) -> Option<R> {
        let open_state = self
            .open
            .expect("Sheet::show requires .open(&mut bool)");
        let ctx = ui.ctx().clone();

        // Stable id keyed off side + an optional caller id, so sheets on
        // different edges don't share animation state.
        let base_id = self
            .id
            .unwrap_or_else(|| Id::new(("apex_sheet", self.side as u32)));

        // Drive the slide / scrim animation off the user-visible boolean.
        let anim_id = base_id.with("anim");
        let t = motion::ease_bool(&ctx, anim_id, *open_state, motion::MED);

        // Fully closed and finished animating out — render nothing.
        if !*open_state && t < 0.001 {
            return None;
        }

        let screen = ctx.screen_rect();

        // Resolve the size along the side axis.
        let axis_extent = match self.side {
            SheetSide::Left | SheetSide::Right => screen.width(),
            SheetSide::Top | SheetSide::Bottom => screen.height(),
        };
        let resolved_size = match self.size {
            SheetSize::Fixed(px) => px.max(0.0),
            SheetSize::Percent(p) => p.clamp(0.0, 1.0) * axis_extent,
            SheetSize::Auto => {
                // One-frame lag: read prior measurement from memory, fall back
                // to a sensible default.
                let measured: f32 = ctx
                    .memory(|m| m.data.get_temp(base_id.with("auto_size")))
                    .unwrap_or(380.0);
                measured.clamp(axis_extent * 0.30, axis_extent * 0.80)
            }
        };

        // ---------------- Backdrop / scrim ----------------
        let mut backdrop_close = false;
        if self.modal {
            let scrim_alpha = (120.0 * t) as u8;
            let scrim_color = Color32::from_black_alpha(scrim_alpha);
            let scrim_id = base_id.with("scrim");
            let _ = egui::Area::new(scrim_id)
                .order(egui::Order::Background)
                .fixed_pos(screen.min)
                .interactable(true)
                .show(&ctx, |ui| {
                    let (rect, resp) =
                        ui.allocate_exact_size(screen.size(), Sense::click_and_drag());
                    ui.painter().rect_filled(rect, 0.0, scrim_color);
                    if resp.clicked() && self.close_on_backdrop && *open_state {
                        backdrop_close = true;
                    }
                });
        }

        // ---------------- Sheet panel rect ----------------
        // Rect when fully open (t = 1).
        let open_rect = match self.side {
            SheetSide::Left => Rect::from_min_size(
                screen.min,
                Vec2::new(resolved_size, screen.height()),
            ),
            SheetSide::Right => Rect::from_min_size(
                egui::pos2(screen.max.x - resolved_size, screen.min.y),
                Vec2::new(resolved_size, screen.height()),
            ),
            SheetSide::Top => Rect::from_min_size(
                screen.min,
                Vec2::new(screen.width(), resolved_size),
            ),
            SheetSide::Bottom => Rect::from_min_size(
                egui::pos2(screen.min.x, screen.max.y - resolved_size),
                Vec2::new(screen.width(), resolved_size),
            ),
        };

        // Translate offset for slide-in: (1 - t) * resolved_size along the
        // perpendicular axis pointing off-screen.
        let slide_offset = (1.0 - t) * resolved_size;
        let translated_min = match self.side {
            SheetSide::Left => egui::pos2(open_rect.min.x - slide_offset, open_rect.min.y),
            SheetSide::Right => egui::pos2(open_rect.min.x + slide_offset, open_rect.min.y),
            SheetSide::Top => egui::pos2(open_rect.min.x, open_rect.min.y - slide_offset),
            SheetSide::Bottom => egui::pos2(open_rect.min.x, open_rect.min.y + slide_offset),
        };
        let panel_rect = Rect::from_min_size(translated_min, open_rect.size());

        // ---------------- Sheet content ----------------
        let bg = theme.bg();
        let border = theme.border();
        let title = self.title.clone();
        let side = self.side;
        let mut header_close = false;
        let mut result: Option<R> = None;
        let mut measured_axis: f32 = 0.0;

        let area_id = base_id.with("panel");
        let _ = egui::Area::new(area_id)
            .order(egui::Order::Foreground)
            .fixed_pos(panel_rect.min)
            .interactable(true)
            .show(&ctx, |ui| {
                // Drop shadow on the inner edge of the sheet (the edge that
                // faces the rest of the app). t naturally fades it during
                // the slide animation since alpha is multiplied here.
                let shadow_spec = match side {
                    SheetSide::Right => super::ShadowSpec::lg().offset(-8.0, 0.0),
                    SheetSide::Left => super::ShadowSpec::lg().offset(8.0, 0.0),
                    SheetSide::Top => super::ShadowSpec::lg().offset(0.0, 8.0),
                    SheetSide::Bottom => super::ShadowSpec::lg().offset(0.0, -8.0),
                };
                let base_a = shadow_spec.color.a() as f32;
                let a = (base_a * t).clamp(0.0, 255.0) as u8;
                super::paint_shadow_gpu(
                    ui.painter(),
                    panel_rect,
                    shadow_spec.color(Color32::from_black_alpha(a)),
                );
                // Background fill for the panel rect.
                ui.painter().rect_filled(panel_rect, 0.0, bg);

                // 1px border on the inner edge only.
                let inner_edge: [egui::Pos2; 2] = match side {
                    SheetSide::Left => [
                        egui::pos2(panel_rect.max.x, panel_rect.min.y),
                        egui::pos2(panel_rect.max.x, panel_rect.max.y),
                    ],
                    SheetSide::Right => [
                        egui::pos2(panel_rect.min.x, panel_rect.min.y),
                        egui::pos2(panel_rect.min.x, panel_rect.max.y),
                    ],
                    SheetSide::Top => [
                        egui::pos2(panel_rect.min.x, panel_rect.max.y),
                        egui::pos2(panel_rect.max.x, panel_rect.max.y),
                    ],
                    SheetSide::Bottom => [
                        egui::pos2(panel_rect.min.x, panel_rect.min.y),
                        egui::pos2(panel_rect.max.x, panel_rect.min.y),
                    ],
                };
                ui.painter()
                    .line_segment(inner_edge, Stroke::new(stroke_thin().max(1.0), border));
                let _ = StrokeKind::Outside; // keep import used path readable

                // Constrain child UI to the panel rect with inner padding.
                let pad = gap_md();
                let inner_rect = panel_rect.shrink(pad);
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(inner_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                child.set_clip_rect(panel_rect);

                // Optional title row + close button.
                if let Some(t_str) = title.as_ref() {
                    child.horizontal(|ui| {
                        // Place the close button on the side opposite the
                        // sheet's anchor edge for L/R sheets; for T/B sheets
                        // it goes on the right by convention.
                        let close_first = matches!(side, SheetSide::Left);
                        if close_first {
                            if Button::icon(Icon::X)
                                .variant(Variant::Ghost)
                                .size(KitSize::Sm)
                                .show(ui, theme)
                                .clicked()
                            {
                                header_close = true;
                            }
                            ui.add_space(gap_md() * 0.5);
                        }
                        super::Label::heading(t_str.clone()).show(ui, theme);
                        if !close_first {
                            // Push close button to far edge.
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if Button::icon(Icon::X)
                                        .variant(Variant::Ghost)
                                        .size(KitSize::Sm)
                                        .show(ui, theme)
                                        .clicked()
                                    {
                                        header_close = true;
                                    }
                                },
                            );
                        }
                    });
                    child.add_space(gap_md() * 0.5);
                    super::Separator::horizontal().show(&mut child, theme);
                    child.add_space(gap_md() * 0.5);
                }

                // Body.
                let r = add_contents(&mut child);
                result = Some(r);

                // Measure used extent for SheetSize::Auto on next frame.
                let used = child.min_rect();
                measured_axis = match side {
                    SheetSide::Left | SheetSide::Right => used.width() + pad * 2.0,
                    SheetSide::Top | SheetSide::Bottom => used.height() + pad * 2.0,
                };
            });

        // Persist measurement for Auto sizing.
        if matches!(self.size, SheetSize::Auto) && measured_axis > 0.0 {
            ctx.memory_mut(|m| {
                m.data.insert_temp(base_id.with("auto_size"), measured_axis)
            });
        }

        // ---------------- Close handling ----------------
        if backdrop_close {
            *open_state = false;
        }
        if header_close {
            *open_state = false;
        }
        if self.close_on_escape && *open_state && ctx.input(|i| i.key_pressed(Key::Escape)) {
            *open_state = false;
        }

        result
    }
}

impl<'a> Default for Sheet<'a> {
    fn default() -> Self {
        Self::new()
    }
}
