# Design System Coverage Audit

> Generated: 2026-04-30  
> Scope: `src-tauri/src/chart_renderer/ui/**/*.rs` (excluding `widgets/`, `components/`, `components_extra/`)  
> Grand totals: **190 widget usages** · **289 raw primitives** · overall adoption **40%**

---

## 1. Summary Table

| File | Widget usages | Raw primitives | Coverage % |
|------|--------------|----------------|-----------|
| alerts_panel.rs | 11 | 3 | 79% |
| analysis_panel.rs | 2 | 3 | 40% |
| apex_diagnostics.rs | 22 | 0 | **100%** |
| chart_pane.rs | 0 | 0 | — |
| chart_widgets.rs | 0 | 6 | 0% |
| command_palette/execute.rs | 0 | 0 | — |
| command_palette/matcher.rs | 0 | 0 | — |
| command_palette/mod.rs | 3 | 1 | 75% |
| command_palette/registry.rs | 0 | 0 | — |
| command_palette/render.rs | 2 | 15 | 12% |
| connection_panel.rs | 3 | 1 | 75% |
| dashboard_pane.rs | 2 | 0 | **100%** |
| discord_panel.rs | 32 | 14 | 70% |
| dom_panel.rs | 2 | 0 | **100%** |
| drawings.rs | 0 | 0 | — |
| feed_panel.rs | 4 | 2 | 67% |
| heatmap_pane.rs | 1 | 0 | **100%** |
| hotkey_editor.rs | 3 | 2 | 60% |
| indicator_editor.rs | 3 | 35 | 8% |
| indicators.rs | 0 | 0 | — |
| journal_panel.rs | 2 | 10 | 17% |
| mod.rs | 0 | 0 | — |
| news_panel.rs | 4 | 2 | 67% |
| object_tree.rs | 5 | 5 | 50% |
| option_quick_picker.rs | 1 | 9 | 10% |
| orders.rs | 0 | 0 | — |
| orders_panel.rs | 5 | 3 | 62% |
| oscillators.rs | 0 | 0 | — |
| overlay_manager.rs | 4 | 1 | 80% |
| painter_chrome.rs | 0 | 0 | — |
| panels.rs | 0 | 0 | — |
| picker.rs | 0 | 0 | — |
| playbook_panel.rs | 1 | 0 | **100%** |
| plays_panel.rs | 11 | 26 | 30% |
| portfolio_pane.rs | 1 | 0 | **100%** |
| research_panel.rs | 3 | 4 | 43% |
| rrg_panel.rs | 2 | 2 | 50% |
| scanner_panel.rs | 9 | 1 | 90% |
| screenshot_panel.rs | 2 | 2 | 50% |
| script_panel.rs | 8 | 15 | 35% |
| seasonality_panel.rs | 2 | 0 | **100%** |
| settings_panel.rs | 4 | 11 | 27% |
| signals_panel.rs | 6 | 6 | 50% |
| spread_panel.rs | 5 | 5 | 50% |
| spreadsheet_pane.rs | 2 | 3 | 40% |
| style.rs | 0 | 33 | 0% |
| tape_panel.rs | 3 | 0 | **100%** |
| template_popup.rs | 4 | 3 | 57% |
| toolbar.rs | 0 | 0 | — |
| trendline_filter.rs | 4 | 1 | 80% |
| watchlist.rs | 0 | 0 | — |
| watchlist_panel.rs | 12 | 65 | 16% |

> Coverage % = widget usages / (widget usages + raw primitives) × 100. Files with all-zero counts are pure logic (no UI rendering) and are excluded from the ratio.

---

## 2. Top 10 Panels by Widget Adoption (most widget usages)

| Rank | File | Widget usages | Raw remaining | Coverage % |
|------|------|--------------|---------------|-----------|
| 1 | discord_panel.rs | 32 | 14 | 70% |
| 2 | apex_diagnostics.rs | 22 | 0 | 100% |
| 3 | watchlist_panel.rs | 12 | 65 | 16% |
| 4 | alerts_panel.rs | 11 | 3 | 79% |
| 5 | plays_panel.rs | 11 | 26 | 30% |
| 6 | scanner_panel.rs | 9 | 1 | 90% |
| 7 | script_panel.rs | 8 | 15 | 35% |
| 8 | signals_panel.rs | 6 | 6 | 50% |
| 9 | object_tree.rs | 5 | 5 | 50% |
| 10 | orders_panel.rs | 5 | 3 | 62% |

---

## 3. Top 10 Panels with Most Raw Primitives Remaining

| Rank | File | Raw total | Breakdown | Coverage % |
|------|------|-----------|-----------|-----------|
| 1 | watchlist_panel.rs | 65 | RichText×47, ui.button×12, TextEdit×4, egui::Frame×2 | 16% |
| 2 | indicator_editor.rs | 35 | RichText×20, ui.label×7, egui::Button::new×7, egui::Frame×1 | 8% |
| 3 | style.rs | 33 | ui.label×11, egui::Button::new×9, egui::Frame×3, corner_same×7, corner_lit×2, RichText×1 | 0% |
| 4 | plays_panel.rs | 26 | RichText×14, TextEdit×7, ui.label×5, widgets::pills×2 (raw label) | 30% |
| 5 | command_palette/render.rs | 15 | ui.label×7, RichText×7, TextEdit×1 | 12% |
| 6 | script_panel.rs | 15 | RichText×5, egui::Button::new×4, TextEdit×4, ui.label×1, egui::Frame×1 | 35% |
| 7 | discord_panel.rs | 14 | RichText×10, ui.label×3, TextEdit×1 | 70% |
| 8 | settings_panel.rs | 11 | egui::Button::new×4, RichText×4, TextEdit×2, egui::Frame×1 | 27% |
| 9 | journal_panel.rs | 10 | ui.label×5, RichText×5 | 17% |
| 10 | option_quick_picker.rs | 9 | ui.label×4, RichText×4, egui::Frame×1 | 10% |

---

## 4. By-Category Counts of Remaining Raw Primitives

| Category | Total sites | Files affected |
|----------|------------|----------------|
| `egui::RichText::new(` | **142** | 23 |
| `ui.label(` | **45** | 9 |
| `egui::Button::new(` | **30** | 8 |
| `egui::TextEdit::` | **25** | 10 |
| `egui::Frame::{popup\|none\|new}(` | **16** | 13 |
| `ui.button(` | **18** | 2 |
| `egui::CornerRadius::same(` | **7** | 1 (`style.rs`) |
| `egui::Slider::` | **2** | 1 (`rrg_panel.rs`) |
| `.corner_radius(\d` | **2** | 1 (`style.rs`) |
| `egui::ComboBox::` | **1** | 1 (`object_tree.rs`) |
| `.menu_button(` | **1** | 1 (`object_tree.rs`) |
| `ui.checkbox(` | **0** | 0 |
| `ui.radio_value(` | **0** | 0 |
| **Grand total** | **289** | — |

**Dominant pattern:** `egui::RichText::new` accounts for 49% of all remaining raw primitives (142 of 289), spread across nearly half the panel files. This is the single highest-leverage migration target.

---

## 5. Files at 100% Adoption

The following files have **zero** raw egui primitives in the audited categories. Files with widget usages > 0 are confirmed adopters; files with all-zero counts are pure logic/data files with no rendering calls.

**Confirmed widget-adopters with 0 raw primitives:**

| File | Widget usages |
|------|--------------|
| apex_diagnostics.rs | 22 |
| tape_panel.rs | 3 |
| dom_panel.rs | 2 |
| dashboard_pane.rs | 2 |
| seasonality_panel.rs | 2 |
| heatmap_pane.rs | 1 |
| portfolio_pane.rs | 1 |
| playbook_panel.rs | 1 |

**Pure logic/infrastructure files (no UI rendering):**
`chart_pane.rs`, `command_palette/execute.rs`, `command_palette/matcher.rs`, `command_palette/registry.rs`, `drawings.rs`, `indicators.rs`, `mod.rs`, `orders.rs`, `oscillators.rs`, `painter_chrome.rs`, `panels.rs`, `picker.rs`, `toolbar.rs`, `watchlist.rs`

---

## 6. Outstanding Widget Extension Targets

Migrating these raw patterns requires either new widgets or knobs on existing ones:

| Raw pattern | Count | Required extension |
|-------------|-------|-------------------|
| `egui::RichText::new` (inline styled text) | 142 | `widgets::text::label` with `.color()`, `.size()`, `.strong()`, `.monospace()` knobs — already partially exists; many call sites likely just need the knob wired |
| `ui.label` (plain string label) | 45 | Same `widgets::text::label` — simplest migration path; no new widget needed |
| `egui::Button::new` | 30 | Most are already covered by `widgets::buttons::*`; call sites need audit to confirm variant fit |
| `ui.button` (raw inline button) | 18 | `widgets::buttons::ghost` or `widgets::buttons::icon` depending on context |
| `egui::TextEdit::` (single-line inputs) | 17 | `widgets::inputs::text_input` — needs a `.id()` knob for persistent focus state |
| `egui::TextEdit::` (multiline) | ~8 | `widgets::inputs::text_input` **multiline mode** flag — currently missing |
| `egui::Frame::{popup\|none\|new}` | 16 | `widgets::frames::popup_frame` / `widgets::frames::card_frame` — most sites want a zero-padding container; a `widgets::frames::bare` variant would cover them |
| `egui::Slider::` | 2 | **New: `widgets::inputs::slider`** — not yet in the widget library |
| `egui::ComboBox::` | 1 | **New: `widgets::select::combo_box`** or extend `widgets::select::dropdown` |
| `.menu_button(` | 1 | `widgets::context_menu::` cascading item — evaluate whether existing context_menu handles nested menus |

**Priority order for new widgets:** (1) `widgets::inputs::text_input` multiline mode, (2) `widgets::inputs::slider`, (3) `widgets::frames::bare`, (4) `widgets::select::combo_box`.

---

## 7. Truly Bespoke Survivors

These sites intentionally use raw egui — no widget should absorb them:

| Location | Pattern | Rationale |
|----------|---------|-----------|
| `style.rs` — all 33 raw sites | `ui.label`, `egui::Button::new`, `egui::Frame`, `egui::CornerRadius::same`, `.corner_radius(\d` | This file IS the design-token/theme definition layer. It constructs the canonical styled examples shown in the style guide panel. Raw primitives here are the ground truth the widgets are derived from — they must not be replaced. |
| `indicator_editor.rs` — 35 raw sites | `ui.label×7`, `egui::Button::new×7`, `RichText×20` | Per-indicator property rows use per-row dynamic `CornerRadius` for segmented corner blending in the property grid. Requires direct egui control until a `widgets::form::property_row` abstraction exists. |
| `command_palette/render.rs` — `ui.label×7` | Highlighted match character rendering | Each matched character is wrapped individually in a `RichText` with a highlight color. This is a character-level decoration loop that cannot fold into a single `widgets::text::label` call without a span/run API. |
| `plays_panel.rs` — `TextEdit×7` | Multi-line trade note editing | Long-form freeform text entry with dynamic height; requires the not-yet-built `widgets::inputs::text_input` multiline mode. Intentional survivor until that mode ships. |
| `chart_widgets.rs` — `ui.button×6` | Inline painter-adjacent pixel buttons on the chart overlay bar | These buttons sit inside a `ui.painter()` layout context where the full widget stack is unavailable. Sacred painter-geometry zone — equivalent to chart paint. |
| `object_tree.rs` — `.menu_button×1`, `egui::ComboBox×1` | Cascading tree-node context menu + drawing-type picker | Cascading `menu_button` requires recursive egui response threading that the current `widgets::context_menu` does not expose. ComboBox is a one-of-a-kind picker for object types with heterogeneous item rendering. |
