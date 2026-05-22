//! Pane mode-notice system.
//!
//! The *notice* is the card shown at the top of the chart body while a **mode**
//! is active — a drawing tool armed, measure mode, zoom-select. It is the
//! persistent "you are in X mode" indicator.
//!
//! This is deliberately distinct from **toasts** (`pending_order_toasts`,
//! `spike_popup`), which are transient *event* notifications. A notice is a
//! pure function of `Chart` mode state — see [`pane_notice_for`] — so there is
//! no queue or registry: the resolver maps current state to the notice to
//! show, in priority order, and the renderer draws it.
//!
//! ## Adding a consumer
//! Add a branch to [`pane_notice_for`] — no other wiring is needed. The card
//! styling, layout, shadow and key-chip rendering are all owned by
//! [`PaneNotice::render`].

use crate::chart_renderer::gpu::{Chart, Theme};
use crate::chart_renderer::ui::style::{
    color_alpha, font_lg, font_sm, mono_2xs, stroke_std, stroke_thin,
};
use crate::ui_kit::icons::Icon;

/// Accent tone for a notice — drives the title colour.
#[derive(Clone, Copy)]
pub(crate) enum NoticeTone {
    /// Neutral / informational — theme accent.
    Accent,
    /// Caution — theme warn colour.
    Warn,
}

/// A pane mode-notice: a stacked, centre-aligned card with a title line, an
/// instruction line, and an optional row of keyboard-key hints.
///
/// Build directly or via [`pane_notice_for`]; render with [`PaneNotice::render`].
pub(crate) struct PaneNotice {
    icon: &'static str,
    title: String,
    instruction: String,
    keys: Vec<(&'static str, String)>,
    tone: NoticeTone,
}

impl PaneNotice {
    /// New notice with a leading `icon` glyph, a `title`, and an `instruction`.
    pub(crate) fn new(
        icon: &'static str,
        title: impl Into<String>,
        instruction: impl Into<String>,
    ) -> Self {
        Self {
            icon,
            title: title.into(),
            instruction: instruction.into(),
            keys: Vec::new(),
            tone: NoticeTone::Accent,
        }
    }

    /// Append a keyboard-key hint — a key glyph and what it does.
    pub(crate) fn key(mut self, glyph: &'static str, label: impl Into<String>) -> Self {
        self.keys.push((glyph, label.into()));
        self
    }

    /// Set the accent tone (default [`NoticeTone::Accent`]).
    #[allow(dead_code)]
    pub(crate) fn tone(mut self, tone: NoticeTone) -> Self {
        self.tone = tone;
        self
    }

    /// Paint the card, centred at the top of the chart body.
    ///
    /// `rect` is the full pane rect, `cw` the chart-body width, `pt` the pane
    /// header height — the card is centred horizontally over `cw` and sits
    /// just below the header.
    pub(crate) fn render(&self, painter: &egui::Painter, t: &Theme, rect: egui::Rect, cw: f32, pt: f32) {
        use egui::{Align2, CornerRadius, FontId, Pos2, Rect, Stroke, StrokeKind, Vec2};

        const PAD_X: f32 = 18.0;
        const PAD_Y: f32 = 11.0;
        const TITLE_GAP: f32 = 4.0; // title -> instruction
        const KEYS_GAP: f32 = 8.0; // instruction -> keys row
        const CHIP_PAD: f32 = 4.0; // key-chip inner x-padding
        const KEY_LABEL_GAP: f32 = 4.0; // chip -> its label
        const KEY_ENTRY_GAP: f32 = 12.0; // gap between (chip+label) entries

        let accent = match self.tone {
            NoticeTone::Accent => t.accent,
            NoticeTone::Warn => t.warn,
        };

        let measure = |s: &str, f: FontId| -> Vec2 {
            painter.layout_no_wrap(s.to_string(), f, egui::Color32::WHITE).size()
        };

        // ── Measure ─────────────────────────────────────────────────────────
        let title_font = FontId::proportional(font_lg());
        let instr_font = FontId::proportional(font_sm());

        let title_str = format!("{}  {}", self.icon, self.title);
        let title_sz = measure(&title_str, title_font.clone());
        let instr_sz = measure(&self.instruction, instr_font.clone());

        let key_h = if self.keys.is_empty() { 0.0 } else { measure("M", mono_2xs()).y.max(13.0) };
        let mut keys_w = 0.0_f32;
        let mut key_pieces: Vec<(f32, f32)> = Vec::new(); // (chip_w, label_w) per entry
        for (glyph, label) in &self.keys {
            let chip_w = measure(glyph, mono_2xs()).x + CHIP_PAD * 2.0;
            let label_w = measure(label, mono_2xs()).x;
            key_pieces.push((chip_w, label_w));
            keys_w += chip_w + KEY_LABEL_GAP + label_w + KEY_ENTRY_GAP;
        }
        if keys_w > 0.0 {
            keys_w -= KEY_ENTRY_GAP; // no trailing gap
        }

        // ── Card geometry ───────────────────────────────────────────────────
        let content_w = title_sz.x.max(instr_sz.x).max(keys_w);
        let mut content_h = title_sz.y + TITLE_GAP + instr_sz.y;
        if !self.keys.is_empty() {
            content_h += KEYS_GAP + key_h;
        }
        let card_w = content_w + PAD_X * 2.0;
        let card_h = content_h + PAD_Y * 2.0;
        let card = Rect::from_min_size(
            Pos2::new(rect.left() + (cw - card_w) * 0.5, rect.top() + pt + 8.0),
            Vec2::new(card_w, card_h),
        );

        // ── Soft drop shadow ────────────────────────────────────────────────
        let shadow = egui::epaint::Shadow {
            offset: [0, 5],
            blur: 18,
            spread: 1,
            color: color_alpha(t.shadow_color, 95),
        };
        painter.add(shadow.as_shape(card, CornerRadius::same(9)));

        // ── Card surface + hairline border ──────────────────────────────────
        painter.rect(
            card,
            CornerRadius::same(9),
            t.toolbar_bg,
            Stroke::new(stroke_std(), color_alpha(t.toolbar_border, 220)),
            StrokeKind::Inside,
        );

        let cx = card.center().x;
        let mut y = card.top() + PAD_Y;

        // Title — bigger, accent-coloured, centred.
        painter.text(
            Pos2::new(cx, y + title_sz.y * 0.5),
            Align2::CENTER_CENTER,
            &title_str,
            title_font,
            accent,
        );
        y += title_sz.y + TITLE_GAP;

        // Instruction — smaller, dim, centred, on its own line.
        painter.text(
            Pos2::new(cx, y + instr_sz.y * 0.5),
            Align2::CENTER_CENTER,
            &self.instruction,
            instr_font,
            color_alpha(t.text, 175),
        );
        y += instr_sz.y;

        // Keyboard-key hints — centred row of chips + labels.
        if !self.keys.is_empty() {
            y += KEYS_GAP;
            let row_cy = y + key_h * 0.5;
            let mut x = cx - keys_w * 0.5;
            for ((glyph, label), (chip_w, label_w)) in self.keys.iter().zip(key_pieces.iter()) {
                let chip = Rect::from_min_size(
                    Pos2::new(x, row_cy - key_h * 0.5),
                    Vec2::new(*chip_w, key_h),
                );
                painter.rect(
                    chip,
                    CornerRadius::same(3),
                    color_alpha(t.toolbar_border, 80),
                    Stroke::new(stroke_thin(), color_alpha(t.toolbar_border, 230)),
                    StrokeKind::Inside,
                );
                painter.text(chip.center(), Align2::CENTER_CENTER, *glyph, mono_2xs(), color_alpha(t.text, 225));
                x += chip_w + KEY_LABEL_GAP;
                painter.text(Pos2::new(x, row_cy), Align2::LEFT_CENTER, label, mono_2xs(), color_alpha(t.dim, 215));
                x += label_w + KEY_ENTRY_GAP;
            }
        }
    }
}

/// Resolve the notice to show for `chart`'s current mode, if any.
///
/// Priority order: drawing tool, then measure mode, then zoom-select. Returns
/// `None` when no mode is active. **To add a new consumer, add a branch here.**
pub(crate) fn pane_notice_for(chart: &Chart) -> Option<PaneNotice> {
    if !chart.draw_tool.is_empty() {
        let (title, instruction) = drawing_tool_text(&chart.draw_tool)?;
        return Some(
            PaneNotice::new(Icon::PENCIL_LINE, title, instruction)
                .key("Esc", "cancel")
                .key("M", if chart.magnet { "magnet on" } else { "magnet off" }),
        );
    }
    if chart.measure_active {
        return Some(
            PaneNotice::new(Icon::RULER, "Measure", "drag across the chart to measure")
                .key("Esc", "exit"),
        );
    }
    if chart.zoom_selecting {
        return Some(
            PaneNotice::new(Icon::MAGNIFYING_GLASS_PLUS, "Zoom Select", "drag to frame an area")
                .key("Esc", "cancel"),
        );
    }
    None
}

/// Render the active mode-notice for `chart` (if any). Called once per pane,
/// every frame, from the pane paint path — ungated by pointer position.
pub(crate) fn render_pane_notice(
    painter: &egui::Painter,
    t: &Theme,
    chart: &Chart,
    rect: egui::Rect,
    cw: f32,
    pt: f32,
) {
    if let Some(notice) = pane_notice_for(chart) {
        notice.render(painter, t, rect, cw, pt);
    }
}

/// Display title + instruction for a drawing-tool id. `None` for unknown ids.
fn drawing_tool_text(tool: &str) -> Option<(&'static str, &'static str)> {
    Some(match tool {
        "trendline" => ("Trendline", "click 2 points"),
        "hline" => ("Horizontal Line", "click to place"),
        "hzone" => ("Zone", "click 2 prices"),
        "fibonacci" => ("Fibonacci", "click start, then end"),
        "channel" => ("Channel", "click 2 points, then offset"),
        "fibchannel" => ("Fib Channel", "click 2 points, then offset"),
        "pitchfork" => ("Pitchfork", "click pivot, then 2 reactions"),
        "gannfan" => ("Gann Fan", "click origin, then scale point"),
        "regression" => ("Regression", "click start, then end"),
        "xabcd" => ("XABCD", "click 5 points: X, A, B, C, D"),
        "vline" => ("Vertical Line", "click to place"),
        "ray" => ("Ray", "click 2 points"),
        "fibext" => ("Fib Extension", "click A, B, then C"),
        "fibtimezone" => ("Fib Time Zones", "click anchor"),
        "fibarc" => ("Fib Arcs", "click 2 points"),
        "gannbox" => ("Gann Box", "click 2 corners"),
        "pricerange" => ("Price Range", "click 2 corners"),
        "riskreward" => ("Risk / Reward", "click entry, stop, target"),
        "textnote" => ("Text Note", "click to place"),
        s if s.starts_with("elliott") => ("Elliott Wave", "click wave points"),
        _ => return None,
    })
}
