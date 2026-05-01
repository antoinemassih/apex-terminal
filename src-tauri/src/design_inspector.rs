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
    /// Rect of the selected element (updated each frame from hits)
    selected_rect: Option<egui::Rect>,
    /// Which corner handle is being dragged (0=TL,1=TR,2=BR,3=BL), None if none
    drag_corner: Option<usize>,
    /// Radius value at the start of the current drag
    drag_start_radius: f32,
    /// When true the inspector floats as a draggable egui::Window instead of SidePanel.
    pub is_popout: bool,
    /// When true, render the Style Preview as a docked left-side panel simultaneously.
    pub is_preview_left_open: bool,
    /// Active sub-tab within the Design category (0=Style, 1=Theme, 2=Preview).
    pub current_design_subtab: u8,
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
    Style,
    Theme,
    Preview,
    Design,
}

impl Category {
    const ALL: &[Category] = &[
        Category::Design,
        Category::Font, Category::Spacing, Category::Radius, Category::Stroke,
        Category::Alpha, Category::Colors,
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
            Category::Style => "Style Editor",
            Category::Theme => "Theme Editor",
            Category::Preview => "Style Preview",
            Category::Design => "Design",
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
            open: true,
            toml_path,
            category: Category::Font,
            dirty: false,
            status: String::new(),
            filter: String::new(),
            inspect_mode: false,
            hovered_family: None,
            selected_family: None,
            selected_rect: None,
            drag_corner: None,
            drag_start_radius: 0.0,
            is_popout: false,
            is_preview_left_open: false,
            current_design_subtab: 0,
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
            // Reset selected_rect each frame; it gets re-set below if a selected hit is found
            self.selected_rect = None;

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

                // Highlight selected elements; also update selected_rect for drag handles
                if is_selected {
                    painter.rect_filled(rect, 2.0, Color32::from_rgba_unmultiplied(166, 227, 161, 30));
                    painter.rect_stroke(rect, 2.0, Stroke::new(1.5, Color32::from_rgba_unmultiplied(166, 227, 161, 160)), egui::StrokeKind::Outside);
                    self.selected_rect = Some(rect);
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

            // ── Drag handles on selected element ──────────────────────────────
            // Only show when an element is selected AND it has an r_* field.
            if let (Some(fam), Some(sel_rect)) = (self.selected_family, self.selected_rect) {
                if let Some(r_field) = family_radius_field(fam) {
                    let handle_size = 8.0f32;

                    // 4 corner positions: TL, TR, BR, BL
                    let corners = [
                        sel_rect.left_top(),
                        sel_rect.right_top(),
                        sel_rect.right_bottom(),
                        sel_rect.left_bottom(),
                    ];

                    let active_id = STYLE_EDITOR_ACTIVE.load(std::sync::atomic::Ordering::Relaxed);
                    let mut s = crate::chart_renderer::ui::style::get_style_settings(active_id);
                    let current_radius = match r_field {
                        "r_xs"  => s.r_xs as f32,
                        "r_sm"  => s.r_sm as f32,
                        "r_md"  => s.r_md as f32,
                        "r_lg"  => s.r_lg as f32,
                        _       => s.r_pill as f32,
                    };
                    let min_dim = sel_rect.width().min(sel_rect.height());
                    let mut new_radius: Option<f32> = None;

                    // Gather pointer state from input
                    let (ptr_pos, ptr_down, ptr_delta) = ctx.input(|i| (
                        i.pointer.hover_pos(),
                        i.pointer.button_down(egui::PointerButton::Primary),
                        i.pointer.delta(),
                    ));

                    for (i, &corner) in corners.iter().enumerate() {
                        let handle_rect = egui::Rect::from_center_size(corner, egui::vec2(handle_size, handle_size));
                        let is_hovered = ptr_pos.map(|p| handle_rect.contains(p)).unwrap_or(false);
                        let is_dragging = self.drag_corner == Some(i) && ptr_down;

                        // Start drag when pointer presses down on handle
                        if is_hovered && ptr_down && self.drag_corner.is_none() {
                            self.drag_corner = Some(i);
                            self.drag_start_radius = current_radius;
                        }

                        // Handle active drag
                        if is_dragging {
                            let inward = match i {
                                0 => egui::vec2( 1.0,  1.0),
                                1 => egui::vec2(-1.0,  1.0),
                                2 => egui::vec2(-1.0, -1.0),
                                _ => egui::vec2( 1.0, -1.0),
                            };
                            let signed_delta = ptr_delta.x * inward.x + ptr_delta.y * inward.y;
                            let proposed = (self.drag_start_radius + signed_delta).max(0.0).min(min_dim / 2.0);
                            new_radius = Some(proposed);
                            self.drag_start_radius = proposed;
                            ctx.request_repaint();
                        }

                        // Draw handle
                        let handle_col = if is_dragging || is_hovered {
                            Color32::from_rgb(250, 179, 135) // orange when active
                        } else {
                            Color32::from_rgb(166, 227, 161) // green default
                        };
                        painter.rect_filled(
                            egui::Rect::from_center_size(corner, egui::vec2(handle_size - 1.0, handle_size - 1.0)),
                            1.0, handle_col);
                        painter.rect_stroke(
                            egui::Rect::from_center_size(corner, egui::vec2(handle_size - 1.0, handle_size - 1.0)),
                            1.0, Stroke::new(1.0, Color32::from_rgb(40, 40, 50)), egui::StrokeKind::Outside);

                        if is_dragging || is_hovered {
                            ctx.set_cursor_icon(egui::CursorIcon::ResizeNwSe);
                        }
                    }

                    // Release drag when pointer is up
                    if !ptr_down {
                        self.drag_corner = None;
                    }

                    if let Some(r) = new_radius {
                        let r_u8 = r.round() as u8;
                        match r_field {
                            "r_xs"  => s.r_xs   = r_u8,
                            "r_sm"  => s.r_sm   = r_u8,
                            "r_md"  => s.r_md   = r_u8,
                            "r_lg"  => s.r_lg   = r_u8,
                            _       => s.r_pill  = r_u8,
                        }
                        crate::chart_renderer::ui::style::set_style_settings(active_id, s);
                        self.status = format!("{}  {} = {}", fam, r_field, r_u8);
                    }
                }
            }
        }

        // Clear hits at end of frame (they get populated again next frame by style.rs)
        crate::design_tokens::clear_hits();

        let mut modified = false;
        let panel_frame = egui::Frame::NONE
            .fill(Color32::from_rgb(18, 18, 24))
            .stroke(Stroke::new(1.0, Color32::from_rgb(40, 42, 54)))
            .inner_margin(0.0);

        // ── Left preview panel (optional, simultaneous with right inspector) ──
        if self.is_preview_left_open {
            egui::SidePanel::left("design_preview_left")
                .min_width(380.0)
                .max_width(640.0)
                .default_width(420.0)
                .frame(panel_frame)
                .show(ctx, |ui| { render_style_preview(ui); });
        }

        // ── Dispatch: SidePanel (docked) or Window (popout) ──────────────────
        let is_popout = self.is_popout;
        if !is_popout {
        egui::SidePanel::right("design_inspector")
            .min_width(320.0)
            .max_width(420.0)
            .default_width(360.0)
            .frame(panel_frame)
            .show(ctx, |ui| { self.show_inspector_body(ui, tokens, &mut modified); });
        } else {
        let mut open = true;
        egui::Window::new("design_inspector")
            .open(&mut open)
            .resizable([true, true])
            .default_size(egui::vec2(360.0, 700.0))
            .default_pos(egui::pos2(80.0, 40.0))
            .frame(panel_frame)
            .show(ctx, |ui| { self.show_inspector_body(ui, tokens, &mut modified); });
        if !open { self.is_popout = false; }
        }
        modified
    }

    /// Inner body rendered by both SidePanel and Window modes.
    fn show_inspector_body(&mut self, ui: &mut egui::Ui, tokens: &mut DesignTokens, modified: &mut bool) {
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
                                // Popout / dock toggle
                                let popout_label = if self.is_popout { "DOCK" } else { "POP" };
                                let popout_col = Color32::from_rgb(137, 180, 250);
                                if ui.add(egui::Button::new(
                                    RichText::new(popout_label).monospace().size(10.0).strong().color(popout_col))
                                    .fill(Color32::from_rgba_unmultiplied(137, 180, 250, 18))
                                    .stroke(Stroke::new(0.5, Color32::from_rgba_unmultiplied(137, 180, 250, 60)))
                                    .corner_radius(3.0)
                                ).on_hover_text(if self.is_popout { "Dock back to side panel" } else { "Float as draggable window" })
                                .clicked() {
                                    self.is_popout = !self.is_popout;
                                }

                                // Preview left toggle
                                let preview_col = if self.is_preview_left_open {
                                    Color32::from_rgb(250, 179, 135)
                                } else {
                                    Color32::from_rgb(100, 100, 110)
                                };
                                if ui.add(egui::Button::new(
                                    RichText::new("👁 LEFT").monospace().size(10.0).strong().color(preview_col))
                                    .fill(if self.is_preview_left_open { Color32::from_rgba_unmultiplied(250, 179, 135, 20) } else { Color32::TRANSPARENT })
                                    .stroke(Stroke::new(0.5, Color32::from_rgba_unmultiplied(preview_col.r(), preview_col.g(), preview_col.b(), 60)))
                                    .corner_radius(3.0)
                                ).on_hover_text("Toggle Style Preview as a docked left panel")
                                .clicked() {
                                    self.is_preview_left_open = !self.is_preview_left_open;
                                }

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
                                    *modified = true;
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
                                        *modified = true;
                                        self.dirty = true;
                                    }
                                });
                        }
                    }
                });

                // ── Selection details panel ────────────────────────────────
                if let Some(fam) = self.selected_family {
                    ui.separator();
                    egui::Frame::NONE
                        .fill(Color32::from_rgb(14, 14, 22))
                        .inner_margin(egui::Margin { left: 12, right: 12, top: 8, bottom: 10 })
                        .show(ui, |ui| {
                            // Header row: family name + clear button
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(fam).monospace().size(11.0).strong()
                                    .color(Color32::from_rgb(166, 227, 161)));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.add(egui::Button::new(
                                        RichText::new("✕").monospace().size(9.0)
                                            .color(Color32::from_rgb(150, 150, 160)))
                                        .fill(Color32::TRANSPARENT)
                                        .stroke(Stroke::NONE)
                                    ).clicked() {
                                        // Deselect — we can't mutate self here directly,
                                        // so we use a flag communicated via the status.
                                        // We'll handle deselect outside: set a sentinel.
                                    }
                                });
                            });

                            // Look up affecting fields
                            let fields = family_affecting_fields(fam);
                            if fields.is_empty() {
                                ui.label(RichText::new("No field mapping registered.")
                                    .monospace().size(9.0)
                                    .color(Color32::from_rgb(100, 100, 110)));
                            } else {
                                ui.add_space(4.0);
                                ui.label(RichText::new("Affecting fields:").monospace().size(9.0)
                                    .color(Color32::from_rgb(130, 130, 140)));
                                ui.add_space(2.0);

                                for &(section, field) in fields {
                                    ui.horizontal(|ui| {
                                        // Section badge
                                        let badge_color = section_badge_color(section);
                                        ui.label(RichText::new(section).monospace().size(8.0)
                                            .color(badge_color));
                                        ui.label(RichText::new("·").monospace().size(8.0)
                                            .color(Color32::from_rgb(70, 70, 80)));
                                        ui.label(RichText::new(field).monospace().size(9.0)
                                            .color(Color32::from_rgb(200, 200, 215)));
                                    });
                                }

                                // Unique categories represented — offer jump links
                                ui.add_space(4.0);
                                let mut shown_cats: Vec<&str> = Vec::new();
                                for &(section, _) in fields {
                                    if !shown_cats.contains(&section) {
                                        shown_cats.push(section);
                                    }
                                }
                                ui.horizontal_wrapped(|ui| {
                                    for &sec in &shown_cats {
                                        if let Some(cat) = category_from_name(sec) {
                                            let _ = cat; // available if we could mutate self
                                            let label = format!("→ {sec}");
                                            ui.label(RichText::new(&label).monospace().size(8.5)
                                                .color(Color32::from_rgb(137, 180, 250)));
                                        }
                                    }
                                });
                            }
                        });
                }

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
            Category::Style => {
                changed |= render_style_editor(ui);
            }
            Category::Theme => {
                render_theme_editor(ui);
            }
            Category::Preview => {
                render_style_preview(ui);
            }
            Category::Design => {
                changed |= render_design_category(ui);
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

// ─── Style Editor ─────────────────────────────────────────────────────────────

/// Serialise a single f32 the same way style.rs literals are written.
fn fmt_f32(v: f32) -> String {
    // Use Rust debug format: gives "1.0", "1.5", "0.5" etc. without excess decimals.
    format!("{v:?}")
}

fn fmt_bool(v: bool) -> &'static str {
    if v { "true" } else { "false" }
}

fn fmt_bt(bt: crate::chart_renderer::ui::style::ButtonTreatment) -> &'static str {
    use crate::chart_renderer::ui::style::ButtonTreatment;
    match bt {
        ButtonTreatment::SoftPill       => "ButtonTreatment::SoftPill",
        ButtonTreatment::OutlineAccent  => "ButtonTreatment::OutlineAccent",
        ButtonTreatment::UnderlineActive=> "ButtonTreatment::UnderlineActive",
        ButtonTreatment::RaisedActive   => "ButtonTreatment::RaisedActive",
        ButtonTreatment::BlackFillActive=> "ButtonTreatment::BlackFillActive",
    }
}

fn build_arm(id: u8, s: &crate::chart_renderer::ui::style::StyleSettings) -> String {
    let pat = if id == 2 { "_".to_string() } else { id.to_string() };
    let i = "            "; // 12 spaces
    let mut out = String::new();
    out.push_str(&format!("        {pat} => StyleSettings {{\n"));
    out.push_str(&format!("{i}r_xs: {}, r_sm: {}, r_md: {}, r_lg: {}, r_pill: {},\n", s.r_xs, s.r_sm, s.r_md, s.r_lg, s.r_pill));
    out.push_str(&format!("{i}serif_headlines: {},\n", fmt_bool(s.serif_headlines)));
    out.push_str(&format!("{i}button_treatment: {},\n", fmt_bt(s.button_treatment)));
    out.push_str(&format!("{i}hairline_borders: {},\n", fmt_bool(s.hairline_borders)));
    out.push_str(&format!("{i}stroke_hair: {}, stroke_thin: {}, stroke_std: {},\n",
        fmt_f32(s.stroke_hair), fmt_f32(s.stroke_thin), fmt_f32(s.stroke_std)));
    out.push_str(&format!("{i}stroke_bold: {}, stroke_thick: {},\n",
        fmt_f32(s.stroke_bold), fmt_f32(s.stroke_thick)));
    out.push_str(&format!("{i}shadows_enabled: {}, solid_active_fills: {},\n",
        fmt_bool(s.shadows_enabled), fmt_bool(s.solid_active_fills)));
    out.push_str(&format!("{i}uppercase_section_labels: {}, label_letter_spacing_px: {},\n",
        fmt_bool(s.uppercase_section_labels), fmt_f32(s.label_letter_spacing_px)));
    out.push_str(&format!("{i}toolbar_height_scale: {}, header_height_scale: {},\n",
        fmt_f32(s.toolbar_height_scale), fmt_f32(s.header_height_scale)));
    out.push_str(&format!("{i}font_hero: {}, vertical_group_dividers: {},\n",
        fmt_f32(s.font_hero), fmt_bool(s.vertical_group_dividers)));
    out.push_str(&format!("{i}show_active_tab_underline: {},\n", fmt_bool(s.show_active_tab_underline)));
    out.push_str(&format!("{i}active_header_fill_multiply: {}, inactive_header_fill: {},\n",
        fmt_f32(s.active_header_fill_multiply), fmt_bool(s.inactive_header_fill)));
    out.push_str(&format!("{i}account_strip_height: {},\n", fmt_f32(s.account_strip_height)));
    out.push_str("        },");
    out
}

/// Rewrite the style_defaults function body in style.rs between the BEGIN/END markers.
/// Returns Ok(()) on success, Err(message) on failure.
fn save_style_defaults_to_source() -> Result<(), String> {
    use crate::chart_renderer::ui::style::get_style_settings;

    // Locate style.rs relative to CARGO_MANIFEST_DIR (set at compile time).
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let style_path = manifest_dir.join("src/chart_renderer/ui/style.rs");

    let src = std::fs::read_to_string(&style_path)
        .map_err(|e| format!("read failed: {e}"))?;

    let begin_marker = "// ┌─ STYLE_DEFAULTS_BEGIN ─────────────────────────────────────────────────────";
    let end_marker   = "// └─ STYLE_DEFAULTS_END ───────────────────────────────────────────────────────";

    let begin_pos = src.find(begin_marker)
        .ok_or_else(|| "STYLE_DEFAULTS_BEGIN marker not found".to_string())?;
    let end_pos = src.find(end_marker)
        .ok_or_else(|| "STYLE_DEFAULTS_END marker not found".to_string())?;
    if end_pos <= begin_pos {
        return Err("END marker before BEGIN marker".to_string());
    }

    // Build the replacement block (BEGIN marker + fn body + END marker)
    let s0 = get_style_settings(0); // Meridien → arm `_`
    let s1 = get_style_settings(1); // Aperture → arm `1`
    let s2 = get_style_settings(2); // Octave   → arm `2`
    let arm1 = build_arm(1, &s1);
    let arm2 = build_arm(2, &s2);
    let arm0 = build_arm(0, &s0); // will render as `_`

    let mut new_block = String::new();
    new_block.push_str(begin_marker);
    new_block.push('\n');
    new_block.push_str("fn style_defaults(id: u8) -> StyleSettings {\n");
    new_block.push_str("    match id {\n");
    new_block.push_str(&arm1);
    new_block.push('\n');
    new_block.push_str(&arm2);
    new_block.push('\n');
    new_block.push_str(&arm0);
    new_block.push('\n');
    new_block.push_str("    }\n");
    new_block.push_str("}\n");
    new_block.push_str(end_marker);

    // Replace the slice from BEGIN to end of END marker line
    let end_of_end = end_pos + end_marker.len();
    let new_src = format!("{}{}{}", &src[..begin_pos], new_block, &src[end_of_end..]);

    std::fs::write(&style_path, new_src)
        .map_err(|e| format!("write failed: {e}"))?;
    Ok(())
}

thread_local! {
    static DESIGN_SUBTAB: std::cell::RefCell<u8> = const { std::cell::RefCell::new(0) };
}

/// Unified "Design" category: Style / Theme / Preview in sub-tabs.
fn render_design_category(ui: &mut Ui) -> bool {
    let mut changed = false;
    let accent = Color32::from_rgb(203, 166, 247);
    let dim = Color32::from_rgb(120, 120, 130);

    // Sub-tab bar
    let current_tab = DESIGN_SUBTAB.with(|t| *t.borrow());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (idx, label) in ["Style", "Theme", "Preview"].iter().enumerate() {
            let idx = idx as u8;
            let active = current_tab == idx;
            let (fg, bg, border) = if active {
                (accent, Color32::from_rgba_unmultiplied(203, 166, 247, 20),
                 Color32::from_rgba_unmultiplied(203, 166, 247, 80))
            } else {
                (dim, Color32::TRANSPARENT, Color32::from_rgb(50, 50, 60))
            };
            if ui.add(egui::Button::new(RichText::new(*label).monospace().size(10.0).strong().color(fg))
                .fill(bg).stroke(Stroke::new(0.8, border)).corner_radius(3.0)
                .min_size(egui::vec2(60.0, 22.0))
            ).clicked() {
                DESIGN_SUBTAB.with(|t| *t.borrow_mut() = idx);
            }
        }
    });

    ui.add_space(6.0);
    ui.separator();
    ui.add_space(4.0);

    match current_tab {
        0 => { changed |= render_style_editor(ui); }
        1 => { render_theme_editor(ui); }
        _ => { render_style_preview(ui); }
    }
    changed
}

fn render_style_editor(ui: &mut Ui) -> bool {
    use crate::chart_renderer::ui::style::{
        ButtonTreatment, get_style_settings, set_style_settings, set_active_style,
        add_style_preset, delete_style_preset, rename_style_preset, list_style_presets,
    };
    let mut changed = false;

    let accent_col = Color32::from_rgb(203, 166, 247);
    let dim_col    = Color32::from_rgb(120, 120, 130);
    let green_col  = Color32::from_rgb(166, 227, 161);

    // ── Active-style switcher (from live preset list) ──────────────────────────
    ui.horizontal_wrapped(|ui| {
        let active_id = STYLE_EDITOR_ACTIVE.load(std::sync::atomic::Ordering::Relaxed);
        for (id, name) in list_style_presets() {
            let is_active = active_id == id;
            let (fg, bg, border) = if is_active {
                (accent_col, Color32::from_rgba_unmultiplied(203, 166, 247, 30),
                 Color32::from_rgba_unmultiplied(203, 166, 247, 120))
            } else {
                (dim_col, Color32::TRANSPARENT, Color32::from_rgb(50, 50, 60))
            };
            if ui.add(egui::Button::new(RichText::new(&name).monospace().size(11.0).strong().color(fg))
                .fill(bg).stroke(Stroke::new(0.8, border)).corner_radius(3.0)
                .min_size(egui::vec2(60.0, 22.0))
            ).clicked() {
                STYLE_EDITOR_ACTIVE.store(id, std::sync::atomic::Ordering::Relaxed);
                set_active_style(id);
                changed = true;
            }
        }
    });

    ui.add_space(4.0);

    // ── Preset management row ──────────────────────────────────────────────────
    ui.horizontal(|ui| {
        // "+ New Preset" button
        if ui.add(egui::Button::new(RichText::new("+ New Preset").monospace().size(9.0).strong().color(green_col))
            .fill(Color32::from_rgba_unmultiplied(166, 227, 161, 15))
            .stroke(Stroke::new(0.8, Color32::from_rgba_unmultiplied(166, 227, 161, 80)))
            .corner_radius(3.0)
        ).clicked() {
            STYLE_NEW_PRESET_OPEN.with(|o| *o.borrow_mut() = true);
        }

        ui.add_space(8.0);

        // Save to source button (only for canonical 3)
        if ui.add(egui::Button::new(
            RichText::new("Save to source").monospace().size(9.0).strong().color(green_col))
            .fill(Color32::from_rgba_unmultiplied(166, 227, 161, 20))
            .stroke(Stroke::new(0.8, Color32::from_rgba_unmultiplied(166, 227, 161, 100)))
            .corner_radius(3.0)
        ).clicked() {
            match save_style_defaults_to_source() {
                Ok(()) => STYLE_SAVE_STATUS.with(|s| *s.borrow_mut() = "Saved ✓".to_string()),
                Err(e) => STYLE_SAVE_STATUS.with(|s| *s.borrow_mut() = format!("Save failed: {e}")),
            }
        }
        STYLE_SAVE_STATUS.with(|s| {
            let msg = s.borrow();
            if !msg.is_empty() {
                ui.label(RichText::new(msg.as_str()).monospace().size(9.0).color(dim_col));
            }
        });
    });

    // ── "New Preset" dialog ────────────────────────────────────────────────────
    let mut do_create = false;
    STYLE_NEW_PRESET_OPEN.with(|o| {
        if !*o.borrow() { return; }
        egui::Window::new("new_preset_dialog")
            .title_bar(false)
            .resizable(false)
            .fixed_size(egui::vec2(260.0, 0.0))
            .show(ui.ctx(), |ui| {
                ui.label(RichText::new("New Style Preset").monospace().size(11.0).strong().color(accent_col));
                ui.add_space(4.0);
                STYLE_NEW_PRESET_NAME.with(|n| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Name:").monospace().size(9.0).color(dim_col));
                        ui.text_edit_singleline(&mut *n.borrow_mut());
                    });
                });
                STYLE_NEW_PRESET_CLONE_FROM.with(|c| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Clone from:").monospace().size(9.0).color(dim_col));
                        egui::ComboBox::from_id_salt("new_preset_clone")
                            .selected_text(list_style_presets().get(*c.borrow()).map(|(_, n)| n.clone()).unwrap_or_default())
                            .show_ui(ui, |ui| {
                                for (id, name) in list_style_presets() {
                                    if ui.selectable_label(*c.borrow() == id as usize, &name).clicked() {
                                        *c.borrow_mut() = id as usize;
                                    }
                                }
                            });
                    });
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.button("Create").clicked() { do_create = true; }
                    if ui.button("Cancel").clicked() {
                        *o.borrow_mut() = false;
                    }
                });
            });
    });
    if do_create {
        STYLE_NEW_PRESET_OPEN.with(|o| *o.borrow_mut() = false);
        let (name, clone_from) = STYLE_NEW_PRESET_NAME.with(|n| {
            STYLE_NEW_PRESET_CLONE_FROM.with(|c| {
                (n.borrow().clone(), *c.borrow() as u8)
            })
        });
        if !name.is_empty() {
            let settings = get_style_settings(clone_from);
            let new_id = add_style_preset(&name, settings);
            STYLE_EDITOR_ACTIVE.store(new_id, std::sync::atomic::Ordering::Relaxed);
            set_active_style(new_id);
            STYLE_NEW_PRESET_NAME.with(|n| n.borrow_mut().clear());
            changed = true;
        }
    }

    ui.add_space(6.0);

    // ── Per-preset collapsible sections ───────────────────────────────────────
    let presets = list_style_presets();
    let mut to_delete: Option<u8> = None;

    for (preset_id, preset_name) in &presets {
        let id = *preset_id;
        let header_color = Color32::from_rgb(180, 180, 200);
        let is_canonical = id < 3;

        // Header: name (inline rename for user presets) + delete button
        let header_label = RichText::new(preset_name).monospace().size(11.0).strong().color(header_color);
        let section = egui::CollapsingHeader::new(header_label)
            .id_salt(egui::Id::new(("style_section", id)))
            .default_open(id == STYLE_EDITOR_ACTIVE.load(std::sync::atomic::Ordering::Relaxed))
            .show(ui, |ui| {
                // Rename row (user presets only)
                if !is_canonical {
                    STYLE_RENAME_BUFS.with(|bufs| {
                        let mut bufs = bufs.borrow_mut();
                        let entry = bufs.entry(id).or_insert_with(|| preset_name.clone());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Name:").monospace().size(9.0).color(dim_col));
                            let r = ui.text_edit_singleline(entry);
                            if r.lost_focus() && !entry.is_empty() {
                                rename_style_preset(id, entry.clone());
                            }
                        });
                        ui.add_space(4.0);
                    });
                }

                let mut s = get_style_settings(id);
                let mut local_changed = false;

                // Corner radii
                ui.label(RichText::new("Corner radii").monospace().size(9.0).color(Color32::from_rgb(130,130,140)));
                local_changed |= style_drag_u8(ui, "r_xs", &mut s.r_xs);
                local_changed |= style_drag_u8(ui, "r_sm", &mut s.r_sm);
                local_changed |= style_drag_u8(ui, "r_md", &mut s.r_md);
                local_changed |= style_drag_u8(ui, "r_lg", &mut s.r_lg);
                local_changed |= style_drag_u8(ui, "r_pill", &mut s.r_pill);

                ui.add_space(4.0);

                // Button treatment
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.label(RichText::new("button_treatment").monospace().size(9.0).color(Color32::from_rgb(170,170,180)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        egui::ComboBox::from_id_salt(egui::Id::new(("bt_combo", id)))
                            .selected_text(RichText::new(format!("{:?}", s.button_treatment)).monospace().size(9.0))
                            .show_ui(ui, |ui| {
                                for bt in [ButtonTreatment::SoftPill, ButtonTreatment::OutlineAccent,
                                           ButtonTreatment::UnderlineActive, ButtonTreatment::RaisedActive,
                                           ButtonTreatment::BlackFillActive] {
                                    if ui.selectable_label(s.button_treatment == bt, format!("{bt:?}")).clicked() {
                                        s.button_treatment = bt;
                                        local_changed = true;
                                    }
                                }
                            });
                    });
                });

                ui.add_space(4.0);

                // Bool flags
                local_changed |= style_checkbox(ui, "hairline_borders", &mut s.hairline_borders);
                local_changed |= style_checkbox(ui, "shadows_enabled", &mut s.shadows_enabled);
                local_changed |= style_checkbox(ui, "solid_active_fills", &mut s.solid_active_fills);
                local_changed |= style_checkbox(ui, "uppercase_section_labels", &mut s.uppercase_section_labels);
                local_changed |= style_checkbox(ui, "serif_headlines", &mut s.serif_headlines);
                local_changed |= style_checkbox(ui, "vertical_group_dividers", &mut s.vertical_group_dividers);
                local_changed |= style_checkbox(ui, "show_active_tab_underline", &mut s.show_active_tab_underline);
                local_changed |= style_checkbox(ui, "inactive_header_fill", &mut s.inactive_header_fill);

                ui.add_space(4.0);

                // Stroke widths
                ui.label(RichText::new("Stroke widths").monospace().size(9.0).color(Color32::from_rgb(130,130,140)));
                local_changed |= style_drag_f32(ui, "stroke_hair", &mut s.stroke_hair, 0.0..=3.0);
                local_changed |= style_drag_f32(ui, "stroke_thin", &mut s.stroke_thin, 0.0..=3.0);
                local_changed |= style_drag_f32(ui, "stroke_std",  &mut s.stroke_std,  0.0..=4.0);
                local_changed |= style_drag_f32(ui, "stroke_bold", &mut s.stroke_bold, 0.0..=4.0);
                local_changed |= style_drag_f32(ui, "stroke_thick",&mut s.stroke_thick,0.0..=6.0);

                ui.add_space(4.0);

                // Scale / size fields
                local_changed |= style_drag_f32(ui, "toolbar_height_scale", &mut s.toolbar_height_scale, 0.5..=2.5);
                local_changed |= style_drag_f32(ui, "header_height_scale",  &mut s.header_height_scale,  0.5..=2.5);
                local_changed |= style_drag_f32(ui, "font_hero",             &mut s.font_hero,             8.0..=80.0);
                local_changed |= style_drag_f32(ui, "active_header_fill_multiply", &mut s.active_header_fill_multiply, 0.0..=1.5);
                local_changed |= style_drag_f32(ui, "account_strip_height", &mut s.account_strip_height, 16.0..=80.0);
                local_changed |= style_drag_f32(ui, "label_letter_spacing_px", &mut s.label_letter_spacing_px, -2.0..=4.0);

                // Delete button for user presets
                if !is_canonical {
                    ui.add_space(6.0);
                    if ui.add(egui::Button::new(
                        RichText::new("Delete preset").monospace().size(9.0).color(Color32::from_rgb(243, 139, 168)))
                        .fill(Color32::from_rgba_unmultiplied(243, 139, 168, 15))
                        .stroke(Stroke::new(0.8, Color32::from_rgba_unmultiplied(243, 139, 168, 80)))
                        .corner_radius(3.0)
                    ).clicked() {
                        to_delete = Some(id);
                    }
                }

                if local_changed {
                    set_style_settings(id, s);
                    changed = true;
                }
            });
        let _ = section;
    }

    // Deferred delete (after loop to avoid borrow issues)
    if let Some(del_id) = to_delete {
        delete_style_preset(del_id);
        // If the deleted preset was active, fall back to Meridien
        if STYLE_EDITOR_ACTIVE.load(std::sync::atomic::Ordering::Relaxed) == del_id {
            STYLE_EDITOR_ACTIVE.store(0, std::sync::atomic::Ordering::Relaxed);
            set_active_style(0);
        }
        changed = true;
    }

    changed
}

static STYLE_EDITOR_ACTIVE: std::sync::atomic::AtomicU8 =
    std::sync::atomic::AtomicU8::new(0);

thread_local! {
    static STYLE_SAVE_STATUS: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    static STYLE_NEW_PRESET_OPEN: std::cell::RefCell<bool> = const { std::cell::RefCell::new(false) };
    static STYLE_NEW_PRESET_NAME: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    static STYLE_NEW_PRESET_CLONE_FROM: std::cell::RefCell<usize> = const { std::cell::RefCell::new(0) };
    /// Rename buffer: maps preset id → current draft name (for user presets).
    static STYLE_RENAME_BUFS: std::cell::RefCell<std::collections::HashMap<u8, String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

fn style_drag_u8(ui: &mut Ui, label: &str, value: &mut u8) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let dim = Color32::from_rgb(170, 170, 180);
        ui.label(RichText::new(label).monospace().size(9.0).color(dim));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(egui::Slider::new(value, 0u8..=99u8).suffix("px")).changed() {
                changed = true;
            }
        });
    });
    changed
}

fn style_drag_f32(ui: &mut Ui, label: &str, value: &mut f32, range: std::ops::RangeInclusive<f32>) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let dim = Color32::from_rgb(170, 170, 180);
        ui.label(RichText::new(label).monospace().size(9.0).color(dim));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(egui::Slider::new(value, range)).changed() {
                changed = true;
            }
        });
    });
    changed
}

fn style_checkbox(ui: &mut Ui, label: &str, value: &mut bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(RichText::new(label).monospace().size(9.0).color(Color32::from_rgb(170,170,180)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.checkbox(value, "").changed() { changed = true; }
        });
    });
    changed
}

// ─── Family → affecting fields lookup ────────────────────────────────────────

/// Returns `(section_label, field_name)` pairs for a registered element family.
/// `section_label` matches the Category labels used in the inspector so we can
/// offer jump links later.
fn family_affecting_fields(family: &str) -> &'static [(&'static str, &'static str)] {
    // Static table: (family_name, [(section, field), ...])
    // Section names must match Category::label() values.
    const TABLE: &[(&str, &[(&str, &str)])] = &[
        ("TOOLBAR", &[
            ("Toolbar", "height"),
            ("Toolbar", "height_compact"),
            ("Toolbar", "btn_min_height"),
            ("Toolbar", "btn_padding_x"),
            ("Style Editor", "toolbar_height_scale"),
            ("Style Editor", "hairline_borders"),
        ]),
        ("TOOLBAR_BTN", &[
            ("Toolbar", "btn_min_height"),
            ("Toolbar", "btn_padding_x"),
            ("Style Editor", "button_treatment"),
            ("Style Editor", "r_md"),
            ("Style Editor", "hairline_borders"),
        ]),
        ("CHROME_BTN", &[
            ("Style Editor", "r_md"),
            ("Style Editor", "button_treatment"),
            ("Style Editor", "hairline_borders"),
            ("Toolbar", "btn_min_height"),
        ]),
        ("PANE_HEADER", &[
            ("Pane Header", "height"),
            ("Pane Header", "height_compact"),
            ("Style Editor", "header_height_scale"),
            ("Style Editor", "active_header_fill_multiply"),
            ("Style Editor", "inactive_header_fill"),
            ("Style Editor", "hairline_borders"),
            ("Style Editor", "font_hero"),
        ]),
        ("CARD", &[
            ("Cards", "radius"),
            ("Cards", "margin_left"),
            ("Cards", "margin_right"),
            ("Cards", "margin_y"),
            ("Cards", "stripe_width"),
            ("Style Editor", "r_md"),
            ("Style Editor", "shadows_enabled"),
            ("Style Editor", "hairline_borders"),
        ]),
        ("MODAL", &[
            ("Dialogs", "header_darken"),
            ("Dialogs", "header_padding_x"),
            ("Dialogs", "header_padding_y"),
            ("Dialogs", "section_indent"),
            ("Style Editor", "r_lg"),
            ("Style Editor", "shadows_enabled"),
            ("Semantic Colors", "dialog_fill"),
            ("Semantic Colors", "dialog_border"),
        ]),
        ("WATCHLIST_ROW", &[
            ("Watchlist", "row_width"),
            ("Watchlist", "strip_width"),
            ("Watchlist", "strip_width_narrow"),
            ("Tables / Rows", "row_height"),
            ("Tables / Rows", "row_height_compact"),
            ("Style Editor", "hairline_borders"),
        ]),
        ("ORDER_ROW", &[
            ("Tables / Rows", "row_height"),
            ("Tables / Rows", "item_height"),
            ("Tables / Rows", "interact_height"),
            ("Style Editor", "hairline_borders"),
        ]),
        ("NEWS_ROW", &[
            ("Tables / Rows", "row_height"),
            ("Tables / Rows", "item_height"),
            ("Style Editor", "hairline_borders"),
        ]),
        ("DOM_ROW", &[
            ("Tables / Rows", "row_height"),
            ("Tables / Rows", "row_height_compact"),
            ("Style Editor", "hairline_borders"),
        ]),
        ("BUTTON_PRIMARY", &[
            ("Buttons", "action_height"),
            ("Buttons", "trade_height"),
            ("Buttons", "trade_brightness"),
            ("Buttons", "trade_hover_brightness"),
            ("Style Editor", "r_md"),
            ("Style Editor", "button_treatment"),
            ("Style Editor", "solid_active_fills"),
        ]),
        ("BUTTON_SECONDARY", &[
            ("Buttons", "action_height"),
            ("Buttons", "small_height"),
            ("Style Editor", "r_md"),
            ("Style Editor", "button_treatment"),
            ("Style Editor", "hairline_borders"),
        ]),
        ("BUTTON_SMALL", &[
            ("Buttons", "small_height"),
            ("Buttons", "simple_height"),
            ("Style Editor", "r_sm"),
            ("Style Editor", "button_treatment"),
        ]),
        ("INPUT_TEXT", &[
            ("Forms", "label_width"),
            ("Forms", "row_height"),
            ("Style Editor", "r_md"),
            ("Style Editor", "stroke_thin"),
            ("Style Editor", "hairline_borders"),
        ]),
        ("INPUT_NUMBER", &[
            ("Forms", "label_width"),
            ("Forms", "row_height"),
            ("Style Editor", "r_md"),
            ("Style Editor", "stroke_thin"),
        ]),
        ("LIST_ROW", &[
            ("Tables / Rows", "row_height"),
            ("Tables / Rows", "row_height_compact"),
            ("Style Editor", "hairline_borders"),
        ]),
        ("TAB_BAR", &[
            ("Tabs", "underline_thickness"),
            ("Tabs", "padding_x"),
            ("Tabs", "close_width"),
            ("Tabs", "add_width"),
            ("Style Editor", "show_active_tab_underline"),
            ("Style Editor", "hairline_borders"),
        ]),
        ("FORM_ROW", &[
            ("Forms", "label_width"),
            ("Forms", "row_height"),
            ("Style Editor", "hairline_borders"),
            ("Style Editor", "uppercase_section_labels"),
            ("Style Editor", "label_letter_spacing_px"),
        ]),
        ("SECTION_LABEL", &[
            ("Style Editor", "uppercase_section_labels"),
            ("Style Editor", "label_letter_spacing_px"),
            ("Style Editor", "font_hero"),
            ("Font Sizes", "sm"),
            ("Font Sizes", "xs"),
        ]),
        ("SEGMENTED", &[
            ("Segmented Control", "trough_darken"),
            ("Segmented Control", "trough_expand_x"),
            ("Segmented Control", "btn_padding_x"),
            ("Segmented Control", "btn_min_height"),
            ("Style Editor", "r_md"),
        ]),
        ("ICON_BTN", &[
            ("Icon Buttons", "icon_padding"),
            ("Icon Buttons", "min_size"),
            ("Style Editor", "r_md"),
            ("Style Editor", "button_treatment"),
        ]),
        ("BADGE", &[
            ("Badges", "font_size"),
            ("Badges", "height"),
            ("Style Editor", "r_pill"),
        ]),
        ("TOOLTIP", &[
            ("Tooltips", "corner_radius"),
            ("Tooltips", "padding"),
            ("Tooltips", "stat_label_size"),
            ("Tooltips", "stat_value_size"),
            ("Style Editor", "shadows_enabled"),
        ]),
        ("SPLIT_DIVIDER", &[
            ("Split Divider", "height"),
            ("Split Divider", "dot_spacing"),
            ("Split Divider", "dot_radius"),
            ("Split Divider", "active_stroke"),
            ("Split Divider", "inactive_stroke"),
            ("Split Divider", "inset"),
        ]),
        ("ORDER_ENTRY", &[
            ("Order Entry", "padding"),
            ("Order Entry", "pill_width_sm"),
            ("Order Entry", "pill_width_md"),
            ("Order Entry", "pill_height"),
            ("Style Editor", "r_md"),
            ("Style Editor", "hairline_borders"),
        ]),
    ];

    for &(fam, fields) in TABLE {
        if fam.eq_ignore_ascii_case(family) {
            return fields;
        }
    }
    &[]
}

/// Returns the most prominent r_* field name for a family, or None if no radius applies.
/// Used to determine which StyleSettings field drag handles should edit.
fn family_radius_field(family: &str) -> Option<&'static str> {
    match family.to_ascii_uppercase().as_str() {
        "BADGE"            => Some("r_pill"),
        "MODAL"            => Some("r_lg"),
        "BUTTON_SMALL"     => Some("r_sm"),
        "TOOLBAR_BTN"
        | "CHROME_BTN"
        | "CARD"
        | "BUTTON_PRIMARY"
        | "BUTTON_SECONDARY"
        | "INPUT_TEXT"
        | "INPUT_NUMBER"
        | "SEGMENTED"
        | "ICON_BTN"
        | "FORM_ROW"
        | "ORDER_ENTRY"    => Some("r_md"),
        // Families with no corner-radius field
        _ => None,
    }
}

/// Color for a section badge in the selection details panel.
fn section_badge_color(section: &str) -> Color32 {
    match section {
        "Style Editor"       => Color32::from_rgb(203, 166, 247), // purple
        "Toolbar"            => Color32::from_rgb(250, 179, 135), // peach
        "Pane Header"        => Color32::from_rgb(137, 220, 235), // sky
        "Semantic Colors"    => Color32::from_rgb(249, 226, 175), // yellow
        "Tables / Rows"      => Color32::from_rgb(166, 227, 161), // green
        "Buttons"            => Color32::from_rgb(243, 139, 168), // red/pink
        "Cards"              => Color32::from_rgb(148, 226, 213), // teal
        "Dialogs"            => Color32::from_rgb(180, 190, 254), // lavender
        "Forms"              => Color32::from_rgb(250, 179, 135), // peach
        "Font Sizes"         => Color32::from_rgb(249, 226, 175), // yellow
        _                    => Color32::from_rgb(150, 150, 165), // dim
    }
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
        "Style Editor" | "Style" => Some(Category::Style),
        "Theme Editor" | "Theme" => Some(Category::Theme),
        "Style Preview" | "Preview" => Some(Category::Preview),
        _ => None,
    }
}

// ─── Theme Editor ─────────────────────────────────────────────────────────────

static THEME_SAVE_STATUS: std::sync::OnceLock<std::sync::Mutex<String>> =
    std::sync::OnceLock::new();

fn theme_save_status() -> &'static std::sync::Mutex<String> {
    THEME_SAVE_STATUS.get_or_init(|| std::sync::Mutex::new(String::new()))
}

fn theme_color_row(ui: &mut Ui, label: &str, color: &mut egui::Color32) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.label(RichText::new(label).monospace().size(9.0).color(Color32::from_rgb(170, 170, 180)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if egui::color_picker::color_edit_button_srgba(
                ui, color, egui::color_picker::Alpha::Opaque
            ).changed() {
                changed = true;
            }
        });
    });
    changed
}

fn save_themes_to_source() -> Result<(), String> {
    use crate::chart_renderer::gpu::get_all_themes;
    let themes = get_all_themes();
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let gpu_path = manifest_dir.join("src/chart_renderer/gpu.rs");

    let src = std::fs::read_to_string(&gpu_path)
        .map_err(|e| format!("read failed: {e}"))?;

    let begin_marker = "// ┌─ THEMES_BEGIN ──────────────────────────────────────────────────────────────";
    let end_marker   = "// └─ THEMES_END ────────────────────────────────────────────────────────────────";

    let begin_pos = src.find(begin_marker)
        .ok_or_else(|| "THEMES_BEGIN marker not found".to_string())?;
    let end_pos = src.find(end_marker)
        .ok_or_else(|| "THEMES_END marker not found".to_string())?;
    if end_pos <= begin_pos {
        return Err("END marker before BEGIN marker".to_string());
    }

    let mut new_block = String::new();
    new_block.push_str(begin_marker);
    new_block.push('\n');
    new_block.push_str("pub(crate) const THEMES: &[Theme] = &[\n");

    for (i, t) in themes.iter().enumerate() {
        let [rb, gb, bb, _] = t.bg.to_array();
        let [rbu, gbu, bbu, _] = t.bull.to_array();
        let [rbe, gbe, bbe, _] = t.bear.to_array();
        let [rd, gd, bd, _] = t.dim.to_array();
        let [rtb, gtb, btb, _] = t.toolbar_bg.to_array();
        let [rtbr, gtbr, btbr, _] = t.toolbar_border.to_array();
        let [ra, ga, ba, _] = t.accent.to_array();
        let [rt, gt, bt, _] = t.text.to_array();
        // Insert known section dividers
        match i {
            8  => new_block.push_str("    // ── Additional themes ──\n"),
            12 => new_block.push_str("    // ── Light themes ──\n"),
            _ => {}
        }
        new_block.push_str(&format!(
            "    Theme {{ name: {:?}, bg: rgb({},{},{}), bull: rgb({},{},{}), bear: rgb({},{},{}), dim: rgb({},{},{}), toolbar_bg: rgb({},{},{}), toolbar_border: rgb({},{},{}), accent: rgb({},{},{}), text: rgb({},{},{}) }},\n",
            t.name,
            rb, gb, bb,
            rbu, gbu, bbu,
            rbe, gbe, bbe,
            rd, gd, bd,
            rtb, gtb, btb,
            rtbr, gtbr, btbr,
            ra, ga, ba,
            rt, gt, bt,
        ));
    }
    new_block.push_str("];\n");
    new_block.push_str(end_marker);

    let end_of_end = end_pos + end_marker.len();
    let new_src = format!("{}{}{}", &src[..begin_pos], new_block, &src[end_of_end..]);

    std::fs::write(&gpu_path, new_src)
        .map_err(|e| format!("write failed: {e}"))?;
    Ok(())
}

fn render_theme_editor(ui: &mut Ui) {
    use crate::chart_renderer::gpu::{get_all_themes, set_theme, THEMES};

    // Save to source button
    ui.horizontal(|ui| {
        if ui.add(egui::Button::new(
            RichText::new("Save to source")
                .monospace().size(10.0).strong()
                .color(Color32::from_rgb(166, 227, 161)))
            .fill(Color32::from_rgba_unmultiplied(166, 227, 161, 20))
            .stroke(egui::Stroke::new(0.8, Color32::from_rgba_unmultiplied(166, 227, 161, 100)))
            .corner_radius(3.0)
        ).clicked() {
            match save_themes_to_source() {
                Ok(()) => { *theme_save_status().lock().unwrap() = "Saved ✓".to_string(); }
                Err(e) => { *theme_save_status().lock().unwrap() = format!("Failed: {e}"); }
            }
        }
        let msg = theme_save_status().lock().unwrap().clone();
        if !msg.is_empty() {
            ui.label(RichText::new(msg).monospace().size(9.0).color(Color32::from_rgb(150,150,160)));
        }
    });

    ui.add_space(4.0);

    if ui.add(egui::Button::new(
        RichText::new("Reset all to defaults").monospace().size(9.0).color(Color32::from_rgb(200,150,150)))
        .fill(Color32::TRANSPARENT)
        .stroke(egui::Stroke::new(0.5, Color32::from_rgb(80,60,60)))
        .corner_radius(2.0)
    ).clicked() {
        for (i, t) in THEMES.iter().enumerate() {
            set_theme(i, t.clone());
        }
    }

    ui.add_space(6.0);

    let mut themes = get_all_themes();
    for (idx, theme) in themes.iter_mut().enumerate() {
        let header_color = Color32::from_rgb(180, 180, 200);
        egui::CollapsingHeader::new(
            RichText::new(theme.name).monospace().size(11.0).strong().color(header_color)
        )
        .id_salt(egui::Id::new(("theme_section", idx)))
        .default_open(false)
        .show(ui, |ui| {
            let mut dirty = false;
            dirty |= theme_color_row(ui, "bg",             &mut theme.bg);
            dirty |= theme_color_row(ui, "bull",           &mut theme.bull);
            dirty |= theme_color_row(ui, "bear",           &mut theme.bear);
            dirty |= theme_color_row(ui, "dim",            &mut theme.dim);
            dirty |= theme_color_row(ui, "toolbar_bg",     &mut theme.toolbar_bg);
            dirty |= theme_color_row(ui, "toolbar_border", &mut theme.toolbar_border);
            dirty |= theme_color_row(ui, "accent",         &mut theme.accent);
            dirty |= theme_color_row(ui, "text",           &mut theme.text);
            if dirty {
                set_theme(idx, theme.clone());
            }
        });
    }
}

// ─── Style Preview Panel ─────────────────────────────────────────────────────
//
// Side-by-side static preview of up to 3 styles. Each column renders the same
// widget set using that style StyleSettings values directly - no global style
// switching, so the rest of the app is unaffected.

thread_local! {
    static PREVIEW_COLS: std::cell::RefCell<[u8; 3]> = const { std::cell::RefCell::new([0, 1, 2]) };
}

fn render_style_preview(ui: &mut Ui) {
    use crate::chart_renderer::ui::style::{get_style_settings, set_active_style, list_style_presets};

    let presets = list_style_presets();

    // Top row: 3 dropdowns for column style selection
    PREVIEW_COLS.with(|cols| {
        let mut cols = cols.borrow_mut();
        ui.horizontal(|ui| {
            for col_idx in 0..3 {
                let label = ["Col A", "Col B", "Col C"][col_idx];
                let selected_name = presets.iter()
                    .find(|(id, _)| *id == cols[col_idx])
                    .map(|(_, n)| n.as_str())
                    .unwrap_or("?");
                ui.label(RichText::new(label).monospace().size(9.0)
                    .color(Color32::from_rgb(140, 140, 150)));
                egui::ComboBox::from_id_salt(egui::Id::new(("preview_col", col_idx)))
                    .selected_text(RichText::new(selected_name).monospace().size(10.0))
                    .width(90.0)
                    .show_ui(ui, |ui| {
                        for (id, name) in &presets {
                            if ui.selectable_label(cols[col_idx] == *id, name).clicked() {
                                cols[col_idx] = *id;
                            }
                        }
                    });
                if col_idx < 2 { ui.add_space(6.0); }
            }
        });
    });

    ui.add_space(6.0);

    // Three-column preview area with horizontal scroll
    egui::ScrollArea::horizontal().show(ui, |ui| {
        ui.horizontal_top(|ui| {
            let col_ids: [u8; 3] = PREVIEW_COLS.with(|c| *c.borrow());

            for col_idx in 0..3usize {
                let style_id = col_ids[col_idx];
                let st = get_style_settings(style_id);
                let style_name = list_style_presets()
                    .into_iter()
                    .find(|(id, _)| *id == style_id)
                    .map(|(_, n)| n)
                    .unwrap_or_else(|| "?".to_string());

                // Vertical separator before columns 1 and 2
                if col_idx > 0 {
                    let sep_x = ui.cursor().left();
                    let sep_top = ui.cursor().top();
                    ui.painter().line_segment(
                        [egui::pos2(sep_x, sep_top),
                         egui::pos2(sep_x, sep_top + 700.0)],
                        Stroke::new(1.0, Color32::from_rgb(40, 42, 54)));
                    ui.add_space(1.0);
                }

                ui.vertical(|ui| {
                    ui.set_min_width(300.0);
                    ui.set_max_width(340.0);

                    // Column header with style name and Set Active button
                    egui::Frame::NONE
                        .fill(Color32::from_rgb(20, 20, 28))
                        .inner_margin(egui::Margin { left: 10, right: 10, top: 6, bottom: 6 })
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&style_name).monospace().size(12.0).strong()
                                    .color(Color32::from_rgb(203, 166, 247)));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.add(egui::Button::new(
                                        RichText::new("Set Active").monospace().size(9.0).strong()
                                            .color(Color32::from_rgb(166, 227, 161)))
                                        .fill(Color32::from_rgba_unmultiplied(166, 227, 161, 20))
                                        .stroke(Stroke::new(0.8, Color32::from_rgba_unmultiplied(166, 227, 161, 100)))
                                        .corner_radius(3.0)
                                    ).clicked() {
                                        set_active_style(style_id);
                                        STYLE_EDITOR_ACTIVE.store(style_id, std::sync::atomic::Ordering::Relaxed);
                                    }
                                });
                            });
                        });

                    // Widget preview area
                    egui::Frame::NONE
                        .fill(Color32::from_rgb(16, 16, 22))
                        .inner_margin(egui::Margin { left: 10, right: 10, top: 8, bottom: 12 })
                        .show(ui, |ui| {
                            preview_widgets(ui, &st);
                        });
                });
            }
        });
    });
}

/// Paint all preview widgets for one column using static StyleSettings values.
/// No global style switching - everything is drawn from `st` directly.
fn preview_widgets(ui: &mut Ui, st: &crate::chart_renderer::ui::style::StyleSettings) {
    let accent  = Color32::from_rgb(137, 180, 250);
    let text    = Color32::from_rgb(205, 210, 225);
    let dim     = Color32::from_rgb(120, 125, 140);
    let border  = Color32::from_rgb(50, 55, 70);
    let green   = Color32::from_rgb(166, 227, 161);
    let red     = Color32::from_rgb(243, 139, 168);
    let amber   = Color32::from_rgb(249, 226, 175);

    let r_sm = egui::CornerRadius::same(st.r_sm);
    let r_lg = egui::CornerRadius::same(st.r_lg);
    let sw   = st.stroke_std;

    // PaneHeader
    preview_section_label(ui, "PaneHeader", dim);
    {
        let hh = 28.0 * st.header_height_scale;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), hh), egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, egui::CornerRadius::ZERO, Color32::from_rgb(26, 28, 38));
        p.rect_stroke(rect, egui::CornerRadius::ZERO,
            Stroke::new(st.stroke_hair, preview_alpha(border, 80)), egui::StrokeKind::Outside);
        p.text(egui::pos2(rect.left() + 8.0, rect.center().y), egui::Align2::LEFT_CENTER,
            "AAPL", egui::FontId::monospace(11.0), accent);
        p.text(egui::pos2(rect.right() - 8.0, rect.center().y), egui::Align2::RIGHT_CENTER,
            "x", egui::FontId::monospace(10.0), dim);
    }
    ui.add_space(6.0);

    // Card
    preview_section_label(ui, "Card", dim);
    {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 52.0), egui::Sense::hover());
        let p = ui.painter();
        let r_md = egui::CornerRadius::same(st.r_md);
        p.rect_filled(rect, r_md, Color32::from_rgb(26, 28, 38));
        p.rect_stroke(rect, r_md, Stroke::new(sw, preview_alpha(border, 60)), egui::StrokeKind::Outside);
        let stripe = egui::Rect::from_min_max(rect.min, egui::pos2(rect.left() + 3.0, rect.bottom()));
        p.rect_filled(stripe, egui::CornerRadius { nw: st.r_md, sw: st.r_md, ne: 0, se: 0 }, accent);
        p.text(egui::pos2(rect.left() + 10.0, rect.top() + 10.0), egui::Align2::LEFT_TOP,
            "AAPL - 185.30", egui::FontId::monospace(11.0), text);
        p.text(egui::pos2(rect.left() + 10.0, rect.top() + 24.0), egui::Align2::LEFT_TOP,
            "1 call @ 1.45", egui::FontId::monospace(9.0), dim);
        p.text(egui::pos2(rect.right() - 8.0, rect.bottom() - 8.0), egui::Align2::RIGHT_BOTTOM,
            "OPEN", egui::FontId::monospace(8.0), green);
    }
    ui.add_space(6.0);

    // Section Label
    preview_section_label(ui, "SectionLabel", dim);
    {
        let lbl = if st.uppercase_section_labels { "POSITIONS" } else { "Positions" };
        ui.label(RichText::new(lbl).monospace().size(7.0).strong().color(dim));
    }
    ui.add_space(4.0);

    // Buttons
    preview_section_label(ui, "Buttons", dim);
    ui.horizontal_wrapped(|ui| {
        preview_btn(ui, "Primary",   accent, preview_alpha(accent, 40), r_sm, sw);
        preview_btn(ui, "Secondary", dim,    preview_alpha(dim,   25),  r_sm, sw);
        {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(50.0, 20.0), egui::Sense::hover());
            ui.painter().rect_stroke(rect, r_sm, Stroke::new(sw, preview_alpha(dim, 60)), egui::StrokeKind::Outside);
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER,
                "Ghost", egui::FontId::monospace(9.0), dim);
        }
        preview_btn(ui, "Delete", red, preview_alpha(red, 30), r_sm, sw);
    });
    ui.add_space(6.0);

    // PillButton
    preview_section_label(ui, "PillButton", dim);
    ui.horizontal(|ui| {
        let pill_r = egui::CornerRadius::same(st.r_pill);
        {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(58.0, 20.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, pill_r, preview_alpha(accent, 40));
            ui.painter().rect_stroke(rect, pill_r, Stroke::new(sw, accent), egui::StrokeKind::Outside);
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER,
                "Active", egui::FontId::monospace(9.0), accent);
        }
        ui.add_space(4.0);
        {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(58.0, 20.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, pill_r, preview_alpha(border, 20));
            ui.painter().rect_stroke(rect, pill_r, Stroke::new(sw, preview_alpha(border, 60)), egui::StrokeKind::Outside);
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER,
                "Idle", egui::FontId::monospace(9.0), dim);
        }
    });
    ui.add_space(6.0);

    // Status Dots
    preview_section_label(ui, "StatusDot", dim);
    ui.horizontal(|ui| {
        for (color, label) in [(green, "OK"), (red, "ERR"), (amber, "WARN"), (dim, "OFF")] {
            let dot_pos = egui::pos2(ui.cursor().left() + 5.0, ui.cursor().top() + 7.0);
            ui.painter().circle_filled(dot_pos, 4.0, color);
            ui.add_space(12.0);
            ui.label(RichText::new(label).monospace().size(9.0).color(color));
            ui.add_space(4.0);
        }
    });
    ui.add_space(6.0);

    // Tab Bar
    preview_section_label(ui, "TabBar", dim);
    {
        let aw = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(egui::vec2(aw, 22.0), egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, egui::CornerRadius::ZERO, Color32::from_rgb(20, 20, 30));
        let tab_labels = ["Chart", "Trades", "News"];
        let tab_w = aw / 3.0;
        for (i, lbl) in tab_labels.iter().enumerate() {
            let tab_rect = egui::Rect::from_min_size(
                egui::pos2(rect.left() + i as f32 * tab_w, rect.top()),
                egui::vec2(tab_w, 22.0));
            let is_active = i == 0;
            let fg = if is_active { accent } else { dim };
            p.text(tab_rect.center(), egui::Align2::CENTER_CENTER,
                *lbl, egui::FontId::monospace(10.0), fg);
            if is_active && st.show_active_tab_underline {
                p.line_segment(
                    [egui::pos2(tab_rect.left() + 2.0, tab_rect.bottom()),
                     egui::pos2(tab_rect.right() - 2.0, tab_rect.bottom())],
                    Stroke::new(2.0, accent));
            }
        }
    }
    ui.add_space(6.0);

    // Form Row
    preview_section_label(ui, "FormRow", dim);
    {
        let aw = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(egui::vec2(aw, 22.0), egui::Sense::hover());
        let p = ui.painter();
        let lw = 70.0;
        p.text(egui::pos2(rect.left() + lw - 4.0, rect.center().y),
            egui::Align2::RIGHT_CENTER, "Symbol", egui::FontId::monospace(9.0), dim);
        let inp = egui::Rect::from_min_max(
            egui::pos2(rect.left() + lw + 4.0, rect.top() + 2.0),
            egui::pos2(rect.right(), rect.bottom() - 2.0));
        p.rect_filled(inp, r_sm, Color32::from_rgb(18, 20, 28));
        p.rect_stroke(inp, r_sm, Stroke::new(st.stroke_thin, preview_alpha(border, 80)), egui::StrokeKind::Outside);
        p.text(egui::pos2(inp.left() + 6.0, inp.center().y),
            egui::Align2::LEFT_CENTER, "AAPL", egui::FontId::monospace(10.0), text);
    }
    ui.add_space(6.0);

    // Modal Dialog
    preview_section_label(ui, "Dialog", dim);
    {
        let aw = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(egui::vec2(aw, 70.0), egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, r_lg, Color32::from_rgb(24, 24, 34));
        p.rect_stroke(rect, r_lg, Stroke::new(sw, preview_alpha(border, 100)), egui::StrokeKind::Outside);
        let hdr = egui::Rect::from_min_size(rect.min, egui::vec2(aw, 24.0));
        p.rect_filled(hdr, egui::CornerRadius { nw: st.r_lg, ne: st.r_lg, sw: 0, se: 0 },
            Color32::from_rgb(18, 18, 28));
        p.text(egui::pos2(hdr.left() + 10.0, hdr.center().y), egui::Align2::LEFT_CENTER,
            "Confirm Order", egui::FontId::monospace(11.0), text);
        p.text(egui::pos2(hdr.right() - 8.0, hdr.center().y), egui::Align2::RIGHT_CENTER,
            "x", egui::FontId::monospace(10.0), dim);
        p.text(egui::pos2(rect.left() + 10.0, rect.top() + 34.0), egui::Align2::LEFT_TOP,
            "Buy 100 AAPL @ 185.30 limit", egui::FontId::monospace(9.0), dim);
        let btn_r = egui::Rect::from_min_size(
            egui::pos2(rect.right() - 60.0, rect.bottom() - 20.0), egui::vec2(52.0, 16.0));
        p.rect_filled(btn_r, r_sm, preview_alpha(green, 40));
        p.rect_stroke(btn_r, r_sm, Stroke::new(sw, green), egui::StrokeKind::Outside);
        p.text(btn_r.center(), egui::Align2::CENTER_CENTER,
            "Place", egui::FontId::monospace(9.0), green);
    }
    ui.add_space(6.0);

    // Tooltip
    preview_section_label(ui, "Tooltip", dim);
    {
        let tw = ui.available_width().min(180.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(tw, 38.0), egui::Sense::hover());
        let p = ui.painter();
        let tip_r = egui::CornerRadius::same(st.r_md.max(4));
        p.rect_filled(rect, tip_r, Color32::from_rgb(20, 20, 30));
        p.rect_stroke(rect, tip_r, Stroke::new(st.stroke_thin, preview_alpha(border, 100)), egui::StrokeKind::Outside);
        p.text(egui::pos2(rect.left() + 8.0, rect.top() + 8.0),
            egui::Align2::LEFT_TOP, "Volume", egui::FontId::monospace(8.0), dim);
        p.text(egui::pos2(rect.right() - 8.0, rect.top() + 8.0),
            egui::Align2::RIGHT_TOP, "1.23M", egui::FontId::monospace(10.0), text);
        p.text(egui::pos2(rect.left() + 8.0, rect.top() + 22.0),
            egui::Align2::LEFT_TOP, "Avg Vol", egui::FontId::monospace(8.0), dim);
        p.text(egui::pos2(rect.right() - 8.0, rect.top() + 22.0),
            egui::Align2::RIGHT_TOP, "980K", egui::FontId::monospace(10.0), text);
    }
    ui.add_space(4.0);
}

fn preview_btn(ui: &mut Ui, label: &str, fg: Color32, bg: Color32, cr: egui::CornerRadius, sw: f32) {
    let w = (label.len() as f32 * 6.5 + 16.0).max(48.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 20.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, cr, bg);
    ui.painter().rect_stroke(rect, cr,
        Stroke::new(sw, preview_alpha(fg, 150)), egui::StrokeKind::Outside);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER,
        label, egui::FontId::monospace(9.0), fg);
}

fn preview_section_label(ui: &mut Ui, text: &str, color: Color32) {
    ui.label(RichText::new(text).monospace().size(7.5).strong()
        .color(preview_alpha(color, 140)));
    ui.add_space(2.0);
}

#[inline(always)]
fn preview_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}