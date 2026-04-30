//! AI mode panel, help mode, preview pane, theme swatches, symbol preview.

use egui;
use super::Category;
use super::registry::*;
use super::super::style::*;
use super::super::components::*;
use super::super::components_extra::*;
use super::super::widgets::text::{BodyLabel, CaptionLabel};
use super::super::widgets::buttons::SimpleBtn;
use super::super::super::gpu::*;

pub(super) fn draw_ai_mode(ui: &mut egui::Ui, watchlist: &mut Watchlist, t: &Theme, pal_w: f32) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("✦ Ask Apex").size(font_lg()).strong().color(t.text));
        ui.add_space(gap_lg());
        let (badge_rect, _) = ui.allocate_exact_size(egui::vec2(68.0, 18.0), egui::Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(badge_rect, current().r_lg, Category::Ai.color(t).gamma_multiply(0.25));
        painter.rect_stroke(badge_rect, current().r_lg, egui::Stroke::new(current().stroke_std, Category::Ai.color(t)), egui::StrokeKind::Inside);
        painter.text(badge_rect.center(), egui::Align2::CENTER_CENTER,
            "GEMMA 4", egui::FontId::proportional(font_sm()), Category::Ai.color(t));
        ui.add_space(gap_lg());
        ui.label(egui::RichText::new("placeholder").size(font_sm()).italics().color(t.dim));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(SimpleBtn::new("← back").color(t.dim)).clicked() { watchlist.cmd_palette_ai_mode = false; }
        });
    });

    ui.add_space(gap_md()); ui.separator(); ui.add_space(gap_lg());
    ui.add(BodyLabel::new("Try:").color(t.dim));
    for hint in [
        "> show me oversold tech stocks breaking out on volume",
        "> alert me if SPY closes below 20ema on daily",
        "> summarize today's price action on QQQ",
        "> what widgets would help me trade earnings season?",
    ] {
        ui.label(egui::RichText::new(hint).size(font_md()).monospace().color(t.text.gamma_multiply(0.7)));
    }
    ui.add_space(gap_xl());
    let te = ui.add(egui::TextEdit::multiline(&mut watchlist.cmd_palette_ai_input)
        .desired_width(pal_w - 16.0).desired_rows(3)
        .hint_text("Ask anything — Gemma 4 will answer (coming soon)…")
        .font(egui::FontId::proportional(font_md())));
    te.request_focus();
    ui.add_space(gap_md());
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Gemma 4 is not wired up yet — this is a placeholder panel.")
            .size(font_sm()).italics().color(t.dim));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let _ = ui.add(ActionButton::new("Send ⏎").primary().small().disabled(true).theme(t));
        });
    });
}

pub(super) fn draw_help_mode(ui: &mut egui::Ui, topic: &str, t: &Theme, _pal_w: f32) {
    ui.add_space(gap_sm());
    section_label_lg(ui, &format!("Help · {}", topic), t.text);
    ui.separator();
    ui.add_space(gap_md());
    egui::ScrollArea::vertical().max_height(380.0).id_salt("cmd_palette_help").show(ui, |ui| {
        match topic {
            "widgets" => {
                for (_, id, label) in widget_catalog() {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("widget:{id}")).monospace().size(font_sm()).color(t.dim));
                        ui.label(egui::RichText::new(label).size(font_md()).color(t.text));
                    });
                }
            }
            "overlays" => {
                for (id, label) in OVERLAY_IDS {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("overlay:{id}")).monospace().size(font_sm()).color(t.dim));
                        ui.label(egui::RichText::new(*label).size(font_md()).color(t.text));
                    });
                }
            }
            "themes" => {
                for n in THEME_NAMES {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("theme:{}", n.to_lowercase())).monospace().size(font_sm()).color(t.dim));
                        ui.label(egui::RichText::new(*n).size(font_md()).color(t.text));
                    });
                }
            }
            "timeframes" => {
                for tf in TF_IDS {
                    ui.label(egui::RichText::new(format!("tf:{tf}")).monospace().size(font_md()).color(t.text));
                }
            }
            "layouts" => {
                for (id, d) in LAYOUT_IDS {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("layout:{id}")).monospace().size(font_sm()).color(t.dim));
                        ui.label(egui::RichText::new(*d).size(font_md()).color(t.text));
                    });
                }
            }
            _ => {}
        }
    });
}

/// File-local helper: dim hint paragraph used throughout the preview pane.
fn preview_hint(ui: &mut egui::Ui, text: &str, t: &Theme) {
    ui.add(BodyLabel::new(text).color(t.text.gamma_multiply(0.75)));
}

pub(super) fn draw_preview(ui: &mut egui::Ui, t: &Theme, selected: Option<&(String, String, String)>, panes: &[Chart], ap: usize) {
    let Some((id, label, cat_label)) = selected else {
        ui.add_space(gap_3xl());
        ui.add(CaptionLabel::new("Select an entry to preview").color(t.dim));
        return;
    };

    let cat_col = super::matcher::cat_from_label(cat_label).map(|c| c.color(t)).unwrap_or(t.dim);
    ui.label(egui::RichText::new(cat_label).size(super::super::style::font_sm()).strong().color(cat_col));
    ui.add_space(gap_sm());
    ui.label(egui::RichText::new(label).size(font_md()).strong().color(t.text));
    ui.add_space(gap_lg());

    if let Some(sym) = id.strip_prefix("sym:") {
        draw_symbol_preview(ui, t, sym, panes, ap);
    } else if id == "ai:chat" {
        ui.label(egui::RichText::new("Conversational assistant\npowered by fine-tuned Gemma 4.")
            .size(super::super::style::font_sm()).color(t.text.gamma_multiply(0.8)));
        ui.add_space(gap_sm());
        ui.label(egui::RichText::new("• scanners in plain English\n• alert creation\n• context-aware answers")
            .size(font_sm()).color(t.dim));
    } else if id == "dyn:reorganize" {
        ui.label(egui::RichText::new("Dynamic UI (Gemma 2B)")
            .size(super::super::style::font_sm()).strong().color(Category::Dynamic.color(t)));
        preview_hint(ui, "LLM-driven layout reorganization.\nPlaceholder — see docs/dynamic-gemma-ui.md.", t);
    } else if id.starts_with("theme:") {
        let name = id.trim_start_matches("theme:");
        if let Some(th) = THEMES.iter().find(|th| th.name.eq_ignore_ascii_case(name)) {
            draw_theme_swatches(ui, th);
        }
    } else if id.starts_with("widget:") {
        preview_hint(ui, "Adds to active pane at next slot.\nResize/drag after placement.", t);
    } else if id.starts_with("overlay:") {
        preview_hint(ui, "Toggle on/off on the active chart.", t);
    } else if id.starts_with("tf:") {
        let tf = id.trim_start_matches("tf:");
        preview_hint(ui, &format!("Set active chart to {tf}.\nTriggers bar fetch for current symbol."), t);
    } else if id.starts_with("layout:") {
        preview_hint(ui, "Switches pane layout preset.\nNew panes seeded with default symbols.", t);
    } else if id.starts_with("play:") {
        preview_hint(ui, "Load this play on the active pane.", t);
    } else if id.starts_with("alert:") {
        preview_hint(ui, "Jump to symbol and scroll to alert price.", t);
    } else {
        ui.add(CaptionLabel::new("Press ⏎ to run").color(t.dim));
    }
}

fn draw_symbol_preview(ui: &mut egui::Ui, t: &Theme, sym: &str, _panes: &[Chart], _ap: usize) {
    ui.label(egui::RichText::new(sym).size(22.0).monospace().strong().color(t.text));
    ui.add_space(gap_xs());

    // Attempt to fetch cached bars (non-blocking — guarded by is_connected)
    let bars: Option<Vec<crate::chart_renderer::types::Bar>> =
        if crate::bar_cache::is_connected() {
            // Prefer 1D, fall back to 1h
            crate::bar_cache::get(sym, "1d").or_else(|| crate::bar_cache::get(sym, "1h"))
                .map(|bs| bs.into_iter().map(|b| crate::chart_renderer::types::Bar {
                    open: b.open as f32, high: b.high as f32, low: b.low as f32,
                    close: b.close as f32, volume: b.volume as f32, _pad: 0.0,
                }).collect())
        } else { None };

    if let Some(bars) = bars.as_ref().filter(|b| b.len() >= 2) {
        let last = bars.last().unwrap().close;
        let prev = bars[bars.len().saturating_sub(2)].close;
        let chg = last - prev;
        let pct = if prev.abs() > 1e-6 { (chg / prev) * 100.0 } else { 0.0 };
        let col = if chg >= 0.0 { t.bull } else { t.bear };
        let vol = bars.last().unwrap().volume;

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("{last:.2}")).size(font_lg()).monospace().strong().color(t.text));
            ui.label(egui::RichText::new(format!("{:+.2} ({:+.2}%)", chg, pct)).size(super::super::style::font_sm()).monospace().color(col));
        });
        ui.label(egui::RichText::new(format!("Vol  {}", human_volume(vol))).size(font_sm()).monospace().color(t.dim));
        ui.add_space(gap_md());

        // Sparkline
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 54.0), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, current().r_md, t.toolbar_bg.gamma_multiply(0.5));
        let tail: Vec<_> = bars.iter().rev().take(60).rev().cloned().collect();
        if tail.len() >= 2 {
            let (mn, mx) = tail.iter().fold((f32::MAX, f32::MIN), |(a, b), bar| (a.min(bar.low), b.max(bar.high)));
            let span = (mx - mn).max(1e-6);
            let pts: Vec<egui::Pos2> = tail.iter().enumerate().map(|(i, bar)| {
                let x = rect.min.x + (i as f32 / (tail.len() - 1) as f32) * rect.width();
                let y = rect.max.y - ((bar.close - mn) / span) * rect.height();
                egui::pos2(x, y)
            }).collect();
            painter.add(egui::Shape::line(pts, egui::Stroke::new(current().stroke_thick, col)));
        }
    } else {
        ui.add(BodyLabel::new("Last      —").color(t.dim));
        ui.add(BodyLabel::new("Change    —").color(t.dim));
        ui.add(BodyLabel::new("Volume    —").color(t.dim));
        ui.add_space(gap_md());
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 40.0), egui::Sense::hover());
        ui.painter().rect_stroke(rect, current().r_md,
            egui::Stroke::new(current().stroke_std, t.dim.gamma_multiply(0.4)), egui::StrokeKind::Inside);
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER,
            "no cached bars", egui::FontId::proportional(font_sm()), t.dim);
    }
}

fn draw_theme_swatches(ui: &mut egui::Ui, th: &Theme) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 40.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let colors = [th.bg, th.toolbar_bg, th.accent, th.bull, th.bear, th.dim, th.text];
    let w = rect.width() / colors.len() as f32;
    for (i, c) in colors.iter().enumerate() {
        let r = egui::Rect::from_min_size(
            egui::pos2(rect.min.x + i as f32 * w, rect.min.y),
            egui::vec2(w, rect.height()));
        painter.rect_filled(r, current().r_sm, *c);
    }
}

fn human_volume(v: f32) -> String {
    if v >= 1e9 { format!("{:.2}B", v / 1e9) }
    else if v >= 1e6 { format!("{:.2}M", v / 1e6) }
    else if v >= 1e3 { format!("{:.1}K", v / 1e3) }
    else { format!("{v:.0}") }
}
