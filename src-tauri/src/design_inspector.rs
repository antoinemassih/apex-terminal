//! Inspector panel — tactile design controls for live token editing.
//!
//! F12 toggles the inspector. It shows a categorized view of all design tokens
//! with sliders, color pickers, and drag values. Changes are immediately applied
//! (egui repaints next frame) and can be saved back to design.toml.
//!
//! Only compiled when the `design-mode` feature is enabled.

#![cfg(feature = "design-mode")]

use egui::{Color32, RichText, Ui, Stroke};
use crate::design_tokens::*;
use std::path::PathBuf;

/// Inspector state — persists across frames.
pub struct Inspector {
    pub open: bool,
    pub toml_path: PathBuf,
    /// Which category is expanded
    pub category: Category,
    /// Dirty flag — tokens changed since last save
    pub dirty: bool,
    /// Status message
    pub status: String,
    /// Search filter
    pub filter: String,
    /// Inspect mode — hover to highlight elements, click to select
    pub inspect_mode: bool,
    /// Currently hovered element family name
    pub hovered_family: Option<&'static str>,
    /// Locked selection (clicked element)
    pub selected_family: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Category {
    Font,
    Spacing,
    Radius,
    Stroke,
    Alpha,
    Shadow,
    Colors,
    Toolbar,
    Panel,
    Dialog,
    Button,
    Card,
    Badge,
    Tab,
    Table,
    Chart,
    Watchlist,
    OrderEntry,
    PaneHeader,
    Segmented,
    IconButton,
    Form,
    SplitDivider,
    Tooltip,
    Separator,
}

impl Category {
    const ALL: &[Category] = &[
        Category::Font, Category::Spacing, Category::Radius, Category::Stroke,
        Category::Alpha, Category::Shadow, Category::Colors,
        Category::Toolbar, Category::Panel, Category::Dialog, Category::Button,
        Category::Card, Category::Badge, Category::Tab, Category::Table,
        Category::Chart, Category::Watchlist, Category::OrderEntry,
        Category::PaneHeader, Category::Segmented, Category::IconButton,
        Category::Form, Category::SplitDivider, Category::Tooltip, Category::Separator,
    ];

    fn label(self) -> &'static str {
        match self {
            Category::Font => "Font Sizes",
            Category::Spacing => "Spacing",
            Category::Radius => "Corner Radii",
            Category::Stroke => "Stroke Widths",
            Category::Alpha => "Alpha / Opacity",
            Category::Shadow => "Shadows",
            Category::Colors => "Semantic Colors",
            Category::Toolbar => "Toolbar",
            Category::Panel => "Panels",
            Category::Dialog => "Dialogs",
            Category::Button => "Buttons",
            Category::Card => "Cards",
            Category::Badge => "Badges",
            Category::Tab => "Tabs",
            Category::Table => "Tables / Rows",
            Category::Chart => "Chart",
            Category::Watchlist => "Watchlist",
            Category::OrderEntry => "Order Entry",
            Category::PaneHeader => "Pane Header",
            Category::Segmented => "Segmented Control",
            Category::IconButton => "Icon Buttons",
            Category::Form => "Forms",
            Category::SplitDivider => "Split Divider",
            Category::Tooltip => "Tooltips",
            Category::Separator => "Separators",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Category::Font => "Aa",
            Category::Spacing => "[ ]",
            Category::Radius => "( )",
            Category::Stroke => "---",
            Category::Alpha => "%%",
            Category::Shadow => "//",
            Category::Colors => "",
            _ => "",
        }
    }
}

impl Inspector {
    pub fn new(toml_path: PathBuf) -> Self {
        Self {
            open: false,
            toml_path,
            category: Category::Font,
            dirty: false,
            status: String::new(),
            filter: String::new(),
            inspect_mode: false,
            hovered_family: None,
            selected_family: None,
        }
    }

    /// Toggle with F12.
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    /// Render the inspector panel. Returns true if tokens were modified this frame.
    pub fn show(&mut self, ctx: &egui::Context, tokens: &mut DesignTokens) -> bool {
        if !self.open { return false; }

        // Sync inspect mode to global flag
        crate::design_tokens::set_inspect_mode(self.inspect_mode);
        if !self.inspect_mode {
            ctx.set_debug_on_hover(false);
        }

        // ── Inspect mode overlay — draw BEFORE the panel so it appears on the chart ──
        if self.inspect_mode {
            let hits = crate::design_tokens::get_hits();
            let pointer = ctx.input(|i| i.pointer.hover_pos());
            let clicked = ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary));
            self.hovered_family = None;

            // Debug: show hit count
            {
                static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if c % 120 == 0 { // log every ~2 seconds at 60fps
                    eprintln!("[inspect] {} hits registered this frame", hits.len());
                }
            }

            // Change cursor to crosshair
            ctx.set_cursor_icon(egui::CursorIcon::Crosshair);

            // Also enable egui's built-in debug overlay for ALL widgets
            ctx.set_debug_on_hover(true);

            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("inspect_overlay")));

            // Find the smallest hit under the cursor (most specific element)
            let mut best_hover: Option<&ElementHit> = None;
            let mut best_area = f32::MAX;

            for h in &hits {
                let rect = egui::Rect::from_min_size(
                    egui::pos2(h.rect[0], h.rect[1]),
                    egui::vec2(h.rect[2], h.rect[3]));

                let is_selected = self.selected_family == Some(h.family);

                // Draw subtle outline on ALL inspectable elements
                painter.rect_stroke(rect, 2.0,
                    Stroke::new(0.5, Color32::from_rgba_unmultiplied(203, 166, 247, 40)),
                    egui::StrokeKind::Outside);

                // Highlight selected elements
                if is_selected {
                    painter.rect_filled(rect, 2.0, Color32::from_rgba_unmultiplied(166, 227, 161, 30));
                    painter.rect_stroke(rect, 2.0, Stroke::new(1.5, Color32::from_rgba_unmultiplied(166, 227, 161, 160)), egui::StrokeKind::Outside);
                }

                // Track smallest hovered element
                if let Some(p) = pointer {
                    if rect.contains(p) {
                        let area = rect.width() * rect.height();
                        if area < best_area {
                            best_area = area;
                            best_hover = Some(h);
                        }
                    }
                }
            }

            // Highlight the hovered element
            if let Some(h) = best_hover {
                let rect = egui::Rect::from_min_size(
                    egui::pos2(h.rect[0], h.rect[1]),
                    egui::vec2(h.rect[2], h.rect[3]));
                painter.rect_filled(rect, 2.0, Color32::from_rgba_unmultiplied(203, 166, 247, 35));
                painter.rect_stroke(rect, 2.0, Stroke::new(2.0, Color32::from_rgba_unmultiplied(203, 166, 247, 200)), egui::StrokeKind::Outside);

                // Family label above the element
                let label_pos = egui::pos2(rect.left(), rect.top() - 2.0);
                // Background for readability
                let galley = painter.layout_no_wrap(h.family.to_string(), egui::FontId::monospace(10.0), Color32::WHITE);
                let label_rect = egui::Rect::from_min_size(
                    egui::pos2(label_pos.x - 2.0, label_pos.y - galley.size().y - 2.0),
                    egui::vec2(galley.size().x + 4.0, galley.size().y + 4.0));
                painter.rect_filled(label_rect, 2.0, Color32::from_rgba_unmultiplied(20, 20, 30, 230));
                painter.text(egui::pos2(label_pos.x, label_pos.y - 1.0), egui::Align2::LEFT_BOTTOM,
                    h.family, egui::FontId::monospace(10.0), Color32::from_rgb(203, 166, 247));

                self.hovered_family = Some(h.family);

                // Click to select
                if clicked {
                    self.selected_family = Some(h.family);
                    if let Some(cat) = category_from_name(h.category) {
                        self.category = cat;
                    }
                    self.status = format!("Selected: {}", h.family);
                }
            }
        }

        // Clear hits at end of frame (they get populated again next frame by style.rs)
        crate::design_tokens::clear_hits();

        let mut modified = false;

        egui::SidePanel::right("design_inspector")
            .min_width(320.0)
            .max_width(420.0)
            .default_width(360.0)
            .frame(egui::Frame::NONE
                .fill(Color32::from_rgb(18, 18, 24))
                .stroke(Stroke::new(1.0, Color32::from_rgb(40, 42, 54)))
                .inner_margin(0.0))
            .show(ctx, |ui| {
                // Header
                egui::Frame::NONE
                    .fill(Color32::from_rgb(14, 14, 20))
                    .inner_margin(egui::Margin { left: 12, right: 12, top: 10, bottom: 10 })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("DESIGN INSPECTOR")
                                .monospace().size(13.0).strong()
                                .color(Color32::from_rgb(203, 166, 247)));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add(egui::Button::new(
                                    RichText::new(if self.dirty { "SAVE" } else { "saved" })
                                        .monospace().size(10.0).strong()
                                        .color(if self.dirty { Color32::from_rgb(166, 227, 161) } else { Color32::from_rgb(100, 100, 110) }))
                                    .fill(if self.dirty { Color32::from_rgba_unmultiplied(166, 227, 161, 25) } else { Color32::TRANSPARENT })
                                    .stroke(Stroke::new(0.5, if self.dirty { Color32::from_rgba_unmultiplied(166, 227, 161, 80) } else { Color32::from_rgb(50, 50, 60) }))
                                    .corner_radius(3.0)
                                ).clicked() && self.dirty {
                                    match tokens.save(&self.toml_path) {
                                        Ok(_) => {
                                            self.dirty = false;
                                            self.status = "Saved!".to_string();
                                        }
                                        Err(e) => {
                                            self.status = format!("Save error: {e}");
                                        }
                                    }
                                }

                                // Reset button
                                if ui.add(egui::Button::new(
                                    RichText::new("RESET").monospace().size(10.0)
                                        .color(Color32::from_rgb(243, 139, 168)))
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::new(0.5, Color32::from_rgba_unmultiplied(243, 139, 168, 50)))
                                    .corner_radius(3.0)
                                ).clicked() {
                                    *tokens = DesignTokens::default();
                                    modified = true;
                                    self.dirty = true;
                                }
                            });
                        });

                        // Inspect mode toggle + selected element display
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            let inspect_color = if self.inspect_mode {
                                Color32::from_rgb(255, 191, 0)
                            } else {
                                Color32::from_rgb(100, 100, 110)
                            };
                            if ui.add(egui::Button::new(
                                RichText::new(if self.inspect_mode { "⊹ INSPECT ON" } else { "⊹ INSPECT" })
                                    .monospace().size(11.0).strong().color(inspect_color))
                                .fill(if self.inspect_mode { Color32::from_rgba_unmultiplied(255, 191, 0, 20) } else { Color32::TRANSPARENT })
                                .stroke(Stroke::new(0.5, Color32::from_rgba_unmultiplied(inspect_color.r(), inspect_color.g(), inspect_color.b(), 80)))
                                .corner_radius(3.0)
                            ).clicked() {
                                self.inspect_mode = !self.inspect_mode;
                                if !self.inspect_mode {
                                    self.selected_family = None;
                                    self.hovered_family = None;
                                }
                            }

                            // Show selected/hovered element
                            let display = self.selected_family.or(self.hovered_family);
                            if let Some(fam) = display {
                                ui.label(RichText::new(fam).monospace().size(10.0).strong()
                                    .color(Color32::from_rgb(166, 227, 161)));
                            }
                        });

                        // Search
                        ui.add_space(4.0);
                        ui.add(egui::TextEdit::singleline(&mut self.filter)
                            .hint_text("Filter tokens...")
                            .desired_width(ui.available_width())
                            .font(egui::FontId::monospace(10.0)));
                    });

                // Category tabs (vertical list)
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;

                    for &cat in Category::ALL {
                        let active = self.category == cat;
                        let resp = ui.add(
                            egui::Button::new(
                                RichText::new(cat.label()).monospace().size(11.0)
                                    .color(if active { Color32::from_rgb(203, 166, 247) } else { Color32::from_rgb(150, 150, 160) }))
                            .fill(if active { Color32::from_rgba_unmultiplied(203, 166, 247, 15) } else { Color32::TRANSPARENT })
                            .stroke(Stroke::NONE)
                            .corner_radius(0.0)
                            .min_size(egui::vec2(ui.available_width(), 28.0))
                        );
                        if resp.clicked() { self.category = cat; }

                        // Show expanded controls when active
                        if active {
                            egui::Frame::NONE
                                .fill(Color32::from_rgb(22, 22, 30))
                                .inner_margin(egui::Margin { left: 16, right: 12, top: 8, bottom: 8 })
                                .show(ui, |ui| {
                                    if self.render_category(ui, cat, tokens) {
                                        modified = true;
                                        self.dirty = true;
                                    }
                                });
                        }
                    }
                });

                // Status bar
                if !self.status.is_empty() {
                    egui::Frame::NONE
                        .fill(Color32::from_rgb(14, 14, 20))
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.label(RichText::new(&self.status).monospace().size(9.0)
                                .color(Color32::from_rgb(150, 150, 160)));
                        });
                }
            });

        modified
    }

    /// Render controls for a specific category. Returns true if any value changed.
    fn render_category(&self, ui: &mut Ui, cat: Category, tokens: &mut DesignTokens) -> bool {
        let mut changed = false;
        match cat {
            Category::Font => {
                changed |= drag_f32(ui, "xxs (7.0)", &mut tokens.font.xxs, 1.0..=20.0);
                changed |= drag_f32(ui, "xs (8.0)", &mut tokens.font.xs, 1.0..=20.0);
                changed |= drag_f32(ui, "sm_tight (9.0)", &mut tokens.font.sm_tight, 1.0..=20.0);
                changed |= drag_f32(ui, "sm (10.0)", &mut tokens.font.sm, 1.0..=20.0);
                changed |= drag_f32(ui, "md (11.0)", &mut tokens.font.md, 1.0..=20.0);
                changed |= drag_f32(ui, "input (12.0)", &mut tokens.font.input, 1.0..=20.0);
                changed |= drag_f32(ui, "lg (13.0)", &mut tokens.font.lg, 1.0..=24.0);
                changed |= drag_f32(ui, "xl (14.0)", &mut tokens.font.xl, 1.0..=24.0);
                changed |= drag_f32(ui, "xxl (15.0)", &mut tokens.font.xxl, 1.0..=30.0);
                changed |= drag_f32(ui, "display (28.0)", &mut tokens.font.display, 10.0..=60.0);
                changed |= drag_f32(ui, "display_lg (36.0)", &mut tokens.font.display_lg, 10.0..=80.0);
            }
            Category::Spacing => {
                changed |= drag_f32(ui, "xs (2.0)", &mut tokens.spacing.xs, 0.0..=20.0);
                changed |= drag_f32(ui, "sm (4.0)", &mut tokens.spacing.sm, 0.0..=20.0);
                changed |= drag_f32(ui, "md (6.0)", &mut tokens.spacing.md, 0.0..=20.0);
                changed |= drag_f32(ui, "lg (8.0)", &mut tokens.spacing.lg, 0.0..=30.0);
                changed |= drag_f32(ui, "xl (10.0)", &mut tokens.spacing.xl, 0.0..=30.0);
                changed |= drag_f32(ui, "xxl (12.0)", &mut tokens.spacing.xxl, 0.0..=30.0);
                changed |= drag_f32(ui, "xxxl (20.0)", &mut tokens.spacing.xxxl, 0.0..=50.0);
            }
            Category::Radius => {
                changed |= drag_f32(ui, "xs (2.0)", &mut tokens.radius.xs, 0.0..=20.0);
                changed |= drag_f32(ui, "sm (3.0)", &mut tokens.radius.sm, 0.0..=20.0);
                changed |= drag_f32(ui, "md (4.0)", &mut tokens.radius.md, 0.0..=20.0);
                changed |= drag_f32(ui, "lg (8.0)", &mut tokens.radius.lg, 0.0..=30.0);
            }
            Category::Stroke => {
                changed |= drag_f32(ui, "hair (0.3)", &mut tokens.stroke.hair, 0.0..=5.0);
                changed |= drag_f32(ui, "thin (0.5)", &mut tokens.stroke.thin, 0.0..=5.0);
                changed |= drag_f32(ui, "std (1.0)", &mut tokens.stroke.std, 0.0..=5.0);
                changed |= drag_f32(ui, "bold (1.5)", &mut tokens.stroke.bold, 0.0..=5.0);
                changed |= drag_f32(ui, "thick (2.0)", &mut tokens.stroke.thick, 0.0..=8.0);
                changed |= drag_f32(ui, "heavy (2.5)", &mut tokens.stroke.heavy, 0.0..=8.0);
                changed |= drag_f32(ui, "xheavy (5.0)", &mut tokens.stroke.xheavy, 0.0..=10.0);
            }
            Category::Alpha => {
                changed |= drag_u8(ui, "faint (10)", &mut tokens.alpha.faint);
                changed |= drag_u8(ui, "ghost (15)", &mut tokens.alpha.ghost);
                changed |= drag_u8(ui, "soft (20)", &mut tokens.alpha.soft);
                changed |= drag_u8(ui, "subtle (25)", &mut tokens.alpha.subtle);
                changed |= drag_u8(ui, "tint (30)", &mut tokens.alpha.tint);
                changed |= drag_u8(ui, "muted (40)", &mut tokens.alpha.muted);
                changed |= drag_u8(ui, "line (50)", &mut tokens.alpha.line);
                changed |= drag_u8(ui, "dim (60)", &mut tokens.alpha.dim);
                changed |= drag_u8(ui, "strong (80)", &mut tokens.alpha.strong);
                changed |= drag_u8(ui, "active (100)", &mut tokens.alpha.active);
                changed |= drag_u8(ui, "heavy (120)", &mut tokens.alpha.heavy);
            }
            Category::Shadow => {
                changed |= drag_f32(ui, "offset", &mut tokens.shadow.offset, 0.0..=20.0);
                changed |= drag_u8(ui, "alpha", &mut tokens.shadow.alpha);
                changed |= drag_f32(ui, "spread", &mut tokens.shadow.spread, 0.0..=20.0);
            }
            Category::Colors => {
                changed |= color_edit(ui, "text_primary", &mut tokens.color.text_primary);
                changed |= color_edit(ui, "text_secondary", &mut tokens.color.text_secondary);
                changed |= color_edit(ui, "text_dim", &mut tokens.color.text_dim);
                changed |= color_edit(ui, "text_on_accent", &mut tokens.color.text_on_accent);
                ui.add_space(4.0);
                changed |= color_edit(ui, "amber", &mut tokens.color.amber);
                changed |= color_edit(ui, "earnings", &mut tokens.color.earnings);
                changed |= color_edit(ui, "paper_orange", &mut tokens.color.paper_orange);
                changed |= color_edit(ui, "live_green", &mut tokens.color.live_green);
                changed |= color_edit(ui, "danger", &mut tokens.color.danger);
                changed |= color_edit(ui, "triggered_red", &mut tokens.color.triggered_red);
                changed |= color_edit(ui, "dark_pool", &mut tokens.color.dark_pool);
                changed |= color_edit(ui, "info_blue", &mut tokens.color.info_blue);
                changed |= color_edit(ui, "discord", &mut tokens.color.discord);
                ui.add_space(4.0);
                changed |= color_edit(ui, "dialog_fill", &mut tokens.color.dialog_fill);
                changed |= color_edit(ui, "dialog_border", &mut tokens.color.dialog_border);
                changed |= color_edit(ui, "deep_bg", &mut tokens.color.deep_bg);
                changed |= color_edit(ui, "deep_bg_alt", &mut tokens.color.deep_bg_alt);
            }
            Category::Toolbar => {
                changed |= drag_f32(ui, "height", &mut tokens.toolbar.height, 20.0..=60.0);
                changed |= drag_f32(ui, "height_compact", &mut tokens.toolbar.height_compact, 16.0..=50.0);
                changed |= drag_f32(ui, "btn_min_height", &mut tokens.toolbar.btn_min_height, 14.0..=40.0);
                changed |= drag_f32(ui, "btn_padding_x", &mut tokens.toolbar.btn_padding_x, 0.0..=20.0);
                changed |= drag_f32(ui, "right_controls_w", &mut tokens.toolbar.right_controls_width, 50.0..=300.0);
            }
            Category::Panel => {
                changed |= drag_f32(ui, "margin_x", &mut tokens.panel.margin_x, 0.0..=30.0);
                changed |= drag_f32(ui, "margin_top", &mut tokens.panel.margin_top, 0.0..=30.0);
                changed |= drag_f32(ui, "margin_bottom", &mut tokens.panel.margin_bottom, 0.0..=30.0);
                changed |= drag_f32(ui, "compact_margin_x", &mut tokens.panel.compact_margin_x, 0.0..=20.0);
                ui.add_space(4.0);
                ui.label(RichText::new("Widths").monospace().size(9.0).color(Color32::from_rgb(130, 130, 140)));
                changed |= drag_f32(ui, "width_sm (240)", &mut tokens.panel.width_sm, 100.0..=500.0);
                changed |= drag_f32(ui, "width_md (260)", &mut tokens.panel.width_md, 100.0..=500.0);
                changed |= drag_f32(ui, "width_default (280)", &mut tokens.panel.width_default, 100.0..=500.0);
                changed |= drag_f32(ui, "width_lg (300)", &mut tokens.panel.width_lg, 100.0..=500.0);
                changed |= drag_f32(ui, "width_xl (320)", &mut tokens.panel.width_xl, 100.0..=500.0);
                changed |= drag_f32(ui, "order_compact (230)", &mut tokens.panel.order_width_compact, 100.0..=500.0);
                changed |= drag_f32(ui, "order_advanced (300)", &mut tokens.panel.order_width_advanced, 100.0..=500.0);
            }
            Category::Dialog => {
                changed |= drag_u8_range(ui, "header_darken", &mut tokens.dialog.header_darken, 0..=30);
                changed |= drag_f32(ui, "header_padding_x", &mut tokens.dialog.header_padding_x, 0.0..=30.0);
                changed |= drag_f32(ui, "header_padding_y", &mut tokens.dialog.header_padding_y, 0.0..=30.0);
                changed |= drag_f32(ui, "section_indent", &mut tokens.dialog.section_indent, 0.0..=30.0);
            }
            Category::Button => {
                changed |= drag_f32(ui, "action_height (24)", &mut tokens.button.action_height, 14.0..=50.0);
                changed |= drag_f32(ui, "trade_height (30)", &mut tokens.button.trade_height, 14.0..=50.0);
                changed |= drag_f32(ui, "small_height (18)", &mut tokens.button.small_height, 10.0..=40.0);
                changed |= drag_f32(ui, "simple_height (20)", &mut tokens.button.simple_height, 10.0..=40.0);
                changed |= drag_f32(ui, "trade_brightness", &mut tokens.button.trade_brightness, 0.1..=1.0);
                changed |= drag_f32(ui, "trade_hover_bright", &mut tokens.button.trade_hover_brightness, 0.1..=1.0);
            }
            Category::Card => {
                changed |= drag_i8(ui, "margin_left", &mut tokens.card.margin_left);
                changed |= drag_i8(ui, "margin_right", &mut tokens.card.margin_right);
                changed |= drag_i8(ui, "margin_y", &mut tokens.card.margin_y);
                changed |= drag_f32(ui, "radius", &mut tokens.card.radius, 0.0..=20.0);
                changed |= drag_f32(ui, "stripe_width", &mut tokens.card.stripe_width, 0.0..=10.0);
                changed |= drag_f32(ui, "width_sm (200)", &mut tokens.card.width_sm, 50.0..=400.0);
                changed |= drag_f32(ui, "width_md (240)", &mut tokens.card.width_md, 50.0..=400.0);
                changed |= drag_f32(ui, "height_sm (48)", &mut tokens.card.height_sm, 20.0..=200.0);
                changed |= drag_f32(ui, "height_md (52)", &mut tokens.card.height_md, 20.0..=200.0);
                changed |= drag_f32(ui, "height_lg (120)", &mut tokens.card.height_lg, 40.0..=300.0);
            }
            Category::Badge => {
                changed |= drag_f32(ui, "font_size (8)", &mut tokens.badge.font_size, 4.0..=16.0);
                changed |= drag_f32(ui, "height (16)", &mut tokens.badge.height, 8.0..=30.0);
            }
            Category::Tab => {
                changed |= drag_f32(ui, "underline (2.0)", &mut tokens.tab.underline_thickness, 0.0..=6.0);
                changed |= drag_f32(ui, "close_width (14)", &mut tokens.tab.close_width, 8.0..=30.0);
                changed |= drag_f32(ui, "padding_x (10)", &mut tokens.tab.padding_x, 0.0..=30.0);
                changed |= drag_f32(ui, "add_width (44)", &mut tokens.tab.add_width, 20.0..=80.0);
            }
            Category::Table => {
                changed |= drag_f32(ui, "header_height (12)", &mut tokens.table.header_height, 8.0..=30.0);
                changed |= drag_f32(ui, "row_height (20)", &mut tokens.table.row_height, 10.0..=50.0);
                changed |= drag_f32(ui, "row_compact (18)", &mut tokens.table.row_height_compact, 10.0..=50.0);
                changed |= drag_f32(ui, "item_height (36)", &mut tokens.table.item_height, 14.0..=60.0);
                changed |= drag_f32(ui, "interact_height (22)", &mut tokens.table.interact_height, 10.0..=40.0);
            }
            Category::Chart => {
                changed |= drag_f32(ui, "padding_top (4)", &mut tokens.chart.padding_top, 0.0..=30.0);
                changed |= drag_f32(ui, "padding_bottom (30)", &mut tokens.chart.padding_bottom, 0.0..=60.0);
                changed |= drag_f32(ui, "padding_right (80)", &mut tokens.chart.padding_right, 20.0..=200.0);
                changed |= drag_f32(ui, "replay_height (28)", &mut tokens.chart.replay_height, 14.0..=50.0);
                changed |= drag_f32(ui, "pnl_strip_h (60)", &mut tokens.chart.pnl_strip_height, 20.0..=120.0);
                changed |= drag_f32(ui, "pnl_header_h (68)", &mut tokens.chart.pnl_header_height, 30.0..=120.0);
                changed |= drag_f32(ui, "style_bar_w (480)", &mut tokens.chart.style_bar_width, 200.0..=800.0);
            }
            Category::Watchlist => {
                changed |= drag_f32(ui, "row_width (236)", &mut tokens.watchlist.row_width, 100.0..=400.0);
                changed |= drag_f32(ui, "strip_width (50)", &mut tokens.watchlist.strip_width, 20.0..=100.0);
                changed |= drag_f32(ui, "strip_narrow (14)", &mut tokens.watchlist.strip_width_narrow, 4.0..=40.0);
            }
            Category::OrderEntry => {
                changed |= drag_f32(ui, "padding (8)", &mut tokens.order_entry.padding, 0.0..=20.0);
                changed |= drag_f32(ui, "pill_width_sm (90)", &mut tokens.order_entry.pill_width_sm, 40.0..=200.0);
                changed |= drag_f32(ui, "pill_width_md (130)", &mut tokens.order_entry.pill_width_md, 60.0..=250.0);
                changed |= drag_f32(ui, "pill_height (22)", &mut tokens.order_entry.pill_height, 14.0..=40.0);
            }
            Category::PaneHeader => {
                changed |= drag_f32(ui, "height (36)", &mut tokens.pane_header.height, 20.0..=60.0);
                changed |= drag_f32(ui, "height_compact (28)", &mut tokens.pane_header.height_compact, 16.0..=50.0);
            }
            Category::Segmented => {
                changed |= drag_u8_range(ui, "trough_darken (12)", &mut tokens.segmented.trough_darken, 0..=30);
                changed |= drag_f32(ui, "trough_expand_x (4)", &mut tokens.segmented.trough_expand_x, 0.0..=20.0);
                changed |= drag_f32(ui, "btn_padding_x (7)", &mut tokens.segmented.btn_padding_x, 0.0..=20.0);
                changed |= drag_f32(ui, "btn_min_height (24)", &mut tokens.segmented.btn_min_height, 14.0..=40.0);
            }
            Category::IconButton => {
                changed |= drag_f32(ui, "icon_padding (5)", &mut tokens.icon_button.icon_padding, 0.0..=20.0);
                changed |= drag_f32(ui, "min_size (26)", &mut tokens.icon_button.min_size, 14.0..=50.0);
            }
            Category::Form => {
                changed |= drag_f32(ui, "label_width (80)", &mut tokens.form.label_width, 30.0..=200.0);
                changed |= drag_f32(ui, "row_height (18)", &mut tokens.form.row_height, 10.0..=40.0);
            }
            Category::SplitDivider => {
                changed |= drag_f32(ui, "height (6)", &mut tokens.split_divider.height, 2.0..=20.0);
                changed |= drag_f32(ui, "dot_spacing (8)", &mut tokens.split_divider.dot_spacing, 2.0..=20.0);
                changed |= drag_f32(ui, "dot_radius (1.5)", &mut tokens.split_divider.dot_radius, 0.5..=5.0);
                changed |= drag_f32(ui, "active_stroke (2)", &mut tokens.split_divider.active_stroke, 0.5..=5.0);
                changed |= drag_f32(ui, "inactive_stroke (1)", &mut tokens.split_divider.inactive_stroke, 0.1..=3.0);
                changed |= drag_f32(ui, "inset (8)", &mut tokens.split_divider.inset, 0.0..=30.0);
            }
            Category::Tooltip => {
                changed |= drag_f32(ui, "corner_radius (8)", &mut tokens.tooltip.corner_radius, 0.0..=20.0);
                changed |= drag_f32(ui, "padding (8)", &mut tokens.tooltip.padding, 0.0..=20.0);
                changed |= drag_f32(ui, "stat_label (8)", &mut tokens.tooltip.stat_label_size, 4.0..=16.0);
                changed |= drag_f32(ui, "stat_value (10)", &mut tokens.tooltip.stat_value_size, 4.0..=16.0);
            }
            Category::Separator => {
                changed |= drag_f32(ui, "after_space (1)", &mut tokens.separator.after_space, 0.0..=10.0);
                changed |= drag_f32(ui, "shadow_space (4)", &mut tokens.separator.shadow_space, 0.0..=20.0);
            }
        }
        changed
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Widget helpers for the inspector
// ─────────────────────────────────────────────────────────────────────────────

fn drag_f32(ui: &mut Ui, label: &str, value: &mut f32, range: std::ops::RangeInclusive<f32>) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(RichText::new(label).monospace().size(9.0).color(Color32::from_rgb(170, 170, 180)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let resp = ui.add(egui::DragValue::new(value)
                .range(range)
                .speed(0.1)
                .max_decimals(1));
            if resp.changed() { changed = true; }
        });
    });
    changed
}

fn drag_u8(ui: &mut Ui, label: &str, value: &mut u8) -> bool {
    let mut v = *value as f32;
    let changed = drag_f32(ui, label, &mut v, 0.0..=255.0);
    if changed { *value = v as u8; }
    changed
}

fn drag_u8_range(ui: &mut Ui, label: &str, value: &mut u8, range: std::ops::RangeInclusive<u8>) -> bool {
    let mut v = *value as f32;
    let changed = drag_f32(ui, label, &mut v, *range.start() as f32..=*range.end() as f32);
    if changed { *value = v as u8; }
    changed
}

fn drag_i8(ui: &mut Ui, label: &str, value: &mut i8) -> bool {
    let mut v = *value as f32;
    let changed = drag_f32(ui, label, &mut v, -20.0..=30.0);
    if changed { *value = v as i8; }
    changed
}

fn color_edit(ui: &mut Ui, label: &str, color: &mut Rgba) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;

        // Color swatch
        let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 2.0,
            Color32::from_rgba_unmultiplied(color[0], color[1], color[2], color[3]));
        ui.painter().rect_stroke(rect, 2.0,
            Stroke::new(0.5, Color32::from_rgb(60, 60, 70)), egui::StrokeKind::Outside);

        ui.label(RichText::new(label).monospace().size(9.0).color(Color32::from_rgb(170, 170, 180)));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Alpha
            let mut a = color[3] as f32;
            if ui.add(egui::DragValue::new(&mut a).range(0.0..=255.0).speed(1.0).prefix("a:")).changed() {
                color[3] = a as u8; changed = true;
            }
            // B
            let mut b = color[2] as f32;
            if ui.add(egui::DragValue::new(&mut b).range(0.0..=255.0).speed(1.0).prefix("b:")).changed() {
                color[2] = b as u8; changed = true;
            }
            // G
            let mut g = color[1] as f32;
            if ui.add(egui::DragValue::new(&mut g).range(0.0..=255.0).speed(1.0).prefix("g:")).changed() {
                color[1] = g as u8; changed = true;
            }
            // R
            let mut r = color[0] as f32;
            if ui.add(egui::DragValue::new(&mut r).range(0.0..=255.0).speed(1.0).prefix("r:")).changed() {
                color[0] = r as u8; changed = true;
            }
        });
    });
    changed
}

// ─── Helper: map category string to enum ─────────────────────────────────────

fn category_from_name(name: &str) -> Option<Category> {
    match name {
        "Font Sizes" | "Font" => Some(Category::Font),
        "Spacing" => Some(Category::Spacing),
        "Corner Radii" | "Radius" => Some(Category::Radius),
        "Stroke Widths" | "Stroke" => Some(Category::Stroke),
        "Alpha / Opacity" | "Alpha" => Some(Category::Alpha),
        "Shadows" | "Shadow" => Some(Category::Shadow),
        "Semantic Colors" | "Colors" => Some(Category::Colors),
        "Toolbar" => Some(Category::Toolbar),
        "Panels" | "Panel" => Some(Category::Panel),
        "Dialogs" | "Dialog" => Some(Category::Dialog),
        "Buttons" | "Button" => Some(Category::Button),
        "Cards" | "Card" => Some(Category::Card),
        "Badges" | "Badge" => Some(Category::Badge),
        "Tabs" | "Tab" => Some(Category::Tab),
        "Tables / Rows" | "Table" => Some(Category::Table),
        "Chart" => Some(Category::Chart),
        "Watchlist" => Some(Category::Watchlist),
        "Order Entry" => Some(Category::OrderEntry),
        "Pane Header" => Some(Category::PaneHeader),
        "Segmented Control" | "Segmented" => Some(Category::Segmented),
        "Icon Buttons" => Some(Category::IconButton),
        "Forms" | "Form" => Some(Category::Form),
        "Split Divider" => Some(Category::SplitDivider),
        "Tooltips" | "Tooltip" => Some(Category::Tooltip),
        "Separators" | "Separator" => Some(Category::Separator),
        _ => None,
    }
}
