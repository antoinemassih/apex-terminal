//! PaneGrid story — interactive split-pane layout playground.
//!
//! Widgets shown:
//!   PaneGrid: recursive split-pane with 3 distinct mock content types (1)
//!   Top toolbar: programmatic Split H / Split V / Close / Reset buttons
//!   Keyboard hint banner
//!
//! MockPane variants:
//!   Color   — fills the pane with a solid color + centered label
//!   Counter — shows a number with Increment/Decrement buttons
//!   ScrollList — 30 labelled rows in a scroll area

use _scaffold_lib::ui_kit::widgets::{
    Button, Size, Variant,
    theme::ComponentTheme,
    pane_grid::{Axis, PaneGrid, PaneId, PaneState},
};
use _scaffold_lib::ui_kit::tokens as st;
use egui::{Color32, RichText, ScrollArea, Ui};

// ─── Mock content ─────────────────────────────────────────────────────────────

/// The three distinct pane content types shown in the playground.
#[derive(Clone, Debug)]
pub enum MockPane {
    /// Solid-color fill with a centered label.
    Color(Color32, String),
    /// Numeric counter with +/- buttons.
    Counter(i32),
    /// A scrollable list of 30 rows.
    ScrollList,
}

impl Default for MockPane {
    fn default() -> Self {
        MockPane::Color(PANE_COLORS[0], "Pane".to_string())
    }
}

// Cycle through three content types for new context-menu splits.
impl MockPane {
    /// Returns the next content type in a cycle (Color → Counter → ScrollList → Color …).
    fn next_variant(idx: u32) -> MockPane {
        match idx % 3 {
            0 => MockPane::Color(PANE_COLORS[(idx as usize / 3) % PANE_COLORS.len()], format!("Panel {}", idx)),
            1 => MockPane::Counter(0),
            _ => MockPane::ScrollList,
        }
    }
}

/// A small palette of distinct swatch colors for Color panes.
const PANE_COLORS: &[Color32] = &[
    Color32::from_rgb( 70, 130, 180), // steel blue
    Color32::from_rgb(180, 100,  60), // rust orange
    Color32::from_rgb( 80, 160,  80), // forest green
    Color32::from_rgb(160,  80, 160), // violet
    Color32::from_rgb(180, 160,  50), // gold
];

// ─── Story state ─────────────────────────────────────────────────────────────

/// Persistent state for the PaneGrid story.
pub struct PaneGridState {
    pub panes:   PaneState<MockPane>,
    pub root_id: PaneId,
    /// Monotonically increasing counter used to pick a content variant for new
    /// panes created via the toolbar (so each successive split gets a different
    /// content type).
    pub next_variant_idx: u32,
}

impl Default for PaneGridState {
    fn default() -> Self {
        let (panes, root_id) = PaneState::new(MockPane::Color(
            PANE_COLORS[0],
            "Panel A".to_string(),
        ));
        Self { panes, root_id, next_variant_idx: 1 }
    }
}

impl PaneGridState {
    /// Split the focused pane along `axis` using the next mock content variant.
    fn split_focused(&mut self, axis: Axis) {
        let focused = self.panes.focused();
        if let Some(pane_id) = focused {
            let content = MockPane::next_variant(self.next_variant_idx);
            self.next_variant_idx += 1;
            let _ = self.panes.split(pane_id, axis, content);
        }
    }

    /// Close the focused pane (no-op if it is the last pane).
    fn close_focused(&mut self) {
        if let Some(pane_id) = self.panes.focused() {
            self.panes.close(pane_id);
        }
    }

    /// Reset to the initial single-pane layout.
    fn reset(&mut self) {
        let (panes, root_id) = PaneState::new(MockPane::Color(
            PANE_COLORS[0],
            "Panel A".to_string(),
        ));
        self.panes = panes;
        self.root_id = root_id;
        self.next_variant_idx = 1;
    }
}

// ─── Story entry point ────────────────────────────────────────────────────────

pub fn show(ui: &mut Ui, theme: &dyn ComponentTheme, state: &mut PaneGridState) {
    // ── Heading ──────────────────────────────────────────────────────────────
    ui.label(
        RichText::new("PaneGrid")
            .color(theme.text())
            .size(st::font_md())
            .strong(),
    );
    ui.add_space(st::gap_xs());

    // ── Keyboard hint ────────────────────────────────────────────────────────
    let hint_color = st::color_alpha(theme.dim(), st::alpha_strong());
    ui.label(
        RichText::new("Right-click any pane to Split / Close. Drag splitters to resize.")
            .color(hint_color)
            .size(st::font_xs()),
    );
    ui.add_space(st::gap_sm());

    // ── Toolbar ──────────────────────────────────────────────────────────────
    ui.horizontal(|ui| {
        ui.label(RichText::new("Focused:").color(theme.dim()).size(st::font_sm()));

        if Button::new("Split → H")
            .variant(Variant::Secondary)
            .size(Size::Sm)
            .show(ui, theme)
            .clicked()
        {
            state.split_focused(Axis::Horizontal);
        }
        if Button::new("Split → V")
            .variant(Variant::Secondary)
            .size(Size::Sm)
            .show(ui, theme)
            .clicked()
        {
            state.split_focused(Axis::Vertical);
        }
        if Button::new("Close")
            .variant(Variant::Ghost)
            .size(Size::Sm)
            .show(ui, theme)
            .clicked()
        {
            state.close_focused();
        }

        ui.add_space(st::gap_md());

        if Button::new("Reset")
            .variant(Variant::Danger)
            .size(Size::Sm)
            .show(ui, theme)
            .clicked()
        {
            state.reset();
        }

        // Pane count badge.
        let count = state.panes.pane_count();
        ui.label(
            RichText::new(format!("({} pane{})", count, if count == 1 { "" } else { "s" }))
                .color(hint_color)
                .size(st::font_xs()),
        );
    });
    ui.add_space(st::gap_sm());

    // ── PaneGrid ─────────────────────────────────────────────────────────────
    // Allocate a fixed-height region for the grid so the story area is stable.
    let grid_height = ui.available_height().min(500.0).max(200.0);
    let grid_rect = {
        let cursor = ui.cursor().min;
        let rect = egui::Rect::from_min_size(cursor, egui::vec2(ui.available_width(), grid_height));
        // Advance the cursor past this rect.
        let _ = ui.allocate_rect(rect, egui::Sense::hover());
        rect
    };

    // We need a child Ui clipped to `grid_rect` to hand to PaneGrid::show.
    let mut grid_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(grid_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    grid_ui.set_clip_rect(grid_rect);

    PaneGrid::new(&mut state.panes, render_mock_pane)
        .splitter_width(6.0)
        .show_pane_chrome(true)
        .show(&mut grid_ui, theme);

    ui.add_space(st::gap_sm());

    // ── Legend ───────────────────────────────────────────────────────────────
    rule(&mut *ui, theme);
    ui.label(RichText::new("Content types:").color(theme.dim()).size(st::font_xs()));
    ui.horizontal(|ui| {
        for (label, color) in [
            ("Color pane", theme.accent()),
            ("Counter pane", theme.dim()),
            ("ScrollList pane", theme.border()),
        ] {
            let dot_rect = ui.allocate_exact_size(
                egui::vec2(8.0, 8.0),
                egui::Sense::hover(),
            ).0;
            ui.painter().circle_filled(dot_rect.center(), 4.0, color);
            ui.label(RichText::new(label).color(theme.dim()).size(st::font_xs()));
            ui.add_space(st::gap_md());
        }
    });
}

// ─── Per-pane renderer ────────────────────────────────────────────────────────

fn render_mock_pane(ui: &mut Ui, _id: PaneId, content: &mut MockPane, theme: &dyn ComponentTheme) {
    match content {
        MockPane::Color(color, label) => render_color_pane(ui, *color, label, theme),
        MockPane::Counter(n)          => render_counter_pane(ui, n, theme),
        MockPane::ScrollList          => render_scroll_pane(ui, theme),
    }
}

/// Color pane — fills the rect with the color and shows the label centered.
fn render_color_pane(ui: &mut Ui, color: Color32, label: &str, _theme: &dyn ComponentTheme) {
    let rect = ui.available_rect_before_wrap();
    if rect.width() < 2.0 || rect.height() < 2.0 {
        return;
    }
    // Tinted background fill.
    ui.painter().rect_filled(rect, egui::CornerRadius::ZERO, st::color_alpha(color, st::alpha_subtle()));

    // Centered label — use painter text at the center to avoid allocate_ui_at_rect.
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(st::font_md()),
        st::color_alpha(color, st::alpha_solid()),
    );

    // Consume the rect so egui knows we used the space.
    ui.allocate_rect(rect, egui::Sense::hover());
}

/// Counter pane — shows a number with +/- buttons.
fn render_counter_pane(ui: &mut Ui, counter: &mut i32, theme: &dyn ComponentTheme) {
    ui.add_space(st::gap_sm());
    ui.centered_and_justified(|ui| {
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(counter.to_string())
                    .color(theme.text())
                    .size(st::font_display_sm())
                    .strong(),
            );
            ui.add_space(st::gap_xs());
            ui.horizontal(|ui| {
                if Button::new("−")
                    .variant(Variant::Secondary)
                    .size(Size::Sm)
                    .show(ui, theme)
                    .clicked()
                {
                    *counter -= 1;
                }
                ui.add_space(st::gap_xs());
                if Button::new("+")
                    .variant(Variant::Primary)
                    .size(Size::Sm)
                    .show(ui, theme)
                    .clicked()
                {
                    *counter += 1;
                }
            });
        });
    });
}

/// ScrollList pane — 30 labelled rows in a scroll area.
fn render_scroll_pane(ui: &mut Ui, theme: &dyn ComponentTheme) {
    let avail_h = ui.available_height();
    ScrollArea::vertical()
        .max_height(avail_h)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for i in 1..=30_usize {
                let color = if i % 2 == 0 { theme.dim() } else { theme.text() };
                ui.label(
                    RichText::new(format!("Row {:02} — placeholder list item", i))
                        .color(color)
                        .size(st::font_sm()),
                );
            }
        });
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn rule(ui: &mut Ui, theme: &dyn ComponentTheme) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 1.0),
        egui::Sense::hover(),
    );
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        egui::Stroke::new(st::stroke_thin(), st::color_alpha(theme.border(), st::alpha_muted())),
    );
    ui.add_space(st::gap_sm());
}
