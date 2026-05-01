//! Design Preview pane — full-screen comprehensive preview of every
//! widget in the design system across selected styles. Use it side
//! by side with the inspector to iterate on the design.
//!
//! Renders statically: reads StyleSettings from `get_style_settings(id)`
//! and paints widget mocks without touching the global active style.

use egui::{self, Color32, RichText, Stroke};
use super::super::gpu::*;
use super::style::{get_style_settings, list_style_presets, StyleSettings};

const COL_W: f32 = 320.0;

// ── Entry point ──────────────────────────────────────────────────────────────

pub(crate) fn render(
    ui: &mut egui::Ui,
    _ctx: &egui::Context,
    panes: &mut [Chart],
    pane_idx: usize,
    _active_pane: &mut usize,
    _visible_count: usize,
    rects: &[egui::Rect],
    _theme_idx: usize,
    _watchlist: &mut Watchlist,
) {
    let rect = rects[0];
    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    draw(&mut child_ui, &mut panes[pane_idx]);
}

pub fn draw(ui: &mut egui::Ui, chart: &mut Chart) {
    let presets = list_style_presets();

    // ── Toolbar ──────────────────────────────────────────────────────────────
    ui.horizontal(|ui| {
        ui.add(egui::Label::new(
            RichText::new("Design Preview").monospace().size(10.0)
                .color(Color32::from_rgb(140, 140, 160))
        ));
        ui.separator();

        // Style column selectors
        let num_cols = chart.design_preview_styles.len();
        for col_idx in 0..num_cols {
            let selected_id = chart.design_preview_styles[col_idx];
            let selected_name = presets.iter()
                .find(|(id, _)| *id == selected_id)
                .map(|(_, n)| n.as_str())
                .unwrap_or("?");
            egui::ComboBox::from_id_salt(egui::Id::new(("dp_col", col_idx)))
                .selected_text(RichText::new(selected_name).monospace().size(9.5))
                .width(80.0)
                .show_ui(ui, |ui| {
                    for (id, name) in &presets {
                        let mut sid = selected_id;
                        if ui.selectable_value(&mut sid, *id, name).clicked() {
                            chart.design_preview_styles[col_idx] = *id;
                        }
                    }
                });
        }

        ui.separator();

        // Add / remove column
        if ui.small_button("+ Col").clicked() && chart.design_preview_styles.len() < 6 {
            let last = *chart.design_preview_styles.last().unwrap_or(&0);
            chart.design_preview_styles.push(last);
        }
        if ui.small_button("− Col").clicked() && chart.design_preview_styles.len() > 1 {
            chart.design_preview_styles.pop();
        }

        ui.separator();

        // Density selector
        ui.label(RichText::new("Density:").monospace().size(9.0)
            .color(Color32::from_rgb(120, 125, 140)));
        for (label, val) in [("Compact", 0u8), ("Normal", 1), ("Roomy", 2)] {
            let active = chart.design_preview_density == val;
            let fg = if active { Color32::from_rgb(137, 180, 250) } else { Color32::from_rgb(90, 95, 110) };
            if ui.add(egui::Button::new(RichText::new(label).monospace().size(9.0).color(fg))
                .fill(if active { Color32::from_rgba_unmultiplied(137, 180, 250, 25) } else { Color32::TRANSPARENT })
            ).clicked() {
                chart.design_preview_density = val;
            }
        }
    });

    ui.separator();

    // ── Column area ──────────────────────────────────────────────────────────
    let col_ids = chart.design_preview_styles.clone();
    let density = chart.design_preview_density;

    egui::ScrollArea::both()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                for (col_idx, &style_id) in col_ids.iter().enumerate() {
                    let st = get_style_settings(style_id);
                    let style_name = presets.iter()
                        .find(|(id, _)| *id == style_id)
                        .map(|(_, n)| n.clone())
                        .unwrap_or_else(|| "?".to_string());

                    // Vertical separator
                    if col_idx > 0 {
                        ui.separator();
                    }

                    ui.vertical(|ui| {
                        ui.set_min_width(COL_W);
                        ui.set_max_width(COL_W);

                        // Column header
                        egui::Frame::NONE
                            .fill(Color32::from_rgb(22, 22, 32))
                            .inner_margin(egui::Margin { left: 10, right: 10, top: 6, bottom: 6 })
                            .show(ui, |ui| {
                                ui.label(RichText::new(&style_name)
                                    .monospace().size(12.0).strong()
                                    .color(Color32::from_rgb(203, 166, 247)));
                            });

                        egui::ScrollArea::vertical()
                            .id_salt(egui::Id::new(("dp_col_scroll", col_idx)))
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                egui::Frame::NONE
                                    .fill(Color32::from_rgb(14, 14, 20))
                                    .inner_margin(egui::Margin { left: 12, right: 12, top: 10, bottom: 16 })
                                    .show(ui, |ui| {
                                        draw_column_widgets(ui, &st, density);
                                    });
                            });
                    });
                }
            });
        });
}

// ── Widget catalogue ─────────────────────────────────────────────────────────

fn draw_column_widgets(ui: &mut egui::Ui, st: &StyleSettings, density: u8) {
    let gap = match density { 0 => 3.0, 2 => 10.0, _ => 6.0 };

    let accent = Color32::from_rgb(137, 180, 250);
    let text   = Color32::from_rgb(205, 210, 225);
    let dim    = Color32::from_rgb(120, 125, 140);
    let border = Color32::from_rgb(50, 55, 70);
    let green  = Color32::from_rgb(166, 227, 161);
    let red    = Color32::from_rgb(243, 139, 168);
    let amber  = Color32::from_rgb(249, 226, 175);
    let purple = Color32::from_rgb(203, 166, 247);

    let r_sm = egui::CornerRadius::same(st.r_sm);
    let r_md = egui::CornerRadius::same(st.r_md);
    let r_lg = egui::CornerRadius::same(st.r_lg);
    let r_pill = egui::CornerRadius::same(st.r_pill);
    let sw   = st.stroke_std;
    let aw   = ui.available_width();

    // ── 1. PaneHeader ────────────────────────────────────────────────────────
    section(ui, "PaneHeader", dim);
    {
        let hh = 28.0 * st.header_height_scale;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(aw, hh), egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, egui::CornerRadius::ZERO, Color32::from_rgb(26, 28, 38));
        p.rect_stroke(rect, egui::CornerRadius::ZERO,
            Stroke::new(st.stroke_hair, fa(border, 80)), egui::StrokeKind::Outside);
        p.text(egui::pos2(rect.left() + 8.0, rect.center().y), egui::Align2::LEFT_CENTER,
            "AAPL  1D", egui::FontId::monospace(11.0), accent);
        p.text(egui::pos2(rect.right() - 8.0, rect.center().y), egui::Align2::RIGHT_CENTER,
            "⊞ ×", egui::FontId::monospace(10.0), dim);
    }
    ui.add_space(gap);

    // ── 2. Tabs / TabBar ─────────────────────────────────────────────────────
    section(ui, "TabBar", dim);
    {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(aw, 22.0), egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, egui::CornerRadius::ZERO, Color32::from_rgb(20, 20, 30));
        let tabs = ["Chart", "Trades", "News", "Alerts"];
        let tab_w = aw / tabs.len() as f32;
        for (i, lbl) in tabs.iter().enumerate() {
            let tr = egui::Rect::from_min_size(
                egui::pos2(rect.left() + i as f32 * tab_w, rect.top()),
                egui::vec2(tab_w, 22.0));
            let active = i == 0;
            let fg = if active { accent } else { dim };
            p.text(tr.center(), egui::Align2::CENTER_CENTER,
                *lbl, egui::FontId::monospace(10.0), fg);
            if active && st.show_active_tab_underline {
                p.line_segment(
                    [egui::pos2(tr.left() + 2.0, tr.bottom()),
                     egui::pos2(tr.right() - 2.0, tr.bottom())],
                    Stroke::new(2.0, accent));
            }
        }
    }
    ui.add_space(gap);

    // ── 3. Buttons ───────────────────────────────────────────────────────────
    section(ui, "Buttons", dim);
    ui.horizontal_wrapped(|ui| {
        btn(ui, "Primary",     accent, fa(accent, 40), r_sm, sw);
        btn(ui, "Secondary",   dim,    fa(dim, 25),    r_sm, sw);
        ghost_btn(ui, "Ghost", dim,    r_sm, sw);
        btn(ui, "Destructive", red,    fa(red, 30),    r_sm, sw);
        btn(ui, "Buy",         green,  fa(green, 35),  r_sm, sw);
        btn(ui, "Sell",        red,    fa(red, 35),    r_sm, sw);
    });
    ui.add_space(gap);

    // ── 4. ToolbarBtn / IconBtn / ChromeBtn ──────────────────────────────────
    section(ui, "ToolbarBtn / IconBtn / ChromeBtn", dim);
    ui.horizontal(|ui| {
        // ToolbarBtn: icon-only compact button
        for icon in ["◎", "⊕", "⌖", "⊞"] {
            let (r, _) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::hover());
            ui.painter().rect_filled(r, r_sm, fa(border, 20));
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER,
                icon, egui::FontId::monospace(11.0), dim);
        }
        ui.add_space(6.0);
        // ChromeBtn: window-chrome style
        for (icon, col) in [("−", dim), ("□", dim), ("×", red)] {
            let (r, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
            ui.painter().rect_filled(r, egui::CornerRadius::ZERO, fa(border, 15));
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER,
                icon, egui::FontId::monospace(10.0), col);
        }
    });
    ui.add_space(gap);

    // ── 5. Pills / Chips / Badges ────────────────────────────────────────────
    section(ui, "Pills / Chips / Badges", dim);
    ui.horizontal_wrapped(|ui| {
        // Active pill
        let (r, _) = ui.allocate_exact_size(egui::vec2(60.0, 20.0), egui::Sense::hover());
        ui.painter().rect_filled(r, r_pill, fa(accent, 40));
        ui.painter().rect_stroke(r, r_pill, Stroke::new(sw, accent), egui::StrokeKind::Outside);
        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER,
            "Active", egui::FontId::monospace(9.0), accent);
        // Idle pill
        let (r, _) = ui.allocate_exact_size(egui::vec2(50.0, 20.0), egui::Sense::hover());
        ui.painter().rect_filled(r, r_pill, fa(border, 18));
        ui.painter().rect_stroke(r, r_pill, Stroke::new(sw, fa(border, 60)), egui::StrokeKind::Outside);
        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER,
            "Idle", egui::FontId::monospace(9.0), dim);
        // Chip: colored badge
        for (lbl, col) in [("BUY", green), ("SELL", red), ("HOLD", amber)] {
            let (r, _) = ui.allocate_exact_size(egui::vec2(38.0, 16.0), egui::Sense::hover());
            ui.painter().rect_filled(r, r_sm, fa(col, 35));
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER,
                lbl, egui::FontId::monospace(8.0), col);
        }
        // Count badge
        let (r, _) = ui.allocate_exact_size(egui::vec2(20.0, 16.0), egui::Sense::hover());
        ui.painter().circle_filled(r.center(), 8.0, fa(red, 200));
        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER,
            "3", egui::FontId::monospace(8.0), Color32::WHITE);
    });
    ui.add_space(gap);

    // ── 6. Status Dots ───────────────────────────────────────────────────────
    section(ui, "StatusDot", dim);
    ui.horizontal(|ui| {
        for (color, label) in [(green, "OK"), (red, "ERR"), (amber, "WARN"), (dim, "OFF"), (accent, "CONN")] {
            let dot_pos = egui::pos2(ui.cursor().left() + 5.0, ui.cursor().top() + 7.0);
            ui.painter().circle_filled(dot_pos, 3.5, color);
            ui.add_space(12.0);
            ui.label(RichText::new(label).monospace().size(8.5).color(color));
            ui.add_space(3.0);
        }
    });
    ui.add_space(gap);

    // ── 7. SectionLabel / Headers / Captions ─────────────────────────────────
    section(ui, "SectionLabel / Headers / Captions", dim);
    {
        let lbl = if st.uppercase_section_labels { "POSITIONS" } else { "Positions" };
        ui.label(RichText::new(lbl).monospace().size(7.0).strong().color(dim));
    }
    ui.label(RichText::new("Chart Title H1").monospace().size(14.0).strong().color(text));
    ui.label(RichText::new("Subtitle / H2").monospace().size(11.0).color(fa(text, 200)));
    ui.label(RichText::new("Caption / helper text").monospace().size(8.5).color(dim));
    ui.add_space(gap);

    // ── 8. Card (Bordered / Elevated / Ghost / Footer) ───────────────────────
    section(ui, "Card variants", dim);
    // Bordered
    {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(aw, 52.0), egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, r_md, Color32::from_rgb(26, 28, 38));
        p.rect_stroke(rect, r_md, Stroke::new(sw, fa(border, 60)), egui::StrokeKind::Outside);
        let stripe = egui::Rect::from_min_max(rect.min, egui::pos2(rect.left() + 3.0, rect.bottom()));
        p.rect_filled(stripe, egui::CornerRadius { nw: st.r_md, sw: st.r_md, ne: 0, se: 0 }, accent);
        p.text(egui::pos2(rect.left() + 10.0, rect.top() + 10.0), egui::Align2::LEFT_TOP,
            "AAPL — 185.30", egui::FontId::monospace(11.0), text);
        p.text(egui::pos2(rect.left() + 10.0, rect.top() + 24.0), egui::Align2::LEFT_TOP,
            "1 call @ 1.45", egui::FontId::monospace(9.0), dim);
        p.text(egui::pos2(rect.right() - 8.0, rect.bottom() - 8.0), egui::Align2::RIGHT_BOTTOM,
            "OPEN", egui::FontId::monospace(8.0), green);
    }
    ui.add_space(3.0);
    // Elevated (shadow hint via double border)
    {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(aw, 36.0), egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect.translate(egui::vec2(2.0, 2.0)), r_md, fa(Color32::BLACK, 60));
        p.rect_filled(rect, r_md, Color32::from_rgb(30, 30, 42));
        p.rect_stroke(rect, r_md, Stroke::new(sw, fa(border, 40)), egui::StrokeKind::Outside);
        p.text(egui::pos2(rect.left() + 10.0, rect.center().y), egui::Align2::LEFT_CENTER,
            "Elevated card", egui::FontId::monospace(10.0), text);
    }
    ui.add_space(3.0);
    // Ghost
    {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(aw, 30.0), egui::Sense::hover());
        ui.painter().rect_stroke(rect, r_md, Stroke::new(st.stroke_hair, fa(border, 50)), egui::StrokeKind::Outside);
        ui.painter().text(egui::pos2(rect.left() + 10.0, rect.center().y), egui::Align2::LEFT_CENTER,
            "Ghost card", egui::FontId::monospace(10.0), fa(text, 160));
    }
    ui.add_space(gap);

    // ── 9. Rows (List / Order / Watchlist / News / Alert / DOM) ──────────────
    section(ui, "Rows (List / Order / Watchlist / News / Alert / DOM)", dim);
    // Watchlist row
    row(ui, aw, Color32::from_rgb(18, 20, 28), |p, r| {
        p.text(egui::pos2(r.left() + 8.0, r.center().y), egui::Align2::LEFT_CENTER,
            "AAPL", egui::FontId::monospace(10.0), accent);
        p.text(egui::pos2(r.center().x, r.center().y), egui::Align2::CENTER_CENTER,
            "185.30", egui::FontId::monospace(10.0), text);
        p.text(egui::pos2(r.right() - 8.0, r.center().y), egui::Align2::RIGHT_CENTER,
            "+1.24%", egui::FontId::monospace(9.0), green);
    });
    // Order row
    row(ui, aw, Color32::from_rgb(20, 22, 30), |p, r| {
        p.text(egui::pos2(r.left() + 8.0, r.center().y), egui::Align2::LEFT_CENTER,
            "BUY 100 AAPL LMT 184.00", egui::FontId::monospace(9.0), text);
        let badge = egui::Rect::from_min_size(egui::pos2(r.right() - 52.0, r.center().y - 7.0), egui::vec2(44.0, 14.0));
        p.rect_filled(badge, r_sm, fa(amber, 35));
        p.text(badge.center(), egui::Align2::CENTER_CENTER,
            "WORKING", egui::FontId::monospace(7.5), amber);
    });
    // News row
    row(ui, aw, Color32::from_rgb(16, 18, 26), |p, r| {
        p.text(egui::pos2(r.left() + 8.0, r.center().y - 4.0), egui::Align2::LEFT_CENTER,
            "Fed holds rates steady — markets react", egui::FontId::monospace(9.0), text);
        p.text(egui::pos2(r.left() + 8.0, r.center().y + 6.0), egui::Align2::LEFT_CENTER,
            "Reuters · 3m ago", egui::FontId::monospace(7.5), dim);
    });
    // DOM row
    row(ui, aw, Color32::from_rgb(18, 22, 28), |p, r| {
        p.text(egui::pos2(r.left() + 8.0, r.center().y), egui::Align2::LEFT_CENTER,
            "300", egui::FontId::monospace(9.0), fa(green, 180));
        p.text(egui::pos2(r.center().x, r.center().y), egui::Align2::CENTER_CENTER,
            "185.28", egui::FontId::monospace(10.0), text);
        p.text(egui::pos2(r.right() - 8.0, r.center().y), egui::Align2::RIGHT_CENTER,
            "120", egui::FontId::monospace(9.0), fa(red, 180));
    });
    ui.add_space(gap);

    // ── 10. Forms (FormRow / TextInput / NumericInput / SearchInput / Toggle / Select) ──
    section(ui, "Forms", dim);
    // TextInput
    form_row(ui, aw, "Symbol", |p, inp, r_inp| {
        p.rect_filled(inp, r_sm, Color32::from_rgb(18, 20, 28));
        p.rect_stroke(inp, r_sm, Stroke::new(sw * 0.5, fa(border, 80)), egui::StrokeKind::Outside);
        p.text(egui::pos2(inp.left() + 6.0, inp.center().y), egui::Align2::LEFT_CENTER,
            "AAPL", egui::FontId::monospace(10.0), text);
        let _ = r_inp;
    });
    // Numeric input
    form_row(ui, aw, "Qty", |p, inp, _| {
        p.rect_filled(inp, r_sm, Color32::from_rgb(18, 20, 28));
        p.rect_stroke(inp, r_sm, Stroke::new(sw * 0.5, fa(border, 80)), egui::StrokeKind::Outside);
        p.text(egui::pos2(inp.left() + 6.0, inp.center().y), egui::Align2::LEFT_CENTER,
            "100", egui::FontId::monospace(10.0), accent);
        // Stepper arrows
        p.text(egui::pos2(inp.right() - 10.0, inp.center().y - 3.0), egui::Align2::CENTER_CENTER,
            "▴", egui::FontId::monospace(7.0), dim);
        p.text(egui::pos2(inp.right() - 10.0, inp.center().y + 4.0), egui::Align2::CENTER_CENTER,
            "▾", egui::FontId::monospace(7.0), dim);
    });
    // Search input
    {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(aw, 22.0), egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, r_sm, Color32::from_rgb(18, 20, 28));
        p.rect_stroke(rect, r_sm, Stroke::new(sw * 0.5, fa(accent, 100)), egui::StrokeKind::Outside);
        p.text(egui::pos2(rect.left() + 6.0, rect.center().y), egui::Align2::LEFT_CENTER,
            "⌕  Search...", egui::FontId::monospace(9.5), dim);
    }
    ui.add_space(3.0);
    // Toggle
    ui.horizontal(|ui| {
        ui.label(RichText::new("Toggle").monospace().size(9.0).color(dim));
        ui.add_space(6.0);
        // ON
        let (r, _) = ui.allocate_exact_size(egui::vec2(30.0, 16.0), egui::Sense::hover());
        ui.painter().rect_filled(r, egui::CornerRadius::same(8), fa(accent, 200));
        ui.painter().circle_filled(egui::pos2(r.right() - 9.0, r.center().y), 6.0, Color32::WHITE);
        ui.add_space(4.0);
        ui.label(RichText::new("ON").monospace().size(8.5).color(accent));
        ui.add_space(10.0);
        // OFF
        let (r, _) = ui.allocate_exact_size(egui::vec2(30.0, 16.0), egui::Sense::hover());
        ui.painter().rect_filled(r, egui::CornerRadius::same(8), fa(border, 120));
        ui.painter().circle_filled(egui::pos2(r.left() + 9.0, r.center().y), 6.0, fa(text, 180));
        ui.add_space(4.0);
        ui.label(RichText::new("OFF").monospace().size(8.5).color(dim));
    });
    ui.add_space(3.0);
    // Select dropdown
    {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(aw, 22.0), egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, r_sm, Color32::from_rgb(22, 24, 32));
        p.rect_stroke(rect, r_sm, Stroke::new(sw * 0.5, fa(border, 80)), egui::StrokeKind::Outside);
        p.text(egui::pos2(rect.left() + 8.0, rect.center().y), egui::Align2::LEFT_CENTER,
            "Limit", egui::FontId::monospace(9.5), text);
        p.text(egui::pos2(rect.right() - 8.0, rect.center().y), egui::Align2::RIGHT_CENTER,
            "▾", egui::FontId::monospace(9.0), dim);
    }
    ui.add_space(gap);

    // ── 11. Sliders / Steppers ───────────────────────────────────────────────
    section(ui, "Slider / Stepper", dim);
    // Slider
    {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(aw, 18.0), egui::Sense::hover());
        let p = ui.painter();
        let track = egui::Rect::from_center_size(rect.center(), egui::vec2(aw - 16.0, 4.0));
        p.rect_filled(track, egui::CornerRadius::same(2), fa(border, 100));
        let fill_w = track.width() * 0.62;
        let fill = egui::Rect::from_min_size(track.min, egui::vec2(fill_w, 4.0));
        p.rect_filled(fill, egui::CornerRadius::same(2), accent);
        let thumb_x = track.min.x + fill_w;
        p.circle_filled(egui::pos2(thumb_x, track.center().y), 7.0, Color32::WHITE);
        p.circle_stroke(egui::pos2(thumb_x, track.center().y), 7.0, Stroke::new(1.5, accent));
    }
    // Stepper
    ui.horizontal(|ui| {
        for lbl in ["−", "100", "+"] {
            let w = if lbl == "100" { 40.0 } else { 20.0 };
            let (r, _) = ui.allocate_exact_size(egui::vec2(w, 20.0), egui::Sense::hover());
            let bg = if lbl == "100" { Color32::from_rgb(18, 20, 28) } else { fa(border, 30) };
            ui.painter().rect_filled(r, r_sm, bg);
            ui.painter().rect_stroke(r, r_sm, Stroke::new(sw * 0.5, fa(border, 60)), egui::StrokeKind::Outside);
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER,
                lbl, egui::FontId::monospace(10.0), if lbl == "100" { text } else { accent });
        }
    });
    ui.add_space(gap);

    // ── 12. Tables ───────────────────────────────────────────────────────────
    section(ui, "Table", dim);
    {
        // Header row
        let col_ws = [70.0_f32, 60.0, 60.0, aw - 70.0 - 60.0 - 60.0];
        let header_cols = ["Symbol", "Price", "Change", "Volume"];
        let (hr, _) = ui.allocate_exact_size(egui::vec2(aw, 18.0), egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(hr, egui::CornerRadius::ZERO, Color32::from_rgb(24, 24, 34));
        let mut x = hr.left();
        for (i, lbl) in header_cols.iter().enumerate() {
            let col_r = egui::Rect::from_min_size(egui::pos2(x, hr.top()), egui::vec2(col_ws[i], hr.height()));
            p.text(egui::pos2(col_r.left() + 4.0, col_r.center().y), egui::Align2::LEFT_CENTER,
                *lbl, egui::FontId::monospace(8.0), dim);
            x += col_ws[i];
        }
        // Data rows
        let table_data = [
            ("AAPL", "185.30", "+1.2%", "12.3M"),
            ("TSLA", "248.50", "-0.8%", "8.7M"),
            ("NVDA", "820.00", "+2.5%", "22.1M"),
        ];
        for (row_i, (sym, price, chg, vol)) in table_data.iter().enumerate() {
            let bg = if row_i % 2 == 0 { Color32::from_rgb(16, 16, 24) } else { Color32::from_rgb(19, 19, 28) };
            let (rr, _) = ui.allocate_exact_size(egui::vec2(aw, 18.0), egui::Sense::hover());
            let p = ui.painter();
            p.rect_filled(rr, egui::CornerRadius::ZERO, bg);
            let chg_col = if chg.starts_with('+') { green } else { red };
            let vals: [(&str, Color32); 4] = [(sym, accent), (price, text), (chg, chg_col), (vol, dim)];
            let mut x2 = rr.left();
            for (i, (v, c)) in vals.iter().enumerate() {
                p.text(egui::pos2(x2 + 4.0, rr.center().y), egui::Align2::LEFT_CENTER,
                    *v, egui::FontId::monospace(9.0), *c);
                x2 += col_ws[i];
            }
        }
    }
    ui.add_space(gap);

    // ── 13. Modal / Dialog ───────────────────────────────────────────────────
    section(ui, "Dialog / Modal", dim);
    {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(aw, 80.0), egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, r_lg, Color32::from_rgb(24, 24, 34));
        p.rect_stroke(rect, r_lg, Stroke::new(sw, fa(border, 100)), egui::StrokeKind::Outside);
        let hdr = egui::Rect::from_min_size(rect.min, egui::vec2(aw, 24.0));
        p.rect_filled(hdr, egui::CornerRadius { nw: st.r_lg, ne: st.r_lg, sw: 0, se: 0 },
            Color32::from_rgb(18, 18, 28));
        p.text(egui::pos2(hdr.left() + 10.0, hdr.center().y), egui::Align2::LEFT_CENTER,
            "Confirm Order", egui::FontId::monospace(11.0), text);
        p.text(egui::pos2(hdr.right() - 8.0, hdr.center().y), egui::Align2::RIGHT_CENTER,
            "×", egui::FontId::monospace(10.0), dim);
        p.text(egui::pos2(rect.left() + 10.0, rect.top() + 34.0), egui::Align2::LEFT_TOP,
            "Buy 100 AAPL @ 185.30 limit", egui::FontId::monospace(9.0), dim);
        let cancel_r = egui::Rect::from_min_size(
            egui::pos2(rect.right() - 120.0, rect.bottom() - 22.0), egui::vec2(52.0, 16.0));
        let place_r = egui::Rect::from_min_size(
            egui::pos2(rect.right() - 62.0, rect.bottom() - 22.0), egui::vec2(52.0, 16.0));
        p.rect_stroke(cancel_r, r_sm, Stroke::new(sw * 0.5, fa(dim, 80)), egui::StrokeKind::Outside);
        p.text(cancel_r.center(), egui::Align2::CENTER_CENTER,
            "Cancel", egui::FontId::monospace(8.5), dim);
        p.rect_filled(place_r, r_sm, fa(green, 40));
        p.rect_stroke(place_r, r_sm, Stroke::new(sw, green), egui::StrokeKind::Outside);
        p.text(place_r.center(), egui::Align2::CENTER_CENTER,
            "Place", egui::FontId::monospace(8.5), green);
    }
    ui.add_space(gap);

    // ── 14. Tooltip ──────────────────────────────────────────────────────────
    section(ui, "Tooltip", dim);
    {
        let tw = aw.min(200.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(tw, 48.0), egui::Sense::hover());
        let p = ui.painter();
        let tip_r = egui::CornerRadius::same(st.r_md.max(4));
        p.rect_filled(rect, tip_r, Color32::from_rgb(20, 20, 30));
        p.rect_stroke(rect, tip_r, Stroke::new(st.stroke_thin, fa(border, 100)), egui::StrokeKind::Outside);
        for (i, (k, v)) in [("Volume", "1.23M"), ("Avg Vol", "980K"), ("Float", "15.4B")].iter().enumerate() {
            let y = rect.top() + 8.0 + i as f32 * 13.0;
            p.text(egui::pos2(rect.left() + 8.0, y), egui::Align2::LEFT_TOP,
                k, egui::FontId::monospace(8.0), dim);
            p.text(egui::pos2(rect.right() - 8.0, y), egui::Align2::RIGHT_TOP,
                v, egui::FontId::monospace(9.0), text);
        }
    }
    ui.add_space(gap);

    // ── 15. SectionLabel variants ────────────────────────────────────────────
    section(ui, "SectionLabel", dim);
    {
        let lbl = if st.uppercase_section_labels { "POSITIONS" } else { "Positions" };
        ui.label(RichText::new(lbl).monospace().size(7.0).strong().color(dim));
        ui.add_space(2.0);
        let lbl2 = if st.uppercase_section_labels { "ORDER HISTORY" } else { "Order History" };
        // With accent rule
        let (r, _) = ui.allocate_exact_size(egui::vec2(aw, 14.0), egui::Sense::hover());
        ui.painter().text(egui::pos2(r.left(), r.center().y), egui::Align2::LEFT_CENTER,
            lbl2, egui::FontId::monospace(7.0), dim);
        ui.painter().line_segment(
            [egui::pos2(r.left() + 72.0, r.center().y), egui::pos2(r.right(), r.center().y)],
            Stroke::new(st.stroke_hair, fa(border, 40)));
    }
    ui.add_space(gap);

    // ── 16. Alert row ────────────────────────────────────────────────────────
    section(ui, "Alert row", dim);
    row(ui, aw, fa(amber, 10), |p, r| {
        let bar = egui::Rect::from_min_size(r.min, egui::vec2(3.0, r.height()));
        p.rect_filled(bar, egui::CornerRadius::ZERO, amber);
        p.text(egui::pos2(r.left() + 10.0, r.center().y - 4.0), egui::Align2::LEFT_CENTER,
            "AAPL  ≥ 186.00", egui::FontId::monospace(9.0), text);
        p.text(egui::pos2(r.left() + 10.0, r.center().y + 5.0), egui::Align2::LEFT_CENTER,
            "Price alert · Active", egui::FontId::monospace(7.5), dim);
        p.text(egui::pos2(r.right() - 8.0, r.center().y), egui::Align2::RIGHT_CENTER,
            "×", egui::FontId::monospace(10.0), fa(dim, 150));
    });
    ui.add_space(gap);

    // ── 17. Trade row (P&L) ──────────────────────────────────────────────────
    section(ui, "Trade Row (P&L)", dim);
    row(ui, aw, Color32::from_rgb(16, 18, 26), |p, r| {
        p.text(egui::pos2(r.left() + 8.0, r.center().y), egui::Align2::LEFT_CENTER,
            "TSLA  2025-04-28", egui::FontId::monospace(9.0), dim);
        p.text(egui::pos2(r.center().x, r.center().y), egui::Align2::CENTER_CENTER,
            "Long 50 @ 242.10", egui::FontId::monospace(9.0), text);
        p.text(egui::pos2(r.right() - 8.0, r.center().y), egui::Align2::RIGHT_CENTER,
            "+$315.00 (+2.6%)", egui::FontId::monospace(9.0), green);
    });
    ui.add_space(gap);

    // ── 18. Progress / Loading ───────────────────────────────────────────────
    section(ui, "Progress / Spinner", dim);
    {
        // Progress bar
        let (rect, _) = ui.allocate_exact_size(egui::vec2(aw, 8.0), egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, egui::CornerRadius::same(4), fa(border, 60));
        let fill = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width() * 0.72, 8.0));
        p.rect_filled(fill, egui::CornerRadius::same(4), accent);
        ui.add_space(3.0);
        // Skeleton placeholder
        let (rect2, _) = ui.allocate_exact_size(egui::vec2(aw * 0.6, 10.0), egui::Sense::hover());
        ui.painter().rect_filled(rect2, egui::CornerRadius::same(3), fa(border, 50));
    }
    ui.add_space(gap);

    // ── 19. Color palette swatches ───────────────────────────────────────────
    section(ui, "Color Palette", dim);
    ui.horizontal(|ui| {
        for col in [accent, green, red, amber, purple, dim, text, border] {
            let (r, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
            ui.painter().rect_filled(r, r_sm, col);
        }
    });
    ui.add_space(gap);

    // ── 20. Kbd / Tag / Monospace ────────────────────────────────────────────
    section(ui, "Kbd / Tag / MonospaceCode", dim);
    ui.horizontal(|ui| {
        for key in ["Ctrl", "Shift", "K"] {
            let (r, _) = ui.allocate_exact_size(
                egui::vec2(key.len() as f32 * 5.5 + 10.0, 16.0), egui::Sense::hover());
            ui.painter().rect_filled(r, r_sm, fa(border, 40));
            ui.painter().rect_stroke(r, r_sm, Stroke::new(sw * 0.5, fa(border, 80)), egui::StrokeKind::Outside);
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER,
                key, egui::FontId::monospace(8.0), text);
            if key != "K" { ui.label(RichText::new("+").monospace().size(8.5).color(dim)); }
        }
        ui.add_space(8.0);
        // MonospaceCode
        let (r, _) = ui.allocate_exact_size(egui::vec2(70.0, 16.0), egui::Sense::hover());
        ui.painter().rect_filled(r, r_sm, Color32::from_rgb(18, 18, 28));
        ui.painter().text(egui::pos2(r.left() + 4.0, r.center().y), egui::Align2::LEFT_CENTER,
            "println!()", egui::FontId::monospace(8.0), purple);
    });
    ui.add_space(gap);

    // ── 21. Notification / Toast ─────────────────────────────────────────────
    section(ui, "Notification / Toast", dim);
    {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(aw, 40.0), egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, r_md, Color32::from_rgb(22, 26, 34));
        p.rect_stroke(rect, r_md, Stroke::new(sw, fa(accent, 80)), egui::StrokeKind::Outside);
        let bar = egui::Rect::from_min_size(rect.min, egui::vec2(3.0, rect.height()));
        p.rect_filled(bar, egui::CornerRadius { nw: st.r_md, sw: st.r_md, ne: 0, se: 0 }, accent);
        p.text(egui::pos2(rect.left() + 10.0, rect.top() + 10.0), egui::Align2::LEFT_TOP,
            "Order filled: BUY 100 AAPL", egui::FontId::monospace(10.0), text);
        p.text(egui::pos2(rect.left() + 10.0, rect.top() + 24.0), egui::Align2::LEFT_TOP,
            "@ 185.32 · just now", egui::FontId::monospace(8.0), dim);
        p.text(egui::pos2(rect.right() - 8.0, rect.top() + 8.0), egui::Align2::RIGHT_TOP,
            "×", egui::FontId::monospace(10.0), dim);
    }
    ui.add_space(gap);

    // ── 22. Breadcrumb / Nav ─────────────────────────────────────────────────
    section(ui, "Breadcrumb / Nav", dim);
    ui.horizontal(|ui| {
        for (i, crumb) in ["Portfolio", "Positions", "AAPL"].iter().enumerate() {
            if i > 0 { ui.label(RichText::new("›").monospace().size(9.0).color(fa(dim, 120))); }
            let col = if i == 2 { text } else { dim };
            ui.label(RichText::new(*crumb).monospace().size(9.0).color(col));
        }
    });
    ui.add_space(gap);

    // ── 23. Metric / KPI tile ────────────────────────────────────────────────
    section(ui, "Metric / KPI tile", dim);
    ui.horizontal(|ui| {
        for (label, value, col) in [("Day P&L", "+$1,245", green), ("Win Rate", "68%", accent), ("Drawdown", "−2.1%", red)] {
            let (rect, _) = ui.allocate_exact_size(egui::vec2((aw - 8.0) / 3.0, 40.0), egui::Sense::hover());
            let p = ui.painter();
            p.rect_filled(rect, r_md, Color32::from_rgb(20, 22, 32));
            p.rect_stroke(rect, r_md, Stroke::new(sw * 0.5, fa(border, 50)), egui::StrokeKind::Outside);
            p.text(egui::pos2(rect.center().x, rect.top() + 10.0), egui::Align2::CENTER_TOP,
                label, egui::FontId::monospace(7.5), dim);
            p.text(egui::pos2(rect.center().x, rect.bottom() - 8.0), egui::Align2::CENTER_BOTTOM,
                value, egui::FontId::monospace(12.0), col);
            ui.add_space(4.0);
        }
    });
    ui.add_space(gap);

    // ── 24. Separator / Divider variants ─────────────────────────────────────
    section(ui, "Separator / Divider", dim);
    {
        let (r, _) = ui.allocate_exact_size(egui::vec2(aw, 1.0), egui::Sense::hover());
        ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(st.stroke_hair, fa(border, 80)));
        ui.add_space(4.0);
        let (r2, _) = ui.allocate_exact_size(egui::vec2(aw, 1.0), egui::Sense::hover());
        ui.painter().line_segment([r2.left_center(), r2.right_center()], Stroke::new(sw, fa(accent, 60)));
    }
    ui.add_space(gap);

    // ── 25. Empty state ──────────────────────────────────────────────────────
    section(ui, "Empty State", dim);
    {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(aw, 52.0), egui::Sense::hover());
        let p = ui.painter();
        p.rect_stroke(rect, r_md, Stroke::new(st.stroke_hair, fa(border, 40)), egui::StrokeKind::Outside);
        p.text(rect.center() - egui::vec2(0.0, 8.0), egui::Align2::CENTER_CENTER,
            "∅", egui::FontId::monospace(18.0), fa(dim, 80));
        p.text(rect.center() + egui::vec2(0.0, 10.0), egui::Align2::CENTER_CENTER,
            "No positions yet", egui::FontId::monospace(9.0), fa(dim, 120));
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

#[inline]
fn fa(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

fn section(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.label(RichText::new(text).monospace().size(7.5).strong().color(fa(color, 140)));
    ui.add_space(2.0);
}

fn btn(ui: &mut egui::Ui, label: &str, fg: Color32, bg: Color32, cr: egui::CornerRadius, sw: f32) {
    let w = (label.len() as f32 * 6.0 + 14.0).max(44.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 20.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, cr, bg);
    ui.painter().rect_stroke(rect, cr,
        Stroke::new(sw, fa(fg, 150)), egui::StrokeKind::Outside);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER,
        label, egui::FontId::monospace(9.0), fg);
}

fn ghost_btn(ui: &mut egui::Ui, label: &str, dim: Color32, cr: egui::CornerRadius, sw: f32) {
    let w = (label.len() as f32 * 6.0 + 14.0).max(44.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 20.0), egui::Sense::hover());
    ui.painter().rect_stroke(rect, cr, Stroke::new(sw, fa(dim, 70)), egui::StrokeKind::Outside);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER,
        label, egui::FontId::monospace(9.0), dim);
}

fn row(ui: &mut egui::Ui, aw: f32, bg: Color32, paint: impl Fn(&egui::Painter, egui::Rect)) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(aw, 28.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, egui::CornerRadius::ZERO, bg);
    ui.painter().line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(0.5, Color32::from_rgb(35, 38, 50)));
    paint(ui.painter(), rect);
}

fn form_row(ui: &mut egui::Ui, aw: f32, label: &str, paint_input: impl Fn(&egui::Painter, egui::Rect, egui::Rect)) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(aw, 22.0), egui::Sense::hover());
    let lw = 60.0;
    let p = ui.painter();
    p.text(egui::pos2(rect.left() + lw - 4.0, rect.center().y), egui::Align2::RIGHT_CENTER,
        label, egui::FontId::monospace(9.0), Color32::from_rgb(120, 125, 140));
    let inp = egui::Rect::from_min_max(
        egui::pos2(rect.left() + lw + 4.0, rect.top() + 2.0),
        egui::pos2(rect.right(), rect.bottom() - 2.0));
    paint_input(p, inp, inp);
    ui.add_space(2.0);
}
