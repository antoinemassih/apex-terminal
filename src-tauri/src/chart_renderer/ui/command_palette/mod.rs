//! Command Palette — universal search, actions, symbol preview, AI chat
//! (Gemma 4 placeholder), Dynamic UI (Gemma 2B placeholder).
//!
//! Prefix modes:
//!   `>` commands  `@` symbols  `#` plays  `/` settings  `?` AI/help  `=` calc
//!
//! Keys: Ctrl+Space toggle · Esc close · ↑↓/j/k nav · gg/G jump · Enter run · Tab AI
//! Chain:  `aapl then 5m then widget rsi-multi`

use egui;
use super::style::*;
use super::components::*;
use super::components_extra::*;
use super::widgets::pills::*;
use super::widgets::frames::PopupFrame;
use super::widgets::text::BodyLabel;
use super::widgets::inputs::TextInput;
use super::super::gpu::*;

mod registry;
mod matcher;
mod render;
mod execute;

use registry::*;
use matcher::*;
use render::*;
use execute::execute;

// ────────────────────────────────────────────────────────────────────────────
// Category
// ────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Category {
    Command, Symbol, Widget, Overlay, Theme, Timeframe,
    Layout, Play, Alert, Setting, Ai, Dynamic, Help, Calc, Recent,
}

impl Category {
    pub(super) fn label(self) -> &'static str {
        match self {
            Category::Command => "CMD",     Category::Symbol => "SYM",
            Category::Widget  => "WIDGET",  Category::Overlay => "OVERLAY",
            Category::Theme   => "THEME",   Category::Timeframe => "TF",
            Category::Layout  => "LAYOUT",  Category::Play => "PLAY",
            Category::Alert   => "ALERT",   Category::Setting => "SETTING",
            Category::Ai      => "AI",      Category::Dynamic => "DYNAMIC",
            Category::Help    => "HELP",    Category::Calc => "CALC",
            Category::Recent  => "RECENT",
        }
    }
    pub(super) fn color(self, t: &Theme) -> egui::Color32 {
        match self {
            Category::Command => t.accent,
            Category::Symbol  => egui::Color32::from_rgb(120, 180, 255),
            Category::Widget  => egui::Color32::from_rgb(180, 140, 240),
            Category::Overlay => egui::Color32::from_rgb(160, 200, 140),
            Category::Theme   => egui::Color32::from_rgb(240, 180, 140),
            Category::Timeframe => egui::Color32::from_rgb(140, 220, 200),
            Category::Layout  => egui::Color32::from_rgb(220, 200, 120),
            Category::Play    => egui::Color32::from_rgb(240, 140, 180),
            Category::Alert   => egui::Color32::from_rgb(240, 120, 120),
            Category::Setting => t.dim,
            Category::Ai      => egui::Color32::from_rgb(255, 120, 200),
            Category::Dynamic => egui::Color32::from_rgb(255, 180, 80),
            Category::Help    => t.dim,
            Category::Calc    => egui::Color32::from_rgb(140, 240, 200),
            Category::Recent  => t.dim,
        }
    }
}

#[derive(Clone)]
pub(super) struct Entry {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) desc: String,
    pub(super) cat: Category,
    pub(super) hotkey: Option<String>,
}

// ────────────────────────────────────────────────────────────────────────────
// Entry point
// ────────────────────────────────────────────────────────────────────────────

pub(crate) fn draw(
    ctx: &egui::Context,
    watchlist: &mut Watchlist,
    panes: &mut Vec<Chart>,
    layout: &mut Layout,
    active_pane: &mut usize,
    t: &Theme,
) {
    if !ctx.wants_keyboard_input() {
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Space)) {
            watchlist.cmd_palette_open = !watchlist.cmd_palette_open;
            if watchlist.cmd_palette_open {
                watchlist.cmd_palette_query.clear();
                watchlist.cmd_palette_results.clear();
                watchlist.cmd_palette_sel = 0;
                watchlist.cmd_palette_ai_mode = false;
            }
        }
    }
    if !watchlist.cmd_palette_open { return; }

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        if watchlist.cmd_palette_ai_mode { watchlist.cmd_palette_ai_mode = false; }
        else { watchlist.cmd_palette_open = false; return; }
    }

    let screen = ctx.screen_rect();
    let pal_w = 640.0_f32;
    let pal_x = (screen.width() - pal_w) / 2.0;
    let pal_y = screen.height() * 0.14;

    egui::Area::new(egui::Id::new("cmd_palette_bg"))
        .fixed_pos(egui::pos2(0.0, 0.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.painter().rect_filled(screen, 0.0, egui::Color32::from_black_alpha(140));
        });

    let ai_mode = watchlist.cmd_palette_ai_mode;

    let pal_resp = egui::Window::new("cmd_palette")
        .fixed_pos(egui::pos2(pal_x, pal_y))
        .fixed_size(egui::vec2(pal_w, 0.0))
        .title_bar(false)
        .frame(PopupFrame::new().colors(color_alpha(t.toolbar_bg, 252), t.accent).ctx(ctx).build())
        .show(ctx, |ui| {
            if ai_mode {
                draw_ai_mode(ui, watchlist, t, pal_w);
            } else {
                draw_normal_mode(ui, watchlist, panes, layout, active_pane, t, pal_w);
            }
        });

    if let Some(wr) = &pal_resp {
        let pal_rect = wr.response.rect;
        if ctx.input(|i| i.pointer.any_pressed()) {
            if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                if !pal_rect.contains(pos) { watchlist.cmd_palette_open = false; }
            }
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Normal mode
// ────────────────────────────────────────────────────────────────────────────

fn draw_normal_mode(
    ui: &mut egui::Ui,
    watchlist: &mut Watchlist,
    panes: &mut Vec<Chart>,
    layout: &mut Layout,
    active_pane: &mut usize,
    t: &Theme,
    pal_w: f32,
) {
    let ap = *active_pane;
    let pane_type = panes[ap].pane_type;

    // Header
    ui.horizontal(|ui| {
        ui.add(BodyLabel::new("⌕").size(font_lg()).color(t.dim));
        let te = TextInput::new(&mut watchlist.cmd_palette_query)
            .width(pal_w - 180.0)
            .font_size(font_lg())
            .proportional(true)
            .placeholder("Search symbols, commands, widgets…  (Tab for AI)")
            .frameless(true)
            .theme(t)
            .show(ui);
        te.request_focus();

        if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
            watchlist.cmd_palette_ai_mode = true; return;
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(ActionButton::new("✦ Gemma 4").secondary().small().palette(Category::Ai.color(t), t.bear, t.dim)).clicked() {
                watchlist.cmd_palette_ai_mode = true;
            }
        });
    });

    // Prefix legend
    ui.horizontal(|ui| {
        let chip = |ui: &mut egui::Ui, p: &str, lbl: &str| {
            ui.add(KeybindChip::new(p).palette(t.accent, t.accent));
            ui.add(BodyLabel::new(lbl).color(t.dim));
            ui.add_space(gap_md());
        };
        chip(ui, ">", "cmd"); chip(ui, "@", "sym"); chip(ui, "#", "play");
        chip(ui, "/", "set"); chip(ui, "?", "ai");  chip(ui, "=", "calc");
        ui.add_space(gap_lg());
        ui.add(BodyLabel::new(&format!("{} pane", match pane_type {
            PaneType::Chart => "Chart", PaneType::Portfolio => "Portfolio",
            PaneType::Dashboard => "Dashboard", PaneType::Heatmap => "Heatmap",
            PaneType::Spreadsheet => "Spreadsheet",
        })).size(font_sm()).color(t.dim.gamma_multiply(0.7)));
    });
    ui.add_space(gap_sm()); ui.separator(); ui.add_space(gap_sm());

    // ── Build results ───────────────────────────────────────────────────
    let raw = watchlist.cmd_palette_query.trim().to_string();

    // `?` help / AI jump
    if raw == "?" || raw.starts_with("? ") == false && raw.starts_with('?') && raw.len() > 1 {
        // `?foo` style
    }
    if raw == "?" {
        watchlist.cmd_palette_ai_mode = true; return;
    }

    // ── Help modes: `? widgets`, `? overlays`, `? themes`, `? timeframes` ──
    if let Some(help_topic) = raw.strip_prefix("?").map(|s| s.trim().to_lowercase()) {
        if matches!(help_topic.as_str(), "widgets" | "overlays" | "themes" | "timeframes" | "layouts") {
            draw_help_mode(ui, &help_topic, t, pal_w);
            return;
        }
    }

    let (filter_cat, q_body) = parse_prefix(&raw);

    // Detect command chain: "aapl then 5m then rsi"
    let chain: Vec<&str> = q_body.split(" then ").filter(|s| !s.trim().is_empty()).collect();
    let is_chain = chain.len() > 1;

    let q = if is_chain { chain[0].trim().to_string() } else { q_body.clone() };

    let mut results: Vec<(String, String, String, i32)> = Vec::new(); // id, label, cat_label, score

    // Calc fast-path
    if q.starts_with('=') {
        let expr = q.trim_start_matches('=').trim();
        if let Some(val) = eval_expr(expr) {
            results.push(("calc:x".into(), format!("= {val}"), Category::Calc.label().into(), 9999));
        }
    }

    if q.is_empty() && !is_chain {
        // Empty query: recent + clipboard + context suggestions
        let recent = watchlist.cmd_palette_recent.clone();
        for id in recent.iter().take(6) {
            let label = pretty_label_for_id(id, watchlist).unwrap_or_else(|| id.clone());
            results.push((id.clone(), format!("↻ {label}"), Category::Recent.label().into(), 0));
        }

        // Clipboard intelligence — if clipboard looks like a ticker
        if let Some(clip) = read_clipboard_ticker() {
            results.insert(0, (
                format!("sym:{clip}"),
                format!("📋 Paste symbol · {clip}"),
                Category::Symbol.label().into(),
                9000,
            ));
        }

        // Context-aware suggestions
        let ctx_ids: &[&str] = match pane_type {
            PaneType::Chart     => &["ai:chat","widget:rsi-multi","overlay:vol-shelves","overlay:confluence","tf:5m","tf:1D","cmd:flatten"],
            PaneType::Dashboard => &["ai:chat","widget:rsi-multi","widget:trend-align","widget:signal-radar","widget:risk-dash","dyn:reorganize"],
            PaneType::Portfolio => &["ai:chat","cmd:flatten","cmd:cancel","widget:risk-dash","widget:position-pnl","widget:daily-pnl"],
            PaneType::Heatmap   => &["ai:chat","widget:sector-rotation","widget:breadth-thermo","widget:market-breadth"],
            PaneType::Spreadsheet => &["ai:chat"],
        };
        let reg = build_registry(watchlist, pane_type);
        for cid in ctx_ids {
            if let Some(e) = reg.iter().find(|e| e.id == *cid) {
                results.push((e.id.clone(), e.label.clone(), e.cat.label().into(), 50));
            }
        }
        // Pad with AI + Dynamic if not already suggested
        for e in reg.iter().filter(|e| matches!(e.cat, Category::Ai | Category::Dynamic)) {
            if !results.iter().any(|r| r.0 == e.id) {
                results.push((e.id.clone(), e.label.clone(), e.cat.label().into(), 30));
            }
        }
    } else {
        // Search
        let registry = build_registry(watchlist, pane_type);
        for e in &registry {
            if let Some(cat) = filter_cat {
                if e.cat != cat { continue; }
            }
            let hay = format!("{} {} {}", e.label, e.id, e.desc);
            if let Some(s) = fuzzy_score(&q, &hay) {
                // Frequency boost
                let bonus = *watchlist.cmd_palette_freq.get(&e.id).unwrap_or(&0) as i32 * 5;
                results.push((e.id.clone(), e.label.clone(), e.cat.label().into(), s + bonus));
            }
        }

        // Symbols
        let allow_symbols = match filter_cat {
            None | Some(Category::Symbol) => true, _ => false,
        };
        if allow_symbols && !q.is_empty() {
            for si in crate::ui_kit::symbols::search_symbols(&q.to_uppercase(), 12) {
                results.push((
                    format!("sym:{}", si.symbol),
                    format!("{}  ·  {}", si.symbol, si.name),
                    Category::Symbol.label().into(),
                    800,
                ));
            }
        }

        // Did-you-mean: if nothing matched, retry with shorter query
        if results.is_empty() && q.len() > 2 {
            let shorter = &q[..q.len().saturating_sub(2)];
            for e in &registry {
                if let Some(s) = fuzzy_score(shorter, &format!("{} {}", e.label, e.id)) {
                    results.push((e.id.clone(), format!("~ {}", e.label), e.cat.label().into(), s - 200));
                }
            }
            results.truncate(8);
        }

        results.sort_by(|a, b| b.3.cmp(&a.3));
        results.truncate(40);
    }

    watchlist.cmd_palette_results = results.iter().map(|r| (r.0.clone(), r.1.clone(), r.2.clone())).collect();

    // Keyboard nav
    let n = watchlist.cmd_palette_results.len() as i32;
    let nav_down = ui.input(|i| i.key_pressed(egui::Key::ArrowDown)
        || (i.key_pressed(egui::Key::J) && !i.modifiers.any()));
    let nav_up = ui.input(|i| i.key_pressed(egui::Key::ArrowUp)
        || (i.key_pressed(egui::Key::K) && !i.modifiers.any()));
    let jump_end = ui.input(|i| i.modifiers.shift && i.key_pressed(egui::Key::G));
    let jump_start = ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::G));
    if nav_down { watchlist.cmd_palette_sel = ((watchlist.cmd_palette_sel + 1).min(n - 1)).max(0); }
    if nav_up   { watchlist.cmd_palette_sel = (watchlist.cmd_palette_sel - 1).max(0); }
    if jump_end   && n > 0 { watchlist.cmd_palette_sel = n - 1; }
    if jump_start && n > 0 { watchlist.cmd_palette_sel = 0; }

    let mut execute_idx: Option<usize> = None;
    if ui.input(|i| i.key_pressed(egui::Key::Enter)) && n > 0 {
        execute_idx = Some(watchlist.cmd_palette_sel.max(0) as usize);
    }

    // Results + preview
    if n > 0 {
        let sel_idx = watchlist.cmd_palette_sel.max(0).min(n - 1) as usize;
        let selected = watchlist.cmd_palette_results.get(sel_idx).cloned();

        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(pal_w * 0.60);
                egui::ScrollArea::vertical()
                    .max_height(420.0)
                    .auto_shrink([false, true])
                    .id_salt("cmd_palette_results")
                    .show(ui, |ui| {
                        let entries = watchlist.cmd_palette_results.clone();
                        for (ri, (id, label, cat_label)) in entries.iter().enumerate() {
                            let is_sel = ri as i32 == watchlist.cmd_palette_sel;
                            let row_h = 26.0;
                            let (rect, resp) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), row_h),
                                egui::Sense::click());
                            let bg = if is_sel { color_alpha(t.accent, alpha_tint()) }
                                     else if resp.hovered() { color_alpha(t.accent, 18) }
                                     else { egui::Color32::TRANSPARENT };
                            let painter = ui.painter_at(rect);
                            painter.rect_filled(rect, current().r_md, bg);

                            let chip_col = cat_from_label(cat_label).map(|c| c.color(t)).unwrap_or(t.dim);
                            let chip_rect = egui::Rect::from_min_size(
                                rect.min + egui::vec2(6.0, (row_h - 14.0) / 2.0),
                                egui::vec2(62.0, 14.0));
                            painter.rect_filled(chip_rect, current().r_sm, chip_col.gamma_multiply(0.22));
                            painter.text(chip_rect.center(), egui::Align2::CENTER_CENTER,
                                cat_label, egui::FontId::proportional(super::style::font_xs()), chip_col);

                            painter.text(
                                rect.min + egui::vec2(76.0, row_h / 2.0),
                                egui::Align2::LEFT_CENTER,
                                label,
                                egui::FontId::proportional(super::style::font_md()),
                                if is_sel { t.text } else { t.text.gamma_multiply(0.88) },
                            );

                            if let Some(hk) = hotkey_for(id) {
                                painter.text(
                                    egui::pos2(rect.max.x - 6.0, rect.center().y),
                                    egui::Align2::RIGHT_CENTER,
                                    hk, egui::FontId::monospace(super::style::font_sm()), t.dim);
                            }

                            if resp.clicked() { execute_idx = Some(ri); }
                            if resp.hovered() { watchlist.cmd_palette_sel = ri as i32; }
                        }
                    });
            });

            ui.separator();
            ui.vertical(|ui| {
                ui.set_width(pal_w * 0.36);
                draw_preview(ui, t, selected.as_ref(), panes, ap);
            });
        });
    } else {
        empty_state_panel(
            ui,
            "⌕",
            "No matches",
            "Press Tab for AI chat, or try a prefix (> @ # / ?).",
            t.dim,
        );
    }

    // Footer
    ui.add_space(gap_md()); ui.separator();
    ui.horizontal(|ui| {
        let hint = |ui: &mut egui::Ui, k: &str, l: &str| {
            ui.add(KeybindChip::new(k).palette(t.text, t.dim));
            ui.add(BodyLabel::new(l).color(t.dim));
            ui.add_space(gap_lg());
        };
        hint(ui, "↑↓/jk", "nav");
        hint(ui, "⏎", "run");
        hint(ui, "Tab", "AI");
        hint(ui, "then", "chain");
        hint(ui, "Esc", "close");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(BodyLabel::new(&format!("{} results", watchlist.cmd_palette_results.len())).size(font_sm()).color(t.dim));
        });
    });

    // Execute
    if let Some(idx) = execute_idx {
        let entry = watchlist.cmd_palette_results.get(idx).cloned();
        if let Some((id, _label, _cat)) = entry {
            execute(&id, watchlist, panes, layout, active_pane);
            watchlist.cmd_palette_recent.retain(|r| r != &id);
            watchlist.cmd_palette_recent.insert(0, id.clone());
            watchlist.cmd_palette_recent.truncate(16);
            *watchlist.cmd_palette_freq.entry(id.clone()).or_insert(0) += 1;

            // Chain: run remaining steps
            if is_chain {
                for step in chain.iter().skip(1) {
                    let step = step.trim();
                    if let Some(chain_id) = resolve_chain_step(step, watchlist) {
                        execute(&chain_id, watchlist, panes, layout, active_pane);
                    }
                }
            }

            if !watchlist.cmd_palette_ai_mode {
                watchlist.cmd_palette_open = false;
            }
        }
    }
}
