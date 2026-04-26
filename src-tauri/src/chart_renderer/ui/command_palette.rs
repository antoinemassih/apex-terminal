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
use super::super::gpu::*;
use super::super::ChartWidgetKind;
use crate::chart_renderer::gpu::fetch_bars_background;
use crate::chart_renderer::trading::OrderStatus;

// ────────────────────────────────────────────────────────────────────────────
// Category
// ────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Category {
    Command, Symbol, Widget, Overlay, Theme, Timeframe,
    Layout, Play, Alert, Setting, Ai, Dynamic, Help, Calc, Recent,
}

impl Category {
    fn label(self) -> &'static str {
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
    fn color(self, t: &Theme) -> egui::Color32 {
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
struct Entry {
    id: String,
    label: String,
    desc: String,
    cat: Category,
    hotkey: Option<String>,
}

// ────────────────────────────────────────────────────────────────────────────
// Registry builders
// ────────────────────────────────────────────────────────────────────────────

const THEME_NAMES: &[&str] = &[
    "Midnight", "Nord", "Monokai", "Solarized", "Dracula", "Gruvbox",
    "Catppuccin", "Tokyo Night", "Kanagawa", "Everforest", "Vesper", "Rosé Pine",
    "Bauhaus", "Peach", "Ivory",
];

const TF_IDS: &[&str] = &["1m","5m","15m","30m","1h","2h","4h","1D","1W","1M"];

const LAYOUT_IDS: &[(&str, &str)] = &[
    ("1","Single pane"),("2","Two panes H"),("2H","Two panes V"),
    ("3","Three panes"),("3L","3 L-shape"),("4","Quad"),("4L","4 L-shape"),
    ("5C","5 centered"),("6","Six panes"),("9","Nine-up"),
];

const OVERLAY_IDS: &[(&str, &str)] = &[
    ("vol-shelves",   "Volume Shelves"),
    ("confluence",    "Confluence Zones"),
    ("momentum",      "Momentum Heatmap"),
    ("trend-strip",   "Trend Strip"),
    ("breadth",       "Breadth Tint"),
    ("vol-cone",      "Vol Cone"),
    ("price-memory",  "Price Memory"),
    ("liquidity",     "Liquidity Voids"),
    ("corr-ribbon",   "Correlation Ribbon"),
    ("analyst",       "Analyst Targets"),
    ("pe-band",       "PE Band"),
    ("insider",       "Insider Trades"),
];

fn widget_catalog() -> Vec<(ChartWidgetKind, &'static str, &'static str)> {
    use ChartWidgetKind::*;
    vec![
        (RsiMulti,          "rsi-multi",        "RSI Multi"),
        (TrendAlign,        "trend-align",      "TrendAlign"),
        (VolumeShelf,       "volume-shelf",     "VolumeShelf"),
        (Confluence,        "confluence",       "Confluence"),
        (FlowCompass,       "flow-compass",     "FlowCompass"),
        (VolRegime,         "vol-regime",       "VolRegime"),
        (MomentumHeat,      "momentum-heat",    "MomentumHeat"),
        (BreadthThermo,     "breadth-thermo",   "BreadthThermo"),
        (SectorRotation,    "sector-rotation",  "SectorRotation"),
        (OptionsSentiment,  "options-sent",     "OptionsSentiment"),
        (RelStrength,       "rel-strength",     "RelStrength"),
        (RiskDash,          "risk-dash",        "RiskDash"),
        (EarningsMom,       "earnings-mom",     "EarningsMom"),
        (LiquidityScore,    "liquidity-score",  "LiquidityScore"),
        (SignalRadar,       "signal-radar",     "SignalRadar"),
        (CrossAssetPulse,   "cross-asset",      "CrossAssetPulse"),
        (TapeSpeed,         "tape-speed",       "TapeSpeed"),
        (PayoffChart,       "payoff",           "PayoffChart"),
        (OptionsFlow,       "options-flow",     "OptionsFlow"),
        (EconCalendar,      "econ-cal",         "EconCalendar"),
        (Latency,           "latency",          "Latency"),
        (TrendStrength,     "trend-strength",   "Trend Strength"),
        (Momentum,          "momentum",         "Momentum"),
        (Volatility,        "volatility",       "Volatility"),
        (KeyLevels,         "key-levels",       "Key Levels"),
        (Fundamentals,      "fundamentals",     "Fundamentals"),
        (PositionPnl,       "position-pnl",     "Position P&L"),
        (DailyPnl,          "daily-pnl",        "Daily P&L"),
        (VixMonitor,        "vix",              "VIX Monitor"),
        (MarketBreadth,     "market-breadth",   "Market Breadth"),
    ]
}

fn widget_kind_from_id(id: &str) -> Option<ChartWidgetKind> {
    widget_catalog().into_iter().find(|(_, i, _)| *i == id).map(|(k,_,_)| k)
}

fn build_registry(watchlist: &Watchlist, active_pane_type: PaneType) -> Vec<Entry> {
    let mut v: Vec<Entry> = Vec::new();
    let mk = |id: &str, label: &str, desc: &str, cat: Category, hk: Option<&str>| -> Entry {
        Entry { id: id.into(), label: label.into(), desc: desc.into(), cat, hotkey: hk.map(|s| s.to_string()) }
    };

    // Hero
    v.push(mk("ai:chat",         "Ask Gemma",                     "Natural-language chat — scanners, alerts, context", Category::Ai, Some("Tab")));
    v.push(mk("dyn:reorganize",  "Reorganize layout (Dynamic UI)","Let Gemma 2B rearrange panels for the current task",  Category::Dynamic, None));

    // Risk / trading
    v.push(mk("cmd:flatten",   "Flatten all positions", "Close every open position via broker", Category::Command, Some("Ctrl+Shift+F")));
    v.push(mk("cmd:cancel",    "Cancel all orders",     "Cancel every working order",           Category::Command, Some("Ctrl+Shift+C")));
    v.push(mk("cmd:reverse",   "Reverse position",      "Flip side on active symbol",           Category::Command, None));
    v.push(mk("cmd:halfsize",  "Halve position",        "Reduce active position by 50%",        Category::Command, None));

    // Layout presets
    for (id, d) in LAYOUT_IDS {
        v.push(mk(&format!("layout:{id}"), &format!("Layout · {id}"), d, Category::Layout, None));
    }

    // Themes
    for name in THEME_NAMES {
        v.push(mk(&format!("theme:{}", name.to_lowercase()), &format!("Theme · {name}"), "Switch global theme", Category::Theme, None));
    }

    // Timeframes
    for tf in TF_IDS {
        v.push(mk(&format!("tf:{tf}"), &format!("Timeframe · {tf}"), "Set active chart timeframe", Category::Timeframe, None));
    }

    // Overlays
    for (id, label) in OVERLAY_IDS {
        v.push(mk(&format!("overlay:{id}"), &format!("Toggle · {label}"), "Chart overlay", Category::Overlay, None));
    }

    // Widgets (with display name)
    for (_, id, label) in widget_catalog() {
        v.push(mk(&format!("widget:{id}"), &format!("Add widget · {label}"), "Add to active pane", Category::Widget, None));
    }

    // Settings
    v.push(mk("setting:hotkeys",     "Edit hotkeys",         "Open hotkey editor",  Category::Setting, None));
    v.push(mk("setting:settings",    "Settings",             "Open settings panel", Category::Setting, None));
    v.push(mk("setting:apex-diag",   "ApexData diagnostics", "Live view of REST/WS/chain state", Category::Setting, None));
    v.push(mk("setting:workspace",   "Save workspace…",      "Save current layout", Category::Setting, None));
    v.push(mk("setting:pane-chart",     "Pane type · Chart",     "Switch active pane to Chart",     Category::Setting, None));
    v.push(mk("setting:pane-portfolio", "Pane type · Portfolio", "Switch active pane to Portfolio", Category::Setting, None));
    v.push(mk("setting:pane-dashboard", "Pane type · Dashboard", "Switch active pane to Dashboard", Category::Setting, None));
    v.push(mk("setting:pane-heatmap",   "Pane type · Heatmap",   "Switch active pane to Heatmap",   Category::Setting, None));

    // Help
    v.push(mk("help:prefixes", "Prefix modes — > @ # / ? =", "Quick reference", Category::Help, None));
    v.push(mk("help:widgets",  "List all widgets",           "`? widgets`",     Category::Help, None));
    v.push(mk("help:overlays", "List all overlays",          "`? overlays`",    Category::Help, None));

    // Dynamic: plays
    for p in watchlist.plays.iter().take(30) {
        v.push(mk(
            &format!("play:{}", p.id),
            &format!("Play · {}", p.title),
            &format!("{} · {} @ {}", p.symbol, match p.direction { super::super::PlayDirection::Long => "long", super::super::PlayDirection::Short => "short" }, p.entry_price),
            Category::Play,
            None,
        ));
    }

    // Dynamic: alerts
    for a in watchlist.alerts.iter().take(30) {
        v.push(mk(
            &format!("alert:{}", a.id),
            &format!("Alert · {} {} {}", a.symbol, if a.above {">"} else {"<"}, a.price),
            if a.triggered { "triggered" } else { "armed" },
            Category::Alert,
            None,
        ));
    }

    // Contextual boost: duplicate a few most-relevant entries at the top as "suggested"
    // (handled at display time via pane_type argument)
    let _ = active_pane_type;

    v
}

// ────────────────────────────────────────────────────────────────────────────
// Fuzzy scoring
// ────────────────────────────────────────────────────────────────────────────

fn fuzzy_score(query: &str, target: &str) -> Option<i32> {
    if query.is_empty() { return Some(0); }
    let q = query.to_lowercase();
    let t = target.to_lowercase();
    if t == q { return Some(2000); }
    if t.starts_with(&q) { return Some(1000 - t.len() as i32); }
    if t.contains(&q) { return Some(500 - t.len() as i32); }
    let mut qi = 0;
    let qb = q.as_bytes();
    for c in t.bytes() {
        if qi < qb.len() && c == qb[qi] { qi += 1; }
    }
    if qi == qb.len() { Some(100 - t.len() as i32) } else { None }
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
        .frame(egui::Frame::popup(&ctx.style())
            .fill(color_alpha(t.toolbar_bg, 252))
            .inner_margin(egui::Margin::same(GAP_LG as i8))
            .stroke(egui::Stroke::new(STROKE_BOLD, color_alpha(t.accent, ALPHA_STRONG)))
            .corner_radius(10.0)
            .shadow(egui::epaint::Shadow {
                offset: [0, 10], blur: 32, spread: 2,
                color: egui::Color32::from_black_alpha(140),
            }))
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
// AI mode (Gemma 4 placeholder)
// ────────────────────────────────────────────────────────────────────────────

fn draw_ai_mode(ui: &mut egui::Ui, watchlist: &mut Watchlist, t: &Theme, pal_w: f32) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("✦ Ask Apex").size(14.0).strong().color(t.text));
        ui.add_space(8.0);
        let (badge_rect, _) = ui.allocate_exact_size(egui::vec2(68.0, 18.0), egui::Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(badge_rect, 9.0, Category::Ai.color(t).gamma_multiply(0.25));
        painter.rect_stroke(badge_rect, 9.0, egui::Stroke::new(1.0, Category::Ai.color(t)), egui::StrokeKind::Inside);
        painter.text(badge_rect.center(), egui::Align2::CENTER_CENTER,
            "GEMMA 4", egui::FontId::proportional(9.0), Category::Ai.color(t));
        ui.add_space(8.0);
        ui.label(egui::RichText::new("placeholder").size(10.0).italics().color(t.dim));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("← back").clicked() { watchlist.cmd_palette_ai_mode = false; }
        });
    });

    ui.add_space(6.0); ui.separator(); ui.add_space(8.0);
    ui.label(egui::RichText::new("Try:").size(10.0).color(t.dim));
    for hint in [
        "> show me oversold tech stocks breaking out on volume",
        "> alert me if SPY closes below 20ema on daily",
        "> summarize today's price action on QQQ",
        "> what widgets would help me trade earnings season?",
    ] {
        ui.label(egui::RichText::new(hint).size(11.0).monospace().color(t.text.gamma_multiply(0.7)));
    }
    ui.add_space(10.0);
    let te = ui.add(egui::TextEdit::multiline(&mut watchlist.cmd_palette_ai_input)
        .desired_width(pal_w - 16.0).desired_rows(3)
        .hint_text("Ask anything — Gemma 4 will answer (coming soon)…")
        .font(egui::FontId::proportional(12.0)));
    te.request_focus();
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Gemma 4 is not wired up yet — this is a placeholder panel.")
            .size(9.0).italics().color(t.dim));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let _ = ui.add_enabled(false, egui::Button::new("Send ⏎"));
        });
    });
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
        ui.label(egui::RichText::new("⌕").size(14.0).color(t.dim));
        let te = ui.add(egui::TextEdit::singleline(&mut watchlist.cmd_palette_query)
            .desired_width(pal_w - 180.0)
            .font(egui::FontId::proportional(13.0))
            .hint_text("Search symbols, commands, widgets…  (Tab for AI)")
            .frame(false));
        te.request_focus();

        if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
            watchlist.cmd_palette_ai_mode = true; return;
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let badge = ui.add(egui::Button::new(
                egui::RichText::new("✦ Gemma 4").size(9.0).color(Category::Ai.color(t))
            ).fill(Category::Ai.color(t).gamma_multiply(0.18))
             .stroke(egui::Stroke::new(1.0, Category::Ai.color(t)))
             .corner_radius(9.0));
            if badge.clicked() { watchlist.cmd_palette_ai_mode = true; }
        });
    });

    // Prefix legend
    ui.horizontal(|ui| {
        let chip = |ui: &mut egui::Ui, p: &str, lbl: &str| {
            ui.label(egui::RichText::new(p).size(9.0).monospace().strong().color(t.accent));
            ui.label(egui::RichText::new(lbl).size(9.0).color(t.dim));
            ui.add_space(6.0);
        };
        chip(ui, ">", "cmd"); chip(ui, "@", "sym"); chip(ui, "#", "play");
        chip(ui, "/", "set"); chip(ui, "?", "ai");  chip(ui, "=", "calc");
        ui.add_space(8.0);
        ui.label(egui::RichText::new(format!("{} pane", match pane_type {
            PaneType::Chart => "Chart", PaneType::Portfolio => "Portfolio",
            PaneType::Dashboard => "Dashboard", PaneType::Heatmap => "Heatmap",
        })).size(9.0).color(t.dim.gamma_multiply(0.7)));
    });
    ui.add_space(4.0); ui.separator(); ui.add_space(4.0);

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
                            let bg = if is_sel { color_alpha(t.accent, ALPHA_TINT) }
                                     else if resp.hovered() { color_alpha(t.accent, 18) }
                                     else { egui::Color32::TRANSPARENT };
                            let painter = ui.painter_at(rect);
                            painter.rect_filled(rect, 4.0, bg);

                            let chip_col = cat_from_label(cat_label).map(|c| c.color(t)).unwrap_or(t.dim);
                            let chip_rect = egui::Rect::from_min_size(
                                rect.min + egui::vec2(6.0, (row_h - 14.0) / 2.0),
                                egui::vec2(62.0, 14.0));
                            painter.rect_filled(chip_rect, 3.0, chip_col.gamma_multiply(0.22));
                            painter.text(chip_rect.center(), egui::Align2::CENTER_CENTER,
                                cat_label, egui::FontId::proportional(8.5), chip_col);

                            painter.text(
                                rect.min + egui::vec2(76.0, row_h / 2.0),
                                egui::Align2::LEFT_CENTER,
                                label,
                                egui::FontId::proportional(11.5),
                                if is_sel { t.text } else { t.text.gamma_multiply(0.88) },
                            );

                            if let Some(hk) = hotkey_for(id) {
                                painter.text(
                                    egui::pos2(rect.max.x - 6.0, rect.center().y),
                                    egui::Align2::RIGHT_CENTER,
                                    hk, egui::FontId::monospace(9.5), t.dim);
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
        ui.add_space(8.0);
        ui.label(egui::RichText::new("No matches. Press Tab for AI chat, or try a prefix (> @ # / ?).")
            .size(11.0).color(t.dim));
    }

    // Footer
    ui.add_space(6.0); ui.separator();
    ui.horizontal(|ui| {
        let hint = |ui: &mut egui::Ui, k: &str, l: &str| {
            ui.label(egui::RichText::new(k).size(9.0).monospace().strong().color(t.text));
            ui.label(egui::RichText::new(l).size(9.0).color(t.dim));
            ui.add_space(8.0);
        };
        hint(ui, "↑↓/jk", "nav");
        hint(ui, "⏎", "run");
        hint(ui, "Tab", "AI");
        hint(ui, "then", "chain");
        hint(ui, "Esc", "close");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(format!("{} results", watchlist.cmd_palette_results.len()))
                .size(9.0).color(t.dim));
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

// ────────────────────────────────────────────────────────────────────────────
// Help mode — `? widgets`, `? overlays`, …
// ────────────────────────────────────────────────────────────────────────────

fn draw_help_mode(ui: &mut egui::Ui, topic: &str, t: &Theme, _pal_w: f32) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(format!("Help · {}", topic)).size(13.0).strong().color(t.text));
    ui.separator();
    ui.add_space(6.0);
    egui::ScrollArea::vertical().max_height(380.0).id_salt("cmd_palette_help").show(ui, |ui| {
        match topic {
            "widgets" => {
                for (_, id, label) in widget_catalog() {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("widget:{id}")).monospace().size(10.0).color(t.dim));
                        ui.label(egui::RichText::new(label).size(11.0).color(t.text));
                    });
                }
            }
            "overlays" => {
                for (id, label) in OVERLAY_IDS {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("overlay:{id}")).monospace().size(10.0).color(t.dim));
                        ui.label(egui::RichText::new(*label).size(11.0).color(t.text));
                    });
                }
            }
            "themes" => {
                for n in THEME_NAMES {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("theme:{}", n.to_lowercase())).monospace().size(10.0).color(t.dim));
                        ui.label(egui::RichText::new(*n).size(11.0).color(t.text));
                    });
                }
            }
            "timeframes" => {
                for tf in TF_IDS {
                    ui.label(egui::RichText::new(format!("tf:{tf}")).monospace().size(11.0).color(t.text));
                }
            }
            "layouts" => {
                for (id, d) in LAYOUT_IDS {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("layout:{id}")).monospace().size(10.0).color(t.dim));
                        ui.label(egui::RichText::new(*d).size(11.0).color(t.text));
                    });
                }
            }
            _ => {}
        }
    });
}

// ────────────────────────────────────────────────────────────────────────────
// Preview pane
// ────────────────────────────────────────────────────────────────────────────

fn draw_preview(ui: &mut egui::Ui, t: &Theme, selected: Option<&(String, String, String)>, panes: &[Chart], ap: usize) {
    let Some((id, label, cat_label)) = selected else {
        ui.add_space(20.0);
        ui.label(egui::RichText::new("Select an entry to preview").size(10.0).color(t.dim));
        return;
    };

    let cat_col = cat_from_label(cat_label).map(|c| c.color(t)).unwrap_or(t.dim);
    ui.label(egui::RichText::new(cat_label).size(9.5).strong().color(cat_col));
    ui.add_space(4.0);
    ui.label(egui::RichText::new(label).size(12.0).strong().color(t.text));
    ui.add_space(8.0);

    if let Some(sym) = id.strip_prefix("sym:") {
        draw_symbol_preview(ui, t, sym, panes, ap);
    } else if id == "ai:chat" {
        ui.label(egui::RichText::new("Conversational assistant\npowered by fine-tuned Gemma 4.")
            .size(10.5).color(t.text.gamma_multiply(0.8)));
        ui.add_space(4.0);
        ui.label(egui::RichText::new("• scanners in plain English\n• alert creation\n• context-aware answers")
            .size(10.0).color(t.dim));
    } else if id == "dyn:reorganize" {
        ui.label(egui::RichText::new("Dynamic UI (Gemma 2B)")
            .size(10.5).strong().color(Category::Dynamic.color(t)));
        ui.label(egui::RichText::new("LLM-driven layout reorganization.\nPlaceholder — see docs/dynamic-gemma-ui.md.")
            .size(10.0).color(t.text.gamma_multiply(0.75)));
    } else if id.starts_with("theme:") {
        let name = id.trim_start_matches("theme:");
        if let Some(th) = THEMES.iter().find(|th| th.name.eq_ignore_ascii_case(name)) {
            draw_theme_swatches(ui, th);
        }
    } else if id.starts_with("widget:") {
        ui.label(egui::RichText::new("Adds to active pane at next slot.\nResize/drag after placement.")
            .size(10.0).color(t.text.gamma_multiply(0.75)));
    } else if id.starts_with("overlay:") {
        ui.label(egui::RichText::new("Toggle on/off on the active chart.")
            .size(10.0).color(t.text.gamma_multiply(0.75)));
    } else if id.starts_with("tf:") {
        let tf = id.trim_start_matches("tf:");
        ui.label(egui::RichText::new(format!("Set active chart to {tf}.\nTriggers bar fetch for current symbol."))
            .size(10.0).color(t.text.gamma_multiply(0.75)));
    } else if id.starts_with("layout:") {
        ui.label(egui::RichText::new("Switches pane layout preset.\nNew panes seeded with default symbols.")
            .size(10.0).color(t.text.gamma_multiply(0.75)));
    } else if id.starts_with("play:") {
        ui.label(egui::RichText::new("Load this play on the active pane.")
            .size(10.0).color(t.text.gamma_multiply(0.75)));
    } else if id.starts_with("alert:") {
        ui.label(egui::RichText::new("Jump to symbol and scroll to alert price.")
            .size(10.0).color(t.text.gamma_multiply(0.75)));
    } else {
        ui.label(egui::RichText::new("Press ⏎ to run").size(10.0).color(t.dim));
    }
}

fn draw_symbol_preview(ui: &mut egui::Ui, t: &Theme, sym: &str, _panes: &[Chart], _ap: usize) {
    ui.label(egui::RichText::new(sym).size(22.0).monospace().strong().color(t.text));
    ui.add_space(2.0);

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
            ui.label(egui::RichText::new(format!("{last:.2}")).size(14.0).monospace().strong().color(t.text));
            ui.label(egui::RichText::new(format!("{:+.2} ({:+.2}%)", chg, pct)).size(10.5).monospace().color(col));
        });
        ui.label(egui::RichText::new(format!("Vol  {}", human_volume(vol))).size(10.0).monospace().color(t.dim));
        ui.add_space(6.0);

        // Sparkline
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 54.0), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, t.toolbar_bg.gamma_multiply(0.5));
        let tail: Vec<_> = bars.iter().rev().take(60).rev().cloned().collect();
        if tail.len() >= 2 {
            let (mn, mx) = tail.iter().fold((f32::MAX, f32::MIN), |(a, b), bar| (a.min(bar.low), b.max(bar.high)));
            let span = (mx - mn).max(1e-6);
            let pts: Vec<egui::Pos2> = tail.iter().enumerate().map(|(i, bar)| {
                let x = rect.min.x + (i as f32 / (tail.len() - 1) as f32) * rect.width();
                let y = rect.max.y - ((bar.close - mn) / span) * rect.height();
                egui::pos2(x, y)
            }).collect();
            painter.add(egui::Shape::line(pts, egui::Stroke::new(1.5, col)));
        }
    } else {
        ui.label(egui::RichText::new("Last      —").size(10.0).color(t.dim));
        ui.label(egui::RichText::new("Change    —").size(10.0).color(t.dim));
        ui.label(egui::RichText::new("Volume    —").size(10.0).color(t.dim));
        ui.add_space(6.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 40.0), egui::Sense::hover());
        ui.painter().rect_stroke(rect, 4.0,
            egui::Stroke::new(1.0, t.dim.gamma_multiply(0.4)), egui::StrokeKind::Inside);
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER,
            "no cached bars", egui::FontId::proportional(9.0), t.dim);
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
        painter.rect_filled(r, 3.0, *c);
    }
}

fn human_volume(v: f32) -> String {
    if v >= 1e9 { format!("{:.2}B", v / 1e9) }
    else if v >= 1e6 { format!("{:.2}M", v / 1e6) }
    else if v >= 1e3 { format!("{:.1}K", v / 1e3) }
    else { format!("{v:.0}") }
}

// ────────────────────────────────────────────────────────────────────────────
// Helpers
// ────────────────────────────────────────────────────────────────────────────

fn parse_prefix(s: &str) -> (Option<Category>, String) {
    if let Some(rest) = s.strip_prefix('>') { return (Some(Category::Command), rest.trim().to_string()); }
    if let Some(rest) = s.strip_prefix('@') { return (Some(Category::Symbol),  rest.trim().to_string()); }
    if let Some(rest) = s.strip_prefix('#') { return (Some(Category::Play),    rest.trim().to_string()); }
    if let Some(rest) = s.strip_prefix('/') { return (Some(Category::Setting), rest.trim().to_string()); }
    (None, s.to_string())
}

fn cat_from_label(lbl: &str) -> Option<Category> {
    Some(match lbl {
        "CMD" => Category::Command, "SYM" => Category::Symbol,
        "WIDGET" => Category::Widget, "OVERLAY" => Category::Overlay,
        "THEME" => Category::Theme, "TF" => Category::Timeframe,
        "LAYOUT" => Category::Layout, "PLAY" => Category::Play,
        "ALERT" => Category::Alert, "SETTING" => Category::Setting,
        "AI" => Category::Ai, "DYNAMIC" => Category::Dynamic,
        "HELP" => Category::Help, "CALC" => Category::Calc,
        "RECENT" => Category::Recent,
        _ => return None,
    })
}

fn hotkey_for(id: &str) -> Option<&'static str> {
    match id {
        "cmd:flatten" => Some("Ctrl+Shift+F"),
        "cmd:cancel"  => Some("Ctrl+Shift+C"),
        "ai:chat"     => Some("Tab"),
        "setting:settings" => Some("Ctrl+,"),
        _ => None,
    }
}

fn pretty_label_for_id(id: &str, watchlist: &Watchlist) -> Option<String> {
    if let Some(sym) = id.strip_prefix("sym:") { return Some(sym.to_string()); }
    if let Some(name) = id.strip_prefix("theme:") { return Some(format!("Theme · {name}")); }
    if let Some(tf) = id.strip_prefix("tf:") { return Some(format!("Timeframe · {tf}")); }
    if let Some(ly) = id.strip_prefix("layout:") { return Some(format!("Layout · {ly}")); }
    if let Some(w) = id.strip_prefix("widget:") {
        if let Some((_, _, label)) = widget_catalog().iter().find(|(_, i, _)| *i == w) {
            return Some(format!("Add widget · {label}"));
        }
    }
    if let Some(pid) = id.strip_prefix("play:") {
        if let Some(p) = watchlist.plays.iter().find(|p| p.id == pid) {
            return Some(format!("Play · {}", p.title));
        }
    }
    None
}

fn read_clipboard_ticker() -> Option<String> {
    // Clipboard integration requires adding `arboard` to Cargo.toml — skipped for now.
    None
}

/// Resolve a chain step like "5m" / "rsi-multi" / "bauhaus" / "AAPL" to an action id.
fn resolve_chain_step(step: &str, watchlist: &Watchlist) -> Option<String> {
    let s = step.trim();
    if s.is_empty() { return None; }

    // Explicit prefixes
    let (cat, body) = parse_prefix(s);
    let body_lc = body.to_lowercase();

    // Timeframe match
    if TF_IDS.iter().any(|&tf| tf.eq_ignore_ascii_case(&body)) {
        return Some(format!("tf:{}", body));
    }
    // Layout id
    if LAYOUT_IDS.iter().any(|(id, _)| id.eq_ignore_ascii_case(&body)) {
        return Some(format!("layout:{}", body.to_uppercase()));
    }
    // Theme
    if THEME_NAMES.iter().any(|n| n.eq_ignore_ascii_case(&body)) {
        return Some(format!("theme:{}", body_lc));
    }
    // Widget id
    if widget_catalog().iter().any(|(_, id, _)| id.eq_ignore_ascii_case(&body)) {
        return Some(format!("widget:{}", body_lc));
    }
    // Overlay id
    if OVERLAY_IDS.iter().any(|(id, _)| id.eq_ignore_ascii_case(&body)) {
        return Some(format!("overlay:{}", body_lc));
    }
    // Explicit sym prefix or uppercase ticker
    if matches!(cat, Some(Category::Symbol)) || (s.len() <= 5 && s.chars().all(|c| c.is_ascii_alphabetic())) {
        return Some(format!("sym:{}", body.to_uppercase()));
    }
    // Command keyword
    if body_lc.contains("flatten") { return Some("cmd:flatten".into()); }
    if body_lc.contains("cancel")  { return Some("cmd:cancel".into()); }
    // Play by title
    if let Some(p) = watchlist.plays.iter().find(|p| p.title.to_lowercase().contains(&body_lc)) {
        return Some(format!("play:{}", p.id));
    }
    None
}

// Simple expression evaluator: supports + - * / with precedence, decimals.
fn eval_expr(s: &str) -> Option<f64> {
    // Shunting-yard — very small, no parens
    let mut out: Vec<f64> = Vec::new();
    let mut ops: Vec<char> = Vec::new();
    let prec = |c: char| match c { '+' | '-' => 1, '*' | '/' => 2, _ => 0 };
    let mut iter = s.chars().peekable();
    let apply = |ops: &mut Vec<char>, out: &mut Vec<f64>| -> Option<()> {
        let op = ops.pop()?; let b = out.pop()?; let a = out.pop()?;
        out.push(match op { '+' => a + b, '-' => a - b, '*' => a * b, '/' => a / b, _ => return None });
        Some(())
    };
    while let Some(&c) = iter.peek() {
        if c.is_whitespace() { iter.next(); continue; }
        if c.is_ascii_digit() || c == '.' {
            let mut num = String::new();
            while let Some(&d) = iter.peek() {
                if d.is_ascii_digit() || d == '.' { num.push(d); iter.next(); } else { break; }
            }
            out.push(num.parse().ok()?);
        } else if "+-*/".contains(c) {
            while let Some(&top) = ops.last() {
                if prec(top) >= prec(c) { apply(&mut ops, &mut out)?; } else { break; }
            }
            ops.push(c); iter.next();
        } else { return None; }
    }
    while !ops.is_empty() { apply(&mut ops, &mut out)?; }
    if out.len() == 1 { Some(out[0]) } else { None }
}

// ────────────────────────────────────────────────────────────────────────────
// Execute
// ────────────────────────────────────────────────────────────────────────────

fn execute(
    id: &str,
    watchlist: &mut Watchlist,
    panes: &mut Vec<Chart>,
    layout: &mut Layout,
    active_pane: &mut usize,
) {
    let ap = *active_pane;

    if id == "ai:chat" { watchlist.cmd_palette_ai_mode = true; return; }
    if id == "dyn:reorganize" {
        watchlist.cmd_palette_ai_mode = true;
        watchlist.cmd_palette_ai_input =
            "Reorganize the layout for the current task (Dynamic UI placeholder — Gemma 2B)".into();
        return;
    }

    // Symbols
    if let Some(sym) = id.strip_prefix("sym:") {
        let tf = panes[ap].timeframe.clone();
        panes[ap].symbol = sym.to_string();
        panes[ap].pending_symbol_change = Some(sym.to_string());
        fetch_bars_background(sym.to_string(), tf);
        return;
    }

    // Themes
    if let Some(name) = id.strip_prefix("theme:") {
        if let Some((i, _)) = THEMES.iter().enumerate().find(|(_, th)| th.name.eq_ignore_ascii_case(name)) {
            for p in panes.iter_mut() { p.theme_idx = i; }
        }
        return;
    }

    // Timeframes
    if let Some(tf) = id.strip_prefix("tf:") {
        panes[ap].timeframe = tf.to_string();
        let sym = panes[ap].symbol.clone();
        fetch_bars_background(sym, tf.to_string());
        return;
    }

    // Layouts
    if let Some(ly_id) = id.strip_prefix("layout:") {
        let ly = match ly_id {
            "1" => Layout::One, "2" => Layout::Two, "2H" => Layout::TwoH,
            "3" => Layout::Three, "3L" => Layout::ThreeL,
            "4" => Layout::Four, "4L" => Layout::FourL,
            "5C" => Layout::FiveC, "6" => Layout::Six, "9" => Layout::Nine,
            _ => return,
        };
        let max = ly.max_panes();
        while panes.len() < max {
            let syms = ["SPY","AAPL","MSFT","NVDA","TSLA","AMZN","META","GOOG","AMD"];
            let sym = syms.get(panes.len()).copied().unwrap_or("SPY");
            let tf = panes[0].timeframe.clone();
            let mut p = Chart::new_with(sym, &tf);
            p.theme_idx = panes[0].theme_idx;
            panes.push(p);
        }
        *layout = ly;
        if *active_pane >= max { *active_pane = 0; }
        return;
    }

    // Widgets
    if let Some(wid) = id.strip_prefix("widget:") {
        if let Some(kind) = widget_kind_from_id(wid) {
            // Place at next sensible slot, avoid stacking
            let n = panes[ap].chart_widgets.len();
            let x = 0.02 + (n as f32 * 0.05).min(0.5);
            let y = 0.05 + (n as f32 * 0.08).min(0.6);
            panes[ap].chart_widgets.push(super::super::ChartWidget::new(kind, x, y));
        }
        return;
    }

    // Overlays
    if let Some(ov) = id.strip_prefix("overlay:") {
        let c = &mut panes[ap];
        match ov {
            "vol-shelves"   => c.show_vol_shelves = !c.show_vol_shelves,
            "confluence"    => c.show_confluence = !c.show_confluence,
            "momentum"      => c.show_momentum_heat = !c.show_momentum_heat,
            "trend-strip"   => c.show_trend_strip = !c.show_trend_strip,
            "breadth"       => c.show_breadth_tint = !c.show_breadth_tint,
            "vol-cone"      => c.show_vol_cone = !c.show_vol_cone,
            "price-memory"  => c.show_price_memory = !c.show_price_memory,
            "liquidity"     => c.show_liquidity_voids = !c.show_liquidity_voids,
            "corr-ribbon"   => c.show_corr_ribbon = !c.show_corr_ribbon,
            "analyst"       => c.show_analyst_targets = !c.show_analyst_targets,
            "pe-band"       => c.show_pe_band = !c.show_pe_band,
            "insider"       => c.show_insider_trades = !c.show_insider_trades,
            _ => {}
        }
        return;
    }

    // Settings
    match id {
        "setting:hotkeys"        => { watchlist.hotkey_editor_open = true; return; }
        "setting:settings"       => { watchlist.settings_open = true; return; }
        "setting:apex-diag"      => { watchlist.apex_diag_open = true; return; }
        "setting:workspace"      => { watchlist.settings_open = true; return; }
        "setting:pane-chart"     => { panes[ap].pane_type = PaneType::Chart; return; }
        "setting:pane-portfolio" => { panes[ap].pane_type = PaneType::Portfolio; return; }
        "setting:pane-dashboard" => { panes[ap].pane_type = PaneType::Dashboard; return; }
        "setting:pane-heatmap"   => { panes[ap].pane_type = PaneType::Heatmap; return; }
        _ => {}
    }

    // Plays — jump active pane to play's symbol
    if let Some(pid) = id.strip_prefix("play:") {
        if let Some(p) = watchlist.plays.iter().find(|p| p.id == pid) {
            let sym = p.symbol.clone();
            let tf = panes[ap].timeframe.clone();
            panes[ap].symbol = sym.clone();
            panes[ap].pending_symbol_change = Some(sym.clone());
            fetch_bars_background(sym, tf);
        }
        return;
    }

    // Alerts — jump active pane to alert's symbol
    if let Some(aid) = id.strip_prefix("alert:") {
        if let Ok(parsed_id) = aid.parse::<u32>() {
            if let Some(a) = watchlist.alerts.iter().find(|a| a.id == parsed_id) {
                let sym = a.symbol.clone();
                let tf = panes[ap].timeframe.clone();
                panes[ap].symbol = sym.clone();
                panes[ap].pending_symbol_change = Some(sym.clone());
                fetch_bars_background(sym, tf);
            }
        }
        return;
    }

    // Trading actions
    match id {
        "cmd:flatten" => {
            for chart in panes.iter_mut() {
                chart.orders.retain(|o| o.status == OrderStatus::Executed);
            }
            std::thread::spawn(|| {
                let _ = reqwest::blocking::Client::new()
                    .post(format!("{}/risk/flatten", APEXIB_URL))
                    .timeout(std::time::Duration::from_secs(5)).send();
            });
        }
        "cmd:cancel" => {
            for chart in panes.iter_mut() { chart.orders.clear(); }
            std::thread::spawn(|| {
                let _ = reqwest::blocking::Client::new()
                    .delete(format!("{}/orders", APEXIB_URL))
                    .timeout(std::time::Duration::from_secs(5)).send();
            });
        }
        "cmd:reverse" => {
            // Placeholder — signal via API if present
            std::thread::spawn(|| {
                let _ = reqwest::blocking::Client::new()
                    .post(format!("{}/risk/reverse", APEXIB_URL))
                    .timeout(std::time::Duration::from_secs(5)).send();
            });
        }
        "cmd:halfsize" => {
            std::thread::spawn(|| {
                let _ = reqwest::blocking::Client::new()
                    .post(format!("{}/risk/halve", APEXIB_URL))
                    .timeout(std::time::Duration::from_secs(5)).send();
            });
        }
        _ => {}
    }
}
