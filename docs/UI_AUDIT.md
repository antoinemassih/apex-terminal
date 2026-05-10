# UI System — Comprehensive Audit
*Generated 2026-05-10 — covers `src-tauri/src/ui_kit/`, `src-tauri/src/chart/renderer/ui/`, and the `style.rs` / `gpu.rs` token foundations.*

## Execution log — tenth pass (2026-05-10, comprehensive forms buildout)

User asked for a comprehensive forms component buildout: survey what exists, audit hardcoded patterns, build what's missing, unify usage. Five sonnet agents in parallel built **10 new widgets** + fixed 5 existing ones. **Final aggregate `cargo check`: clean.**

**Buildout summary — 10 new ui_kit widgets:**

| # | Widget | File | Approx LOC | Purpose |
|---|---|---|---:|---|
| 1 | `TextArea` | `text_area.rs` | ~180 | Multiline text input (mirrors Input's animation/states) |
| 2 | `SearchInput` | `search_input.rs` | ~50 | Convenience wrapper: Input + magnifier icon + clearable |
| 3 | `SegmentedControl<T>` | `segmented_control.rs` | ~200 | Connected-pill selector for fixed-set picks |
| 4 | `ToggleGroup<T>` | `toggle_group.rs` | ~65 | Generic single-select button row (replaces NmfToggle hack) |
| 5 | `TagInput` | `tag_input.rs` | ~200 | Free-form chip entry with suggestions + max limit |
| 6 | `TimePicker` | `time_picker.rs` | ~220 | HH:MM(:SS) picker with hour/minute scroll columns + presets |
| 7 | `RangeSlider<T>` | `range_slider.rs` | ~210 | Dual-thumb min/max selector |
| 8 | `FormRow` | `form_row.rs` | ~140 | Label-left/top row with helper, error, required marker |
| 9 | `FormSection` / `FieldSet` / `FormActions` | `form_section.rs` | ~185 | Layout primitives: titled groups, bordered groups, footer button bars |
| 10 | `Icon::CLOCK` | `icons.rs` | +1 line | Added Phosphor `CLOCK` constant for TimePicker |

**Total new code:** ~1450 LOC of new reusable widgets.

**Existing widget gap fixes:**

| Widget | Fix |
|---|---|
| `Slider` | Added `.disabled(bool)` builder; thumb scale frozen, hover cursor suppressed, colors at 0.5× when disabled |
| `DatePicker` | Added `.disabled(bool)`; trigger disabled, popup never opens, colors faded |
| `Tag` | Added `.disabled(bool)`; close button disabled, colors faded uniformly |
| `NumberStepper` | Wired theme parameter into DragValue's frame styling — surface fill, border stroke, hover/active strokes, `radius_sm()` rounding |
| `ChoiceGrid` | Added `.disabled(bool)` + motion-based hover fade via `motion::ease_bool(FAST)`; cells now have proper interaction polish |

**Pre-existing build issue (not introduced by these agents):**

`OrderIntent` struct gained `strategy_id: Option<String>` and `override_warnings: bool` fields without all 18 call sites being updated. Verified resolved before final check — all sites have correct defaults (`None`, `false`).

**Coverage summary — what the design system now offers:**

✅ **Text inputs:** `Input` (single-line), `TextArea` (multiline), `SearchInput` (with icon)
✅ **Numeric:** `NumberStepper` (DragValue-based, theme-styled), `Slider` (single, w/ disabled), `RangeSlider` (dual-thumb)
✅ **Selection:** `Select` (dropdown, single + multi + searchable), `SegmentedControl` (pill row), `ToggleGroup` (button row), `Radio`, `Checkbox`, `Switch`, `ChoiceGrid`
✅ **Date/time:** `DatePicker` (w/ disabled), `Calendar`, `TimePicker`
✅ **Color:** `ColorPicker`
✅ **Tags:** `Tag` (display, w/ disabled), `Badge` (count), `TagInput` (entry)
✅ **Layout:** `FormRow` (label+control), `FormSection` (titled group), `FieldSet` (bordered group), `FormActions` (button bar footer)
✅ **Composite:** `ToggleRow` (label+desc+switch), `MetricRow` (label/value pair)

**What's left for future passes:**
- Adopt the new widgets in panels (orders, settings, indicator editor, scanner) — currently they exist but aren't yet used
- Deprecate renderer-side `TextInput`, `Dropdown`, `Combobox` (port their call sites to `Input`/`Select`)
- Port `chart/renderer/ui/inputs/form.rs::FormRow` callers to `ui_kit::FormRow`
- Form validation framework (centralized state machine for multi-field forms)
- Multi-step form / wizard composition primitive

---

## Execution log — ninth pass (2026-05-10, polish + registry wiring)

Three agents ran in parallel. **Build clean throughout.**

**Agent T — Warning cleanup:**
- Initial: 444 warnings → Final: 438 warnings
- Removed 6 unused imports across `tooltip.rs`, `hover_card.rs`, `tabs.rs`, `toggle_row.rs`, `choice_grid.rs`, `order_entry_panel.rs`
- 52 of remaining "unused import" warnings are **wildcard re-exports in `mod.rs` files** — deliberate API surface, can't be removed without restructuring exports. Audit's recommendation: leave them.
- Out-of-scope warnings (gpu.rs, data feeds, render/pane.rs) deferred per Don't-Touch list

**Agent U — Registry wiring proof-of-concept:**
- Migrated command-palette input handling from scattered `ctx.input(...)` checks to `ShortcutRegistry::matches(ctx)` dispatch
- Cmd+Space, Cmd+K (alt), and Esc now all driven by registered shortcuts via a single `match action {}` block
- Cmd+K was previously registered with no consumer — now functional
- Esc dismissal preserves ai_mode → open precedence logic
- **Validates the registry design**: scattered handlers can be replaced with declarative registration + central dispatch

**Agent V — Frame shadow audit:**
- 0 migrations needed in scope: all `Frame::shadow(...)` sites in panels/tools/cards/rows already migrated to `shadow_color_alpha(t, …)` in earlier waves
- 1 follow-up flagged: `panels/kit.rs:112` `paint_header_shadow` uses `from_black_alpha(42)` in a painter-mode mesh (not inside `Frame::shadow`); migrating would require threading `t: &Theme` into the function — out of scope for this pass

**Aggregate this wave:**
- 6 unused imports cleaned (warning count down 6)
- 1 input handler migrated to registry pattern (proof-of-concept)
- Frame shadow audit confirmed scope-clean

---

## Execution log — eighth pass (2026-05-10, WatchlistRow + DomRow brought into the system)

User clarified: they like the visual aesthetic of these two rows but want them under the design system. So this is a strict 1:1 token-swap pass — no pixel changes, only literals → tokens. Two agents in parallel, one per file. **Final aggregate `cargo check`: clean.**

**Agent R — `WatchlistRow` (~792 LOC) refactor:**
- ✅ Removed `fn fallback_theme()` (the THEMES[0] antipattern equivalent under a different name)
- ✅ Replaced 6 `fallback_theme()` call sites with the inline `let default_t = &THEMES[0]` pattern matching `order_row.rs`
- ✅ 3 `gamma_multiply` → color helpers: 1× `color_very_dim` (unpinned star), 2× `color_half` (correlation dot, hover X)
- 5 literals deliberately left: `gamma_multiply(0.2)` drag handle (no token), `color_alpha(_, 220)` and `color_alpha(_, 160)` RVOL strip (between tiers), `proportional(11.0)` ×4 (mono-only token set), `6.0` radius on earnings pill (passed as bare f32 to `rect_filled`)
- **Confidence: byte-for-byte visual fidelity preserved**

**Agent S — `DomRow` (~677 LOC) refactor:**
- ✅ 9 `gamma_multiply` → color helpers: 2× `color_muted(0.6)` (delta column), 5× `color_subtle(0.7)` (bid/ask normal, price imbalance), 2× `color_half(0.5)` (vol text)
- ✅ 1 `FontId::monospace(font_md())` → `mono_md()`
- ✅ 6 `CornerRadius::same(2.0)` → `radius_xs()` on order chips
- ✅ 2 alpha literals → `alpha_intense()` (140) and `alpha_prominent()` (180) on order chip fill/stroke
- 7 literals deliberately left: `1.5` and `1.0` radius on depth/vol bars (no exact token), `160` and `35` alphas (between tiers), 2× dynamic float multiplies in dragged-chip code (computed alpha can't safely literal-swap), pre-existing `fn fallback_theme()` (would need parameter threading through `draw_order_chip_label` — out of scope for token-only pass)
- **Confidence: byte-for-byte visual fidelity preserved**

**Aggregate this wave:**
- 12 `gamma_multiply` → color helpers
- 1 mono font literal → token
- 6 corner radii → tokens
- 2 alpha literals → tokens
- 1 `fn fallback_theme` removed (in WatchlistRow)
- 6 `THEMES[0]` call sites localized

**Both rows are now under the design system** — using `color_*`, `mono_*`, `radius_*`, `alpha_*`, `stroke_*` helpers wherever exact-tier matches existed, while preserving every pixel and behavior the user values.

---

## Execution log — seventh pass (2026-05-10, parallel sonnet agents, fourth wave)

Three sonnet agents on disjoint scopes. Two finished clean; the third (polish) over-reached and broke an import — fixed manually, build clean again. **Final aggregate `cargo check`: clean (444 warnings, mostly pre-existing).**

**Agent O — Final stroke literal sweep:**
- ✅ 13 stroke literals migrated across 10 panels not previously swept:
  - `analysis_panel`, `connection_panel`, `feed_panel`, `heatmap_pane`, `portfolio_pane`, `seasonality_panel`, `signals_panel`, `spreadsheet_pane`, `order_edit_dialog`, `pending_order_toasts`
- Imports added in 2 files (`order_edit_dialog`, `pending_order_toasts`)
- All exact-tier matches; non-tier values (10.0, fractional like 1.2) intentionally left

**Agent P — Keyboard-shortcut registry adoption:**
- ✅ 14 new shortcuts registered (15 total now in registry):
  - **Command palette** (3): `Cmd+Space`, `Cmd+K` (alt), `Esc` (modal.dismiss)
  - **Chart** (5): `Cmd+Z` undo, `Cmd+Shift+Z` redo, `Cmd+D` duplicate, `Cmd+Shift+S` screenshot, `M` magnet
  - **Trading** (7): `Cmd+B` buy, `Cmd+Shift+B` sell, `Cmd+Shift+Q` cancel-all, `Cmd+Shift+F` flatten, `Cmd+Shift+K` kill-switch, `Cmd+Shift+H` halt, `Cmd+Shift+R` resume
  - **Panels** (1): `Cmd+L` order-ledger toggle
- Registered via `Once`-guarded blocks in `command_palette/mod.rs`, `render/pane.rs`, `top_nav.rs`
- No conflicts at runtime
- Local widget shortcuts (Tab nav, single-key drawing tools, replay arrows) intentionally not registered to avoid false conflicts

**Agent Q — Polish pass (partial):**
- Started warning sweep but over-reached: removed `use super::super::widgets;` from `discord_panel.rs` while the file still uses `widgets::text::...` shorthand
- **Manual recovery**: re-added the import; build clean
- Other polish items (444 warnings) deferred — most are pre-existing `#[allow(dead_code)]` in scaffold areas

**Aggregate impact this wave:**
- 13 stroke literals → token helpers
- 14 keyboard shortcuts registered (registry now has real content for conflict detection + future help-screen generation)
- 1 import recovery (manual, post-agent)

---

## Execution log — sixth pass (2026-05-10, parallel sonnet agents, third wave)

Four sonnet agents on disjoint scopes. **Final aggregate `cargo check`: clean.**

**Agent K — Stroke + radius literal sweep:**
- ✅ 36 stroke literals migrated to `stroke_*()` helpers across `top_nav`, `chart_widgets`, `indicators_panel`, `chrome/pane`, `perf_hud`, `form`, `order_entry_panel`, `properties_bar`
- ✅ Imports added in `perf_hud`, `order_entry_panel`, `properties_bar`
- 0 corner-radius literals migrated — all `CornerRadius::same(...)` already used variables (`st.r_xs`, `card_radius`, etc.) — already token-driven
- 13 stroke literals remain in panels not in the dispatch list (analysis, connection, feed, heatmap_pane, portfolio, seasonality, signals, spreadsheet_pane, order_edit_dialog, pending_order_toasts) — left for follow-up
- 2 stroke literals at non-tier values (5.0, 3.5) intentionally left

**Agent L — Variant adoption (Pattern A: Chrome+frameless → TextOnly):**
- ✅ 8 sites migrated: `hotkey_editor.rs`, `discord_panel` (×2), `watchlist_panel` (×6 — plus, X, fav-circle ×2, chevron, X×0.3, fg×0.4 ×2)
- 0 Pattern B/C/D adoptions — every candidate had at least one shape override (`min_size`, `corner_radius`, `stroke`, or non-default color) that would have lost pixel precision
- 75+ remaining `Variant::Chrome`/`Ghost` sites are intentionally precision-overrides; not blanket-migrated
- ~16 LOC reduced

**Agent M — Font literal cleanup (proportional + RichText.size sweep):**
- ✅ 16 sites migrated:
  - `proportional(11.0)` → `font_sm()`: 5 sites (top_nav, watchlist_panel, pane.rs)
  - `proportional(13.0)` → `font_md()`: 2 sites
  - `proportional(16.0)` → `font_lg()`: 2 sites
  - `proportional(22.0)` → `font_xl()`: 2 sites
  - `.size(16.0)` → `.size(font_lg())`: 4 sites in top_nav menu buttons
  - `.size(22.0)` → `.size(font_xl())`: 1 site in command_palette
- Imports added in `top_nav.rs` (font_lg, font_xl, mono_lg) and `pane.rs` (font_sm, font_xl)
- Non-tier sizes (proportional 7.0/8.0/12.0/15.0/18.0/24.0) intentionally left

**Agent N — Hardcoded RGB → named color constants:**
- ✅ Added 4 constants to `style.rs`:
  - `COLOR_INFO_CYAN = (74, 158, 255)`
  - `COLOR_PROFIT_GREEN = (46, 204, 113)`
  - `COLOR_LOSS_RED = (231, 76, 60)`
  - `COLOR_PURPLE = (180, 100, 255)`
- ✅ Migration: 23 sites for INFO_CYAN, 9 for PROFIT_GREEN, 9 for LOSS_RED, 5 for PURPLE = **46 total**
- Imports added in `pane.rs` (4 constants), `trading/mod.rs` (1)
- All other files used existing `style::*` wildcard

**Aggregate impact across this single batch:**
- 36 stroke literals → token helpers
- 16 font literals → token helpers
- 46 RGB hardcodes → named color constants
- 8 Variant::Chrome escape hatches → Variant::TextOnly
- ~22 LOC reduced

---

## Execution log — fifth pass (2026-05-10, parallel sonnet agents, second wave)

Four sonnet agents ran in parallel on disjoint scopes. **Final aggregate `cargo check`: clean.**

**Agent G — Frame consolidation + theme-aware shadow sweep:**
- ✅ 3 hardcoded shadow sites migrated to `shadow_color_alpha(t, …)`:
  - `tools/option_quick_picker.rs:64` (`Color32::from_black_alpha(80)` → `shadow_color_alpha(t, 80)`)
  - `tools/template_popup.rs:34` (same)
  - `foundation/shell.rs:402` (CardShell drop shadow — now uses theme when present, falls back when not)
- ✅ Audited `frames_widget.rs` (31 sites): each is a *distinct primitive* (`CardFrame`, `DialogFrame`, `PopupFrame`, `TooltipFrame`, etc.) — no copy-paste duplication to extract
- 6 remaining shadow hardcodes flagged in files without `&Theme` in scope (`frames_widget`, `frames`, `style.rs::shadow_from_preset`, `kit::paint_header_shadow`, modal scrim) — would need API changes to migrate; documented as out-of-scope

**Agent H — Centralized keyboard-shortcut registry:**
- ✅ Created `src/foundation/shortcuts.rs` (97 lines): `Shortcut`, `ShortcutEntry`, `ShortcutRegistry` with conflict detection, lookup, by-category grouping, kbd-formatted display
- ✅ `pub mod shortcuts` added to `foundation/mod.rs`
- ✅ Demonstrator: `command_palette.open` registered via `Once`-guarded `ensure_registered()` in `command_palette/mod.rs::draw()` — additive, doesn't disturb existing `Cmd+Space` handler
- Foundation in place; future passes can migrate the 40+ scattered `ctx.input(...)` checks to use `registry().matches(ctx)`

**Agent I — Three widget extractions (visual fidelity preserved byte-for-byte):**
- ✅ `HeatmapGrid` widget: extracted 74 lines from `heat_panel.rs:147–220`, replacement is 10 lines. All intensity formulas, alpha values (12/120+135/190/230), `gap=3.0`, `col_w`, `cell_h(26/28)`, `font_sz(10/12)`, `stroke_bold()` active border preserved
- ✅ `TradeCard` widget: extracted 57 lines from `journal_panel.rs:209–265`, replacement is 15 lines. Card heights (52/66), `radius_sm()`, stripe geometry, row offsets, `color_subtle(accent)`, `color_half(dim)`, `0.35` gamma for notes preserved
- ✅ `GuildAvatarGrid` widget: extracted 66 lines from `discord_panel.rs:233–297`, replacement is 22 lines. `icon_size=32`, glow geometry, alpha tiers, gray levels (35/50/70), initials font (9/11) preserved
- Net **-150 LOC** in panels, +3 reusable widgets

**Agent J — MetricRow adoption + straggler cleanup + ui_kit `THEMES[0]` verification:**
- ✅ Task A: 1 `MetricRow` adoption in `orders_panel.rs` (Total P&L footer). Other panels intentionally skipped after careful audit — patterns are too complex for the simple label/value contract (custom indicators, multi-column grids, GPU painter calls)
- ✅ Task B: **19 `egui::FontId::monospace(7.5)` → `mono_3xs()` migrations in `pane.rs`** (rounded 0.5px down — imperceptible at chart-annotation scale). No `(8.5)` literals existed.
- ✅ Task C: **Zero `&THEMES[0]` references remain in `src/ui_kit/widgets/`** — confirmed via grep. The fourth-pass cleanup was complete.

**Aggregate impact across this single parallel batch:**
- 3 shadow color hardcodes fixed for light-theme parity
- New centralized shortcut registry (97 LOC) + 1 registered shortcut as proof-of-concept
- 3 reusable widgets extracted (-150 LOC in panels)
- 19 font literal stragglers eliminated in pane.rs
- 1 MetricRow adoption (with intentional 0 in other panels — visual-fidelity gates)
- Verified zero `THEMES[0]` ui_kit fallbacks remain

---

## Execution log — fourth pass (2026-05-10, parallel sonnet agents)

Six sonnet agents ran in parallel on disjoint file scopes. **Final aggregate `cargo check`: clean.** Each agent ran their own check at the end; the 17 "pre-existing errors" they each reported were transient (other agents mid-flight). With all six landings composited, build is green.

**Agent A — `THEMES[0]` sweep in `ui_kit/widgets/*.rs` Widget impls (24 sites):**
- ✅ Added `active_theme_idx(ctx) -> usize` and `active_theme(ctx) -> &'static Theme` helpers to `ui_kit/widgets/theme.rs`
- ✅ Stashed active theme idx per-frame in `gpu.rs::setup_theme` (~line 3494) via `ctx.data_mut(|d| d.insert_temp(...))`
- ✅ Updated 24 widget impl fallbacks: `alert`, `badge`, `button`, `calendar`, `checkbox`, `date_picker`, `indicator`, `kbd`, `label`, `link`, `polished_label`, `progress`, `radio`, `selectable_row`, `separator`, `skeleton`, `slider`, `spinner`, `stepper`, `switch`, `tag`, `toggle_row`, `toast`, `mod.rs::delete_button`
- Now `ui.add(Button::new(...))` (no explicit `.show(ui, theme)`) honors the active theme instead of always-Midnight

**Agent B — `fn ft()` sweep in `lists/cards/*.rs` and 5 safe row files (14 sites):**
- ✅ Cards: `trade_card`, `stat_card`, `signal_card`, `news_card`, `metric_card`, `event_card`, `earnings_card`, `widgets/cards/mod.rs`
- ✅ Rows: `alert_row`, `news_row`, `option_chain_row`, `order_row`, `table`, `widgets/rows/mod.rs`
- All 14 `fn ft()` deleted; pattern: `let default_t = &THEMES[0]; ... self.theme.unwrap_or(default_t)` localized to `show()`
- **`watchlist_row.rs` and `dom_row.rs` untouched** per user instruction (aesthetic preserved)

**Agent C — `egui::Window` → ui_kit `Modal`/`Sheet` (3 migrated, 4 retained with reasons):**
- ✅ `tools/order_edit_dialog.rs:64` → `Modal::new(...).anchor(Anchor::Area).header_style(HeaderStyle::Dialog)` + `shadow_color_alpha(c.t, 80)`
- ✅ `tools/trendline_filter.rs:202` → `Modal` (removed manual click-away)
- ✅ `tools/overlay_manager.rs:17` → `Modal::draggable_header(true)`
- Retained as `Window` with documented reasons: `order_entry_panel.rs:39+69` (custom drag delta), `pending_order_toasts.rs:32` (per-order accent + chart-relative position), `trendline_filter.rs:20` (wraps FloatingPaneChrome), `perf_hud.rs:54` (collapsible/resizable dev tool)

**Agent D — `gamma_multiply(0.X)` → `color_*()` helpers (~165 replacements across 22 files):**
- Toolbar: `top_nav.rs` (16 replacements)
- Chart: `chart_widgets.rs` (~55)
- Panels: `discord_panel` (9), `plays_panel` (19), `script_panel` (5), `journal_panel` (7), `feed_panel` (1), `connection_panel` (1), `scanner_panel` (3), `orders_panel` (3), `apex_diagnostics` (4), `indicators_panel` (5)
- Chrome + tools + cards: `painter_pane` (4), `floating_pane` (2), `play_card` (7), `trendline_filter` (5), `order_entry_panel` (12), `indicator_editor` (3)
- Skipped 30+ values at non-tier multipliers (`0.2`, `0.25`, `0.35`, `0.55`, `0.8`, `0.85`, `0.92`, `0.95`) — left as `gamma_multiply` with no clean tier match

**Agent E — Motion + focus + a11y polish:**
- ✅ Added 4 motion tokens: `INSTANT(0.03)`, `DELAY_TOOLTIP(0.4)`, `DELAY_HOVER_CARD(0.6)`, `SCROLL_EASE_DURATION(0.20)`. Wired into `motion.rs`, `tooltip.rs`, `hover_card.rs`
- ✅ Added `StyleSettings::animations_enabled: bool` (default true) — `ease_bool` + `ease_value` early-return when off
- ✅ Added focus-ring overlay in `paint_button` — drawn outside rect on `response.has_focus()`, additive (doesn't replace resting border)

**Agent F — Widget extractions:**
- ✅ Created `ui_kit/widgets/opacity_picker.rs` (`OpacityPicker` builder) — extracted from `object_tree.rs:75–108` (-46 lines net)
- ✅ Created `ui_kit/widgets/risk_reward_bar.rs` (`RiskRewardBar` builder) — extracted from `plays_panel.rs:399–406` (-8 lines net)
- Both re-exported from `ui_kit::widgets::mod`. Net **-54 LOC** in panels, +2 reusable widgets

**Aggregate impact across this single parallel batch:**
- ~165 gamma_multiply calls migrated to color helpers
- 24 widget impl `THEMES[0]` fallbacks fixed via per-frame ctx stash
- 14 `fn ft()` helpers deleted from cards/rows
- 3 `egui::Window` → `Modal` migrations (with 4 well-documented retentions)
- 4 motion tokens added + reduced-motion gate + focus-ring overlay
- 2 new shared widgets extracted (-54 LOC)

---

## Execution log — third pass (2026-05-10, even later)

**Massive font-literal sweep:**
- ✅ `chart_widgets.rs` — `FontId::monospace(6.0)` → `mono_4xs()` (~32 sites)
- ✅ `pane.rs` — full sweep: `monospace(6.0)` → `mono_4xs`, `(7.0)` → `mono_3xs`, `(8.0)` → `mono_2xs`, `(9.0)` → `mono_xs`, `(10.0)` → `mono_xs_plus`, `(11.0)` → `mono_sm`, `(13.0)` → `mono_md`, `(14.0)` → `mono_md_plus` (~50 sites)
- ✅ `top_nav.rs` — `monospace(11.0)` → `mono_sm()` (~6 sites)
- ✅ `form.rs` — `monospace(11.0)` → `mono_sm()` (~8 sites)
- ✅ Mass batch (one `replace_all` per file) — `monospace(11.0)` → `mono_sm()` across 17 files: `dom_panel`, `plays_panel`, `settings_panel`, `watchlist_panel`, `spread_panel`, `script_panel`, `alert_row`, `lists/table`, `rrg_panel`, `scanner_panel`, `tape_panel`, `journal_panel`, `watchlist_row`, `components/status`, `watchlist_columns`, `order_row`, `option_chain_row`, `news_row`, `dom_row`, `seasonality_panel`, `portfolio_pane`, `widgets/rows/mod`
- ✅ `design_inspector.rs` — `monospace(11.0)` → `style::mono_sm()` (3 sites)
- ✅ `dom_panel.rs` + `pane.rs` — `monospace(13.0)` → `mono_md()`

**`mono_*` import added to** `pane.rs` and `top_nav.rs` (the only files that didn't already have `style::*` wildcard).

**Total font-literal sites converted in this pass: ~150 across 23 files.**

**`DragValue` → `NumberStepper` migrations:**
- ✅ `indicator_editor.rs` (9 sites: MACD slow/signal, Stochastic %D, BollingerBands mult, KeltnerChannels/Supertrend mult, Ichimoku kijun/senkou, ParabolicSAR start/step/max)
- ✅ `settings_panel.rs` (4 sites: Stock Qty, Options Qty, Max Order Qty, Max Position) — 3 conditional-formatter sites left as `DragValue` (Max Notional, Fat Finger %, Max Daily Loss with "OFF" sentinel)
- ✅ `NumberStepper` extended with `.decimals(n)` / `.integer()` to cover the integer-cast formatters

**Build status: clean throughout. `cargo check` ran after every batch.**

**Stragglers (`FontId::monospace(N.0)` with N not in token tier):**
- `pane.rs` — 22 remaining at sizes `7.5`/`8.5` (between `mono_3xs` and `mono_2xs`) — chart annotations needing sub-token sizing
- `chart_widgets.rs` — 3 stragglers
- `lists/rows/table.rs` — 1 straggler
- `design_inspector.rs` — 20 stragglers at sizes used for the design tools UI itself

These don't have clean tier mappings; left as-is. Future work could add 7.5/8.5 helpers if the count grows.

---

## Execution log — second pass (2026-05-10, later)

**Variant adoption (the Chrome escape hatch shrinks):**
- ✅ `inputs/filter_pill.rs` → `Variant::Chip` (5 lines of override → 1 line)
- ✅ `inputs/nmf_toggle.rs` → `Variant::Chip` (8 lines of override → 1 line)
- ✅ `panels/analysis_panel.rs:90–98` tab strip → `Variant::Tab` (5 lines × N → 1 line × N)
- ✅ `panels/analysis_panel.rs:102` close button → `Variant::InlineClose` (4 lines → 1 line) + `Icon::X` instead of `\u{00D7}`
- ✅ `chrome/floating_pane.rs:185–190` close → `Variant::InlineClose` (5 lines → 1 line)
- ✅ `top_nav.rs:810,813` indicator edit/delete → `Variant::MutedIcon`
- ✅ `panels/dom_panel.rs:405–411` FLATTEN → `Variant::NeutralAction` (4 lines of fill/fg/min_size → 1 line)

**Hardcoded-theme cleanup:**
- ✅ `panels/object_tree.rs` — `fn ft() -> &THEMES[0]` deleted; `sig_color(score, t: &Theme)` now threads the active theme. Last on-purpose `THEMES[0]` in panels gone.
- ✅ `panels/discord_panel.rs:223` — `color_alpha(Color32::BLACK, alpha_tint())` → `shadow_color_alpha(t, alpha_tint())`. Server-strip background now respects light themes.
- ✅ `tools/order_edit_dialog.rs:75` — modal shadow → `shadow_color_alpha(c.t, 80)`
- ✅ `render/pane.rs:6968` — alert pill border → `shadow_color_alpha(t, 120)`
- ✅ `render/pane.rs:7012` — X-hover background → `shadow_color_alpha(t, 80)` + `radius_sm()` token

**New widgets shipped:**
- ✅ `ui_kit::NumberStepper` (`number_stepper.rs`) — `egui::DragValue` replacement with token-driven font sizing
- ✅ `ui_kit::MetricRow` (`metric_row.rs`) — "Label: Value" composition with semantic `Tone` (Default/Muted/Accent/Bull/Bear/Warn) + optional progress bar

**Dead-code removal:**
- ✅ `foundation/tokens.rs::Density` enum removed (zero callers — the alive density knob is `StyleSettings::density: u8`)
- ✅ `components/chips.rs::filter_chip()` removed (deprecated v0.10, zero callers)
- ✅ `components/pills.rs::pill_btn()` removed (deprecated v0.10, zero callers)

**Build status: clean throughout. `cargo check` ran after every batch — no errors introduced.**

---

## Execution log (2026-05-10)

**Tier 1 fixes — done, build clean:**
- ✅ Token foundation reconciled in `style.rs`:
  - Real `font_4xs/3xs/2xs/xs/xs_plus/sm/md/md_plus/lg/xl` tiers (was 4 tiers + aliases that lied)
  - Matching `mono_*` companions
  - `FONT_*` consts agree with the function returns
  - `gap_2xs()` is now `2.0` (was a duplicate of `gap_xs`)
  - `radius_*()` function fallbacks reconciled with `RADIUS_*` consts (4/6/12); added `radius_xs()` and `radius_pill()`
  - `stroke_medium/extra_thick/heavy` added for the 0.8/2.5/3.0 hardcodes
  - Alpha tier expanded with `whisper(25) / hint(30) / intense(140) / prominent(180) / near_opaque(230)` to absorb literal alphas
  - Existing alpha values preserved → no visual shifts
  - `color_subtle/muted/half/dim/very_dim` helpers replace ad-hoc `gamma_multiply` chains
  - `shadow_color_alpha(t, a)` helper for theme-aware shadows
- ✅ Hardcoded black shadows fixed in `panels/plays_panel.rs` (card drop shadow), `panels/dom_panel.rs` (inset gradient)
- ✅ Hardcoded `Color32::WHITE` on icon hover (`render/pane.rs:6677`) → `contrast_fg(t.bear)` (works on light themes)
- ✅ Drawing-selection `Color32::WHITE` → `t.accent` everywhere `if is_sel { WHITE } else { dc }` patterns appeared (≈10 sites in `render/pane.rs`)
- ✅ `&THEMES[0]` removed from `chrome/pane.rs:1011` — `FloatingOrderPaneChrome` now stores `theme_ref: Option<&'a Theme>` set by `.theme(t)`; falls back only if caller forgot
- ✅ `Button::show_menu(ui, theme, body)` added to `ui_kit/widgets/button.rs` — renders the trigger via the design system, body runs in egui menu popup
- ✅ Unicode glyphs → Phosphor:
  - IBKR connection ●/○ → `Icon::CIRCLE_FILL` / `Icon::CIRCLE` (`top_nav.rs:355`)
  - Clear-button `\u{2715}` → `Icon::X` (`ui_kit/widgets/input.rs:268`)
  - Plays panel `\u{00D7}` → `Icon::X` (3 sites in `panels/plays_panel.rs`)
  - Spread panel `"+"` / `"-"` → `Icon::PLUS` / `Icon::MINUS` (`panels/spread_panel.rs:374,376`)
  - Chart-widget icons in `chart/renderer/mod.rs`: 8 mappable emoji replaced with Phosphor (📅→CALENDAR_BLANK, 📰→NEWSPAPER, ⚡→LIGHTNING, 📊→CHART_BAR, 📈→CHART_LINE, 💰→CURRENCY_DOLLAR, ⚙→GEAR, $→CURRENCY_DOLLAR). Block-shading and arrow glyphs left intentional.

**Still in punch list (priority order):**

*Tier 1 remaining:*
- Sweep the **24 `THEMES[0]` fallbacks in `ui_kit/widgets/*.rs` Widget impls** — the right fix is `ctx`-stored active theme; intermediate fix is to delete the `Widget` impl and force `.show(ui, theme)` everywhere
- Sweep the **32 `fn ft() -> &Theme` helpers in `chart/renderer/ui/`** — make every UI fn accept `theme: &Theme` instead

*Tier 2 (large/multi-file):*
- Apply `color_subtle/muted/dim/very_dim` helpers to **30+ `gamma_multiply` chains** (need to add `use super::style::color_*` to many files first)
- Replace **83 `egui::Button` direct calls** with `ui_kit::Button` (mostly in `chart/renderer/ui/components/`)
- Replace **33 `ui.menu_button(...)` direct calls** with the new `Button::show_menu()`
- Replace **100+ hardcoded font sizes** with new `font_*` tier helpers (e.g. `FontId::monospace(8.0)` → `FontId::monospace(font_2xs())`)
- Replace **250+ literal stroke widths** with `stroke_*()` calls
- Replace **80+ literal radii** with `radius_*()` calls
- Decide density: wire `compact_mode` through `Size::height()` and `gap_*()`, OR delete the dead `Density` enum and `compact_mode` flag (currently affects exactly one site: chart top padding)
- Add **central keyboard-shortcut registry**; surface conflicts; auto-generate help; wire `Kbd` widget to all bindings
- Apply **`focus_ring()`** consistently to all interactive widgets via `apply_interaction()`
- Add **`prefers-reduced-motion`** opt-out (`style_settings.animations_enabled` gate in `ease_bool/ease_value`)
- Add **`DELAY_TOOLTIP / DELAY_HOVER_CARD / INSTANT`** motion tokens; replace 4 hardcoded `0.06 / 0.4 / 0.6 / 0.20` durations
- Replace remaining ~40 chart-widget Unicode glyphs in `chart/renderer/mod.rs` that have intentional non-Phosphor styling — decide policy per glyph (block-shading characters should stay; rotation/arrows could become Phosphor `ARROW_*`)
- Extract shared widgets: `StatBox` (24+ sites), unified `Section` (3 implementations today), `IconBadge`, `EmptyState` (promote from panels/kit), `LabeledSwitch`, `ConfirmDialog`
- Add 6+ missing `Button` variants (`menu` ✅ done; remaining: `icon_muted`, `danger_ghost`, `status_icon`, `category_tinted`, `secondary_muted`)

*Tier 3 (docs/process):*
- Promote `style.rs` header doc → `docs/UI_TOKENS.md` cheatsheet
- Add field doc comments to `Theme` struct in `gpu.rs` (currently zero `///` comments on 85 fields)
- Add module-level doc to `gpu.rs` (file has no top-level `//!`)
- Light-theme visual regression test (Bauhaus diff vs Midnight)

*Tier 4 (lints/CI):*
- CI grep for: `Color32::from_rgba_unmultiplied(0, 0, 0,`, `&THEMES[0]`, `FontId::monospace(\d+\.\d)` outside style.rs, `gamma_multiply\(0\.\d+\)`
- Audit `Theme` for legacy field removal (60+ candidates)

---


> **TL;DR.** The design system has solid bones — `Theme`, `ComponentTheme`, `StyleSettings`, `style.rs` token helpers, `ui_kit::widgets::Button` — but **31% of UI values are still hardcoded** and **56 widgets fall back to `THEMES[0]`** instead of threading the active theme. The biggest leverage points: kill the `THEMES[0]` fallback, add 5–6 missing token tiers (sub-`font_xs` sizes, gamma helpers, badge dimensions), extract 3 new shared widgets (StatBox, SectionHeader, Button::menu), and fix the hardcoded-black shadow problem that breaks light themes.

---

## Table of contents

1. [Inventory snapshot](#1-inventory-snapshot)
2. [Token system](#2-token-system)
3. [Component inventory](#3-component-inventory)
4. [Hardcoded values catalog](#4-hardcoded-values-catalog)
5. [Missing tokens & helpers](#5-missing-tokens--helpers)
6. [Missing components & variants](#6-missing-components--variants)
7. [Theme threading — `THEMES[0]` antipattern](#7-theme-threading--themes0-antipattern)
8. [Light-theme compatibility (critical)](#8-light-theme-compatibility-critical)
9. [Documentation gaps](#9-documentation-gaps)
10. [Inconsistencies — same intent, different code](#10-inconsistencies--same-intent-different-code)
11. [Pattern duplication — what gets reimplemented](#11-pattern-duplication--what-gets-reimplemented)
12. [Optimizations](#12-optimizations)
13. [Prioritized punch list](#13-prioritized-punch-list)

---

## 1. Inventory snapshot

| Dimension | Count | Status |
|---|---|---|
| Token-defining files | 5 | Solid (some overlap) |
| Named tokens | 80+ functions/constants | Good coverage, gaps in sub-9px font + gamma helpers |
| Theme palettes | 15 (12 dark, 3 light + 1 mid-light) | Light themes break under hardcoded black shadows |
| `ui_kit` widgets | 47 | Most underdocumented; one (Button) is exemplary |
| `chart/renderer/ui` widgets/helpers | 30+ free fns + custom widgets | Heavy duplication of Button-shaped patterns |
| Hardcoded violations (total) | 1210+ | 31% of all UI values |
| `THEMES[0]` fallback sites | 56 | Almost every `ui_kit::widgets/*.rs` falls back |
| `egui::Button` direct calls | 83 | Should be `ui_kit::Button` |
| `ui.menu_button(...)` direct calls | 33 | No `Button::menu` variant exists |
| `painter.rect_filled + painter.text` ad-hoc widgets | 24+ | Should be `StatBox` / `StatusLabel` |
| `.fg(...)` / `.glyph_color(...)` overrides on Button | 50+ | Indicates 8–12 missing variants |
| Hardcoded black shadows (`Color32::from_rgba_unmultiplied(0,0,0,...)`) | 6+ | **Breaks 4 light themes** |

---

## 2. Token system

### 2.1 Token sources (5 files)

| File | Role |
|---|---|
| `src/ui_kit/widgets/tokens.rs` | `Size`/`Variant`/`Density`/`Radius` widget enums |
| `src/chart/renderer/ui/style.rs` | Token *functions* (`font_xs()`, `gap_md()`, etc.) and consts (`FONT_XS`, `GAP_MD`) — single source of truth for visual tokens |
| `src/chart/renderer/gpu.rs` | `Theme` struct (85+ fields), `THEMES` array (15 themes), `hairline_border()` color synthesizer |
| `src/foundation/design_tokens.rs` | Runtime design-mode token storage (`dt_f32!`, `dt_u8!` macros) — overrides at runtime when feature is on |
| `src/chart/renderer/ui/foundation/tokens.rs` | Foundation `Size`/`Density`/`Radius` (parallel to `ui_kit/widgets/tokens.rs`) — **duplication** |

> 🔧 **Issue.** Two `Size` enums exist — `ui_kit::widgets::tokens::Size` and `chart_renderer::ui::foundation::tokens::Size`. They differ slightly (foundation has `Xl`, ui_kit doesn't). Pick one and remove the other.

### 2.2 Typography

| Token | Value | Use cases | Hardcoded violations |
|---|---|---|---|
| `font_xs()` | 9.0 | Badge text, micro-labels, dropdown items | 124+ correct uses |
| `font_sm()` | 11.0 | Default body, list rows, tab labels | 156+ correct uses |
| `font_md()` | 13.0 | Emphasized body, panel titles | 89+ correct uses |
| `font_lg()` | 16.0 | Section headers, modal titles | 102+ correct uses |
| `mono_xs/sm/md/lg()` | wraps above with `Monospace` family | Financial data | underused (4–12 each) |
| **`font_2xs()`** | aliased to `font_xs()` | Legacy | should be **a real token at 7.0–8.0** (see [§5](#5-missing-tokens--helpers)) |

**Hardcoded fonts** (top offenders):
- `FontId::monospace(6.0)` × 32 in `chart_widgets.rs` (RSI zone labels, market phase labels) — micro-labels below the scale floor
- `FontId::monospace(7.0)` × 24 in `render/pane.rs` (volume ratio, trade entries)
- `FontId::monospace(8.0)` × 32 (price axis order labels, axis tick labels)
- `FontId::monospace(10.0)` × 39 — between `font_xs` (9) and `font_sm` (11), no token
- `FontId::monospace(14.0)` × 10 in pane.rs (large chart annotations) — between `font_md` and `font_lg`

### 2.3 Spacing

| Token | Value | Use cases |
|---|---|---|
| `gap_2xs()` / `gap_xs()` | 4.0 | Intra-cluster (between adjacent buttons) |
| `gap_sm()` | 8.0 | Default inter-element |
| `gap_md()` | 12.0 | Section padding |
| `gap_lg()` | 16.0 | Panel inner margin |
| `gap_xl/2xl/3xl()` | 20/24/32 | Between sections, panel groups, page breaks |

> ⚠️ `gap_2xs()` and `gap_xs()` both return `4.0`. One should be `2.0` (intra-icon padding) — there are 9 hardcoded `2.0` spacings indicating the gap.

**Hardcoded spacings** (top): 151× `4.0` (most correct), 78× `8.0` (most correct), 13× `6.0` (no token), 9× `2.0` (no token), 8× `3.0` (no token).

### 2.4 Strokes

| Token | Value |
|---|---|
| `stroke_hair()` | 0.3 |
| `stroke_thin()` | 0.5 |
| `stroke_std()` | 1.0 |
| `stroke_bold()` | 1.5 |
| `stroke_thick()` | 2.0 |

**Hardcoded violations**: 17× `0.8` (no token), 5× `2.5` (no token), 3× `1.2` (no token), plus 66× `0.5` and 123× `1.0` literals that should call the token functions.

### 2.5 Alpha

| Token | Value | Intent |
|---|---|---|
| `alpha_faint()` | 10 | Barely visible hints |
| `alpha_ghost()` | 15 | Very subtle overlay |
| `alpha_soft()` | 20 | Soft tint |
| `alpha_subtle()` | 40 | Light accent |
| `alpha_tint()` | 48 | Tint overlay |
| `alpha_muted()` | 60 | UI chrome |
| `alpha_dim()` | 60 | (duplicate of muted) |
| `alpha_line()` | 80 | Hairline borders |
| `alpha_strong()` | 80 | (duplicate of line) |
| `alpha_active()` | 100 | Active/hovered |
| `alpha_heavy()` | 120 | Very prominent |
| `alpha_solid()` | 200 | Nearly opaque |

> 🔧 **Issue.** `alpha_muted()` and `alpha_dim()` are both `60`; `alpha_line()` and `alpha_strong()` are both `80`. Either disambiguate the values or collapse the pairs.

**Hardcoded alpha violations**: 18× `180` (between heavy and solid), 8× `230`, 8× `30`, 6× `25` — none of these have tokens.

### 2.6 Corner radii

| Token | Function fallback | Const value |
|---|---|---|
| `radius_sm()` | `dt_f32!(radius.sm, 3.0)` | `RADIUS_SM = 4.0` |
| `radius_md()` | `dt_f32!(radius.md, 4.0)` | `RADIUS_MD = 6.0` |
| `radius_lg()` | `dt_f32!(radius.lg, 8.0)` | `RADIUS_LG = 12.0` |

> 🔧 **Issue.** Function fallbacks (3/4/8) and const values (4/6/12) **disagree**. This will produce inconsistent radii depending on whether the caller uses `radius_sm()` or `RADIUS_SM`.

### 2.7 Theme palette

`Theme` struct in `gpu.rs:104` has **85+ fields**. The intentional core:
- **Surfaces**: `bg`, `toolbar_bg`
- **Foreground core (6)**: `accent`, `bull`, `bear`, `text`, `dim`, `toolbar_border`
- **State overlays**: `element_hover/active/selected/disabled`, `ghost_hover/active`
- **Icon ramp**: `icon`, `icon_muted`, `icon_disabled`, `icon_accent`
- **Other**: `warn`, `border_variant`, `cmd_palette[11]`, `shadow_color`, `notification_red`, `gold`, `overlay_text`, RRG colors, hud bg/border

The remaining 60+ fields are flagged `LEGACY:` in source comments and should be derived via `color_alpha`/`gamma_multiply` instead of stored.

---

## 3. Component inventory

### 3.1 `ui_kit/widgets/` (47 files)

**Well-themed and exemplary:**
- **Button** (`button.rs`) — 305+ usages. Variants: `Primary | Secondary | Ghost | Danger | Link | Chrome`. Sizes: `Xs/Sm/Md/Lg`. Takes `&dyn ComponentTheme`. Best documentation of any widget.
- **Select** (`select.rs`) — 50+ usages. Single/multi/searchable, sticky-last, custom render fns.
- **Tooltip** (`tooltip.rs`) — 40+ usages. Text + Rich variants, configurable delay.
- **Kbd** (`kbd.rs`) — 20+ usages. Keyboard shortcut chip.
- **Badge** (`badge.rs`) — 30+ usages. Count/Dot/Text with TagTone.

**Other ui_kit widgets** (mostly underdocumented, varying maturity):
alert, breadcrumb, calendar, checkbox, choice_grid, color_picker, context_menu, date_picker, disclosure, hover_card, indicator, input, label, link, modal, motion (easing fns), pagination, polished_label, popover, progress, radio, resizable, scroll_area, separator, sheet, shadow, sidebar, skeleton, slider, spinner, stepper, switch, tabs, tag, text_engine, text_subpixel_pipeline, toast, toggle_row, selectable_row, tree.

### 3.2 `chart/renderer/ui/` widgets

**`/components/`**:
- `action_button.rs` — `big_action_btn`, `side_pane_action_btn`, `brand_cta_button` (legacy free fns wrapping `egui::Button`)
- `chips.rs` — `keybind_chip`, `notification_badge`, `display_chip`, `removable_chip` (deprecated `filter_chip`; says "use pill_button instead")
- `pills.rs` — `pill_button` (canonical), `status_badge`, `status_pill`
- `header_buttons.rs`, `headers.rs`, `inputs.rs`, `menus.rs`, `metrics.rs`, `panels.rs`, `perf_hud.rs`, `semantic_label.rs`, `sortable_headers.rs`, `status.rs`, `text.rs`, `toasts.rs`, `toolbar/`

**`/inputs/`**: `inputs.rs` (TextInput/NumberInput/TextArea), `select.rs` (RadioGroup, SegmentedControl), `filter_pill.rs`, `form.rs`, `nmf_toggle.rs`.

**`/widgets/`**: row renderers (`watchlist_row`, `alert_row`, etc.), card renderers (`earnings_card`, `metric_card`, `play_card`, etc.), table widget.

### 3.3 Naming inconsistencies

| Concept | Names found | Recommendation |
|---|---|---|
| Button | `Button`, `tb_btn`, `toolbar_btn`, `big_action_btn`, `side_pane_action_btn`, `action_btn`, `brand_cta_button`, `IconBtn`, `TradeBtn`, `ChromeBtn`, `SimpleBtn`, `ToolbarBtn` | Keep only `ui_kit::Button` + variants; delete free fns |
| Chip / pill / badge | `Badge`, `chip`, `keybind_chip`, `pill_button`, `notification_badge`, `status_badge`, `status_pill`, `display_chip`, `removable_chip`, `Tag`, `FilterPill` | Consolidate into `Badge`/`Tag`/`Pill` triad |
| Section header | `PanelSection`, `SectionHeader`, free fn `section_header()` | Collapse to one widget |
| Toggle / switch | `Switch`, `toggle_switch()` (free fn), `Toggle`, `ToggleRow`, `NmfToggle` | Standardize on `Switch` for binary, `Toggle` for radio-like |
| Text input | `Input` (ui_kit), `TextInput` (renderer), `text_input_field()` | Pick one |

---

## 4. Hardcoded values catalog

### 4.1 Headline numbers

| Category | Total uses | Correct (token) | Hardcoded | % wrong |
|---|---:|---:|---:|---:|
| Font sizes | 250 | 150 | 100 | **40%** |
| Spacing | 350 | 280 | 70 | 20% |
| Strokes | 260 | 240 | 20 | 8% |
| Corner radii | 80 | 55 | 25 | 31% |
| Alpha values | 150 | 90 | 60 | **40%** |
| Color RGB triples | 40+ | 20 | 20+ | **50%** |
| Magic dimensions (heights/widths) | 80+ | n/a | 80+ | **100%** |
| **Totals** | **1210+** | **835 (69%)** | **375+ (31%)** | |

### 4.2 Top 10 worst offender files

| Rank | File | Violations |
|---:|---|---:|
| 1 | `chart/renderer/render/pane.rs` | 498 |
| 2 | `chart/renderer/ui/chart_widgets.rs` | 93 |
| 3 | `chart/renderer/ui/style.rs` | 39 |
| 4 | `chart/renderer/ui/inputs/form.rs` | 32 |
| 5 | `chart/renderer/gpu.rs` | 31 |
| 6 | `chart/renderer/ui/components/toolbar/top_nav.rs` | 16 |
| 7 | `chart/renderer/ui/panels/indicators_panel.rs` | 14 |
| 8 | `chart/renderer/ui/foundation/shell.rs` | 12 |
| 9 | `chart/renderer/ui/panels/plays_panel.rs` | 11 |
| 10 | `chart/renderer/ui/panels/discord_panel.rs` | 11 |

### 4.3 Most repeated hardcoded RGB triples

| RGB | Count | Should be |
|---|---:|---|
| `(255, 191, 0)` AMBER | 42 | `t.warn` or `t.gold` |
| `(74, 158, 255)` CYAN BLUE | 24 | new `t.info` token |
| `(231, 76, 60)` RED | 9 | `t.bear` or `t.notification_red` |
| `(46, 204, 113)` GREEN | 5+ | `t.bull` |
| `(180, 100, 255)` PURPLE | 8 | new theme slot |

### 4.4 `.gamma_multiply()` chain hot spots

The codebase has **30+** `gamma_multiply(0.X)` calls indicating missing color-dimming tokens:

| Multiplier | Count | Suggested token |
|---|---:|---|
| `0.3` | 8 | `color_very_dim(c)` |
| `0.4` | 10 | `color_dim(c)` |
| `0.5` | 6 | `color_half(c)` |
| `0.6` | 15 | `color_muted(c)` |
| `0.65` | 4 | (use `color_muted`) |
| `0.7` | 12 | `color_subtle(c)` |
| `0.95` | 1 | (likely a bug — almost no change) |

---

## 5. Missing tokens & helpers

### 5.1 Typography (high priority)

| Proposed | Value | Justification |
|---|---|---|
| `font_2xs()` (real, not aliased) | 8.0 | 32 hardcoded uses for badge/overlay text |
| `font_3xs()` | 7.0 | 24 hardcoded uses for chart micro-labels |
| `font_4xs()` | 6.0 | 32 hardcoded uses (RSI zones, market phase) — *or* document a hard floor and refactor away |
| `font_xs_plus()` | 10.0 | 39 hardcoded uses (between xs and sm) |
| `font_xl()` (real, not aliased) | 14.0 | 10 hardcoded uses for large chart annotations |

### 5.2 Spacing

| Proposed | Value | Justification |
|---|---|---|
| `gap_2xs()` real value | 2.0 | Distinguish from `gap_xs()` (4.0); 9 hardcoded uses |
| `gap_sm_tight()` | 6.0 | 13 hardcoded uses |
| `gap_md_tight()` | 10.0 | recurring 10.0 padding |

### 5.3 Color dimming helpers (replace `gamma_multiply` calls)

```rust
pub fn color_subtle(c: Color32) -> Color32 { c.gamma_multiply(0.7) }   // 12+ uses
pub fn color_muted(c: Color32) -> Color32  { c.gamma_multiply(0.6) }   // 15+ uses
pub fn color_dim(c: Color32) -> Color32    { c.gamma_multiply(0.4) }   // 10+ uses
pub fn color_very_dim(c: Color32) -> Color32 { c.gamma_multiply(0.3) } // 8+ uses
```

### 5.4 Strokes

| Proposed | Value |
|---|---|
| `stroke_medium()` | 0.8 (17 uses) |
| `stroke_extra_thick()` | 2.5 (5 uses) |
| `stroke_extra_heavy()` | 3.0 (sparse but recurring) |

### 5.5 Layout / dimensions

| Proposed | Value | Justification |
|---|---|---|
| `button_height_sm()` | 18.0 | hardcoded in pane.rs option chain rows |
| `button_height_md()` | 22.0 | hardcoded in stepper, action rows |
| `label_height()` | 16.0 | hardcoded in pane.rs:1059–63 |
| `badge_padding_x()` / `_y()` | 4–5 / 2–3 | repeated `vec2(8,4)` / `vec2(10,6)` patterns |
| `radius_pill()` | already exists in enum, no value defined | derive from `Size::height() / 2` or set 99px |

### 5.6 Theme palette additions

| Proposed | Why |
|---|---|
| `t.info` | 24 uses of `(74, 158, 255)` for informational tone — currently no token |
| `t.success` | 5+ uses where `bull` is misused as "success" semantically |
| `t.error` | distinct from `bear` (price-down) — alerts/rejections need their own slot |
| `t.surface_subtle` | DOM panel needs a "header strip" color distinct from `bg` and `toolbar_bg` |

### 5.7 Shadow colors

Replace 6+ hardcoded `Color32::from_rgba_unmultiplied(0, 0, 0, X)` with a `t.shadow_color`-based helper:

```rust
pub fn shadow_color_alpha(t: &Theme, alpha: u8) -> Color32 {
    let c = t.shadow_color;
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}
```

---

## 6. Missing components & variants

### 6.1 Missing `Button` variants (8–12)

Each justified by 5+ call sites that apply manual `.fg()` / `.glyph_color()` overrides:

| Proposed | Replaces pattern | Sites |
|---|---|---|
| `Button::menu()` | `ui.menu_button(RichText::new(label).monospace().size(font_sm()).color(t.dim), ...)` | 33 sites |
| `Button::icon_muted()` | `Button::icon(i).variant(Ghost).glyph_color(t.dim.gamma_multiply(0.5))` | 6+ sites |
| `Button::danger_ghost()` | `Button::icon(X).variant(Ghost).glyph_color(t.bear)` | 3+ sites |
| `Button::status_icon(active)` | Ghost icon that's muted when idle, full when active | object_tree visibility/lock toggles |
| `Button::category_tinted(color)` | `Button::new(label).fg(category_color)` | filter_pill, heat_panel, plays_panel |
| `Button::secondary_muted()` | `Button::new(label).variant(Secondary).simple_treatment(true).fg(t.dim)` | 10+ sites |

> **`Variant::Chrome` is used 40+ times as an escape hatch** — that's the smell: the palette of variants is incomplete. Adding the variants above should drop Chrome usage to ~5.

### 6.2 New widgets to extract

| Widget | Replaces | Justification |
|---|---|---|
| `StatBox` / `StatusLabel` | hand-rolled `painter.rect_filled` + `painter.text` | 24+ sites |
| `MetricRow` | `insight_stat_bar()` and similar label/value/bar compositions | 5+ sites |
| `Section` (unified) | `PanelSection` + `SectionHeader` + free-fn `section_header()` | 3 implementations of same concept |
| `EmptyState` (in ui_kit) | `PanelEmpty` is in panels/kit.rs only | promote for cross-panel use |
| `IconBadge` | `Icon` + `Badge` composition | recurring (e.g. tree-button count badge — already painted manually in top_nav) |
| `LabeledSwitch` | free fn `toggle_switch()` | wrap as struct |
| `ConfirmDialog` | `Modal::confirm()` constructor | 4+ panels paint their own |
| `InsetShadowedDivider` | manual gradient-line loops in dom_panel | recurring "header→body" treatment |

---

## 7. Theme threading — `THEMES[0]` antipattern

**56 fallback sites** ignore the active theme and use `THEMES[0]` (Midnight). This means:

- **Switching themes silently leaves widgets in Midnight** if they were added with `ui.add(widget)` instead of `widget.show(ui, theme)`.
- **Light themes look broken** because dark-blue accents bleed through.

### 7.1 Distribution

| Location | Count | Notes |
|---|---:|---|
| `ui_kit/widgets/*.rs` (Widget impl fallback) | 24 | Pattern: `let theme = &crate::chart_renderer::gpu::THEMES[0];` inside `Widget::ui` |
| `chart/renderer/ui/components/*.rs` | 8 | Free `fn ft()` returning `&'static Theme` |
| `chart/renderer/ui/inputs/*.rs` | 5 | Same `fn ft()` pattern |
| `chart/renderer/ui/lists/cards/*.rs` | 8 | Same |
| `chart/renderer/ui/lists/rows/*.rs` | 7 | Same |
| `chart/renderer/ui/chrome/floating_pane.rs` etc. | 4 | Same |

### 7.2 Fix patterns

**A. Widget impl fallback** (e.g. `button.rs:240`):

```rust
// Current — silently uses THEMES[0]
impl Widget for Button<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let theme = &crate::chart_renderer::gpu::THEMES[0];
        self.show(ui, theme)
    }
}
```

→ Read the active theme from `ui.ctx().data()` (store the active theme idx as `ui::Memory` data on app start, or as a `ctx.options().theme` extension), or remove the `Widget` impl and force callers to use `.show(ui, theme)`.

**B. `fn ft()` helpers**: rip them out. Every function that uses `ft()` should accept `theme: &dyn ComponentTheme` (or `&Theme`) as a parameter.

---

## 8. Light-theme compatibility (critical)

15 themes ship; 4 are light (Bauhaus, Peach, Ivory, Newsprint). They will **all** look wrong in places due to:

### 8.1 Hardcoded black shadows

Search: `Color32::from_rgba_unmultiplied(0, 0, 0,`

| Site | Effect on light theme |
|---|---|
| `ui/panels/plays_panel.rs` (card drop shadow) | Black halo around cards on cream background |
| `ui/style.rs:888` (drop shadow utility) | Same — affects every consumer |
| `ui/panels/dom_panel.rs` (inset gradient — *added recently for header/button-area treatment*) | Dark band on light background |
| `ui_kit/widgets/shadow.rs:64` (raw shadow) | Hardcoded `(0,0,0,64)` |
| `chart/renderer/render/pane.rs` (2 sites: pane-edge stroke, pane fill) | Dark borders on light themes |
| `chart/renderer/ui/tools/order_edit_dialog.rs` (dialog shadow) | Dark halo |

`Theme.shadow_color` already exists per theme — light themes use `(40,40,40)` (dark gray, not pure black). Plumb it through, don't hardcode.

### 8.2 Hardcoded white tints (the inverse)

A few sites use `Color32::from_rgba_unmultiplied(255, 255, 255, X)` for highlight overlays. On light themes those vanish. Use `t.element_hover` / `t.text` derived overlays instead.

---

## 9. Documentation gaps

### 9.1 What exists

`docs/` already has a lot of design-system content:
- `DESIGN_SYSTEM.md` — strong token reference
- `COMPONENT_LIBRARY_PLAN.md` — porting strategy from gpui-component
- `design-system-component-audit.md`, `design-system-coverage.md`, `design-system-usage-map.md`
- `meridien-recovery-audit.md`, `r4-ui-unification-plan.md`, `r5-token-promotion-summary.md`
- `remaining-hardcoded-report.md`

### 9.2 What's missing

| Doc | Why it matters |
|---|---|
| **Project root `CLAUDE.md`** | No agent guidance file — agents repeatedly improvise hardcoded values because there's no canonical reference |
| **`src-tauri/CLAUDE.md`** | Same; should encode the policies in §7, §8, §10 |
| **In-code module docs**: `gpu.rs` has zero `//!` doc | This is the most critical file in the renderer — it should describe Theme, THEMES, theme threading |
| **Theme field doc comments** | Only `LEGACY:` markers exist; new fields need intent docs |
| **"How to add a component" guide** | COMPONENT_LIBRARY_PLAN has it embedded but not as a standalone walk-through |
| **Light-theme test policy** | Hardcoded-black-shadow bug would have been caught with one |
| **Token cheatsheet** | A 1-page TL;DR mapping intent → token (e.g. "secondary text → `t.dim`, dimmed body → `color_alpha(t.text, alpha_strong())`") |

### 9.3 Existing `style.rs` module doc is close to the cheatsheet

The header doc in `style.rs` lists every category. Promote it to a top-level `docs/UI_TOKENS.md` and add intent → token mappings.

---

## 10. Inconsistencies — same intent, different code

| Intent | Implementations seen | Recommendation |
|---|---|---|
| Section label | `font_2xs()` (legacy alias), `font_xs()`, `font_sm()`, `font_sm_tight()` | Pick one: `font_xs()` for SECTION/CAPS labels |
| Panel header | `font_md()` (panel_header), `font_lg()` (dialog_header) | Document the distinction (panel vs dialog) |
| Dimmed text | `t.dim` *or* `t.dim.gamma_multiply(0.6)` *or* `color_alpha(t.dim, alpha_dim())` | Add `color_muted(t.dim)` helper, use everywhere |
| Border | `stroke_thin()` *or* literal `0.5` *or* `0.8` | Replace literals; `stroke_thin` for default borders |
| Button height | `Size::Md.height()=28`, `Size::Sm.height()=22`, hardcoded 18/20/24 | Always use `Size::*.height()` |
| Toggle on/off color | `.fg(col)` ad-hoc, `.semantic_state()` doesn't exist | Add `Button::semantic_state(active)` |
| Card padding | `gap_lg()` *or* `vec2(10,6)` *or* custom | Use design_tokens::CardTokens (already exists, mostly ignored) |

---

## 11. Pattern duplication — what gets reimplemented

### 11.1 Hand-rolled `egui::Button` (83 sites)

Mostly inside `chart/renderer/ui/components/` files — they wrap egui::Button with custom `.fill()` / `.stroke()` / `.corner_radius()`. These should *all* be `ui_kit::Button` calls.

Representative: `components/chips.rs` × 6, `components/header_buttons.rs` × 2, `components/headers.rs` × 3, `components/menus.rs` × 3, `components/pills.rs` × 4, `components/pills_widget.rs` × 5, plus 37 across panels.

### 11.2 Hand-rolled `painter.rect_filled + painter.text` (24 sites)

Mostly status indicators and DOM action buttons. Extract `StatBox { rect, fill, label, fg, font_size }`.

### 11.3 Hand-rolled `ui.menu_button(...)` (33 sites)

Mostly toolbar dropdowns and tool selectors. Add `Button::menu(label, |ui| { ... })`.

### 11.4 `.fg()` / `.glyph_color()` overrides (50+)

Each is evidence of a missing variant. See [§6.1](#61-missing-button-variants-812).

---

## 12. Optimizations

### 12.1 `Theme` struct bloat (85 fields)

60+ fields are flagged `LEGACY` in source comments. Each pulls bytes through cache lines on every render. Audit and remove:
- Fields synthesizable via `color_alpha(t.dim, ALPHA_X)` — most legacy fields fall here.
- RRG colors — only used in one widget; consider feature-gating to a sub-struct.

### 12.2 `THEMES` is `&[Theme]` — a 15-element array of 85-field structs

If most fields are legacy, slimming `Theme` could halve cache pressure for theme reads. Bench before changing — cold rendering may not care.

### 12.3 Per-frame text layout

Many `painter.text(...)` calls re-layout the same string each frame. `painter.layout_no_wrap(...)` (already used in some sites) caches layout. Extracting `StatBox` is a chance to use `Galley` caching everywhere.

### 12.4 `gamma_multiply` arithmetic

Every `gamma_multiply(0.6)` does a per-channel float multiply each call. The proposed `color_muted/dim/subtle` helpers (§5.3) can memoize once per theme switch, not per draw call.

---

## 13. Prioritized punch list

### Tier 1 — Foundational fixes (do first)

1. **Kill `THEMES[0]` fallback in widget `Widget` impls.** Either thread `ctx.theme()` or remove the impl. Forces callers to `.show(ui, theme)`. (24 sites in `ui_kit/widgets/*.rs`)
2. **Replace all `fn ft() -> &Theme` helpers** with explicit `theme: &Theme` params. (32 sites)
3. **Replace hardcoded `Color32::from_rgba_unmultiplied(0, 0, 0, X)` shadows** with a `shadow_color_alpha(t, X)` helper. (6+ sites — fixes 4 light themes)
4. **Reconcile token disagreements**:
   - `RADIUS_*` consts vs `radius_*()` fns return different values
   - `alpha_muted == alpha_dim` and `alpha_line == alpha_strong` — disambiguate
   - `gap_2xs == gap_xs` — make `gap_2xs` truly smaller (2.0)
5. **Add `font_2xs() = 8.0`, `font_3xs() = 7.0`, `font_xs_plus() = 10.0`, `font_xl() = 14.0`** as real tokens; refactor 100+ font-size hardcodes.
6. **Add `color_subtle/muted/dim/very_dim` helpers**; refactor 30+ `gamma_multiply` chains.
7. **Add `Button::menu(label, body)` variant**; replace 33 raw `ui.menu_button` sites.

### Tier 2 — Component consolidation

8. **Extract `StatBox` widget**; replace 24+ `painter.rect_filled + painter.text` ad-hoc renders.
9. **Add 6 missing `Button` variants** (`menu`, `icon_muted`, `danger_ghost`, `status_icon`, `category_tinted`, `secondary_muted`); drop `Variant::Chrome` use from 40+ sites to ~5.
10. **Unify `PanelSection` / `SectionHeader` / `section_header()`** into one widget.
11. **Promote `EmptyState` from `panels/kit.rs` into `ui_kit/widgets/`**.
12. **Replace 83 `egui::Button` direct calls** with `ui_kit::Button` (most are in `components/chips.rs`, `components/pills.rs`, `components/action_button.rs`).
13. **Delete one of the two `Size` enums** (`ui_kit/widgets/tokens.rs::Size` vs `chart/renderer/ui/foundation/tokens.rs::Size`).

### Tier 3 — Documentation

14. **Create `src-tauri/CLAUDE.md`** with the agent-facing UI policy: theme threading, no `THEMES[0]`, no hardcoded shadows, prefer `ui_kit::Button` over `egui::Button`, what to grep for before adding a new widget.
15. **Promote `style.rs` header doc** into `docs/UI_TOKENS.md` cheatsheet with intent → token mapping.
16. **Add field doc comments to `Theme` struct**; mark legacy fields explicitly with deprecation pointers.
17. **Add module-level doc to `gpu.rs`** describing Theme architecture and threading rules.
18. **Update `docs/COMPONENT_LIBRARY_PLAN.md`** with the missing variants from §6.1.

### Tier 4 — Hardening

19. **Light-theme visual regression test** (one screenshot per panel × Bauhaus/Midnight, diff for accessibility).
20. **Lint rule / CI grep** for: `Color32::from_rgba_unmultiplied(0, 0, 0,`, `&THEMES[0]`, `FontId::monospace(\d+\.\d)` outside style.rs, `gamma_multiply\(0\.\d+\)` (suggest `color_muted` etc.).
21. **Audit `Theme` struct** for legacy field removal (60+ candidates).
22. **Centralize repeated RGB triples** (amber, cyan info, purple) into theme slots or named constants.

---

## Appendix A — Quick file map

| Question | File |
|---|---|
| Where are typography tokens defined? | `src/chart/renderer/ui/style.rs:49–85` |
| Where are spacing tokens? | `style.rs:100–115` |
| Where are alpha tokens? | `style.rs:144–169` |
| Where is `Theme`? | `src/chart/renderer/gpu.rs:104` |
| Where is `THEMES`? | `gpu.rs:320` |
| Where is `ComponentTheme`? | `src/ui_kit/widgets/theme.rs` |
| Where is `Button`? | `src/ui_kit/widgets/button.rs` |
| Where is `Size` (canonical)? | `src/ui_kit/widgets/tokens.rs:18` |
| Where is `StyleSettings`? | `style.rs:1162+` |
| Where are themes selected? | `pane.rs` reads `chart.theme_idx`; `gpu.rs::get_theme(idx)` |

---

## 14. Deeper-pass findings (added after first execution review)

### 14.1 Iconography

**Phosphor coverage gaps — 50+ Unicode glyphs in `chart/renderer/mod.rs:148–199` chart-widget enum** that should be Phosphor:

| Current | Phosphor equivalent | Sites |
|---|---|---|
| 📅 (`\u{1F4C5}`) | `Icon::CALENDAR_BLANK` | mod.rs:162 |
| 📰 (`\u{1F4F0}`) | `Icon::NEWSPAPER` | mod.rs:163 |
| ⚡ (`\u{26A1}`) | `Icon::LIGHTNING` (already exists!) | mod.rs:165, 414 |
| 📊 (`\u{1F4CA}`) | `Icon::CHART_BAR` | mod.rs:191 |
| 📈 (`\u{1F4C8}`) | `Icon::CHART_LINE` | mod.rs:194 |
| 💰 (`\u{1F4B0}`) | `Icon::CURRENCY_DOLLAR` | mod.rs:197 |
| ✕ (`\u{2715}`) | `Icon::X` | `ui_kit/widgets/input.rs:268` |
| × (`\u{00D7}`) | `Icon::X` | `panels/plays_panel.rs:327, 354` |
| `"+"` / `"-"` | `Icon::PLUS` / `Icon::MINUS` | `panels/spread_panel.rs:374–376` |
| `● / ○` | `Icon::CIRCLE_FILL` / `Icon::CIRCLE` | `top_nav.rs:355` (IBKR connection state) |

Block/shading characters (█ ▓ ▒ ░) used in volume profile/heatmaps are **intentional** — Phosphor has no equivalent and they paint as text-mode bars; leave them alone.

**Icon-size inconsistencies** for the same icon:
- `Icon::X` rendered at 8px (pane:5203), 9px (pane:6676), 24px (button defaults) — pick a tier per context
- `Icon::CIRCLE_FILL` at `font_xs()` (≈9px), 16px (menus), 24px (buttons), 32px (empty states)
- `Icon::CHART_LINE` at 11px (chart annotation) and 16px (menu trigger)

**Critical hardcoded color**: `render/pane.rs:6677` uses `Color32::WHITE` on icon hover — breaks 4 light themes.

**One stray `KitButton::icon(...)` falls back to `&THEMES[0]`** at `chrome/pane.rs:1011` — adds to the 56 already documented.

### 14.2 Motion / animation

**Strong points** — `ui_kit/widgets/motion.rs` is well-designed:
- `FAST=0.12`, `MED=0.18`, `SLOW=0.28` duration tokens used consistently
- `ease_in_out_cubic` applied via `ease_bool` / `ease_value` helpers
- `lerp_color` is RGBA-premultiplied-aware (correct)
- Animation tracking via thread-local `ANIM_STATE` for frame-profiler idle detection

**Gaps**:
- **No `prefers-reduced-motion` honoring.** No way to opt out of animations. Add a `style_settings.animations_enabled` flag and gate `ease_bool/ease_value` on it.
- **Hardcoded durations not in the token set**:
  - Cursor blink smoothing: `0.06` at `motion.rs:40`
  - Tooltip delay: `0.4` (400ms) at `tooltip.rs:32`
  - Hover-card delay: `0.6` (600ms) at `hover_card.rs:19`
  - Scroll ease: `0.20` at `motion.rs:58`
  Add `DELAY_TOOLTIP`, `DELAY_HOVER_CARD`, `INSTANT` (≤30ms) tokens.

### 14.3 Focus & keyboard accessibility

| Issue | Location | Impact |
|---|---|---|
| `focus_ring()` defined but **not universally applied** to interactive widgets | `style.rs:240` | Keyboard users have no focus indicator on most buttons |
| **No app-wide tab-order management** | n/a | Tab navigation is per-widget; can escape modals |
| **No focus trap on modals** | `modal.rs`, `sheet.rs` | Tab keypress in modal can move focus to background UI |
| **Keyboard shortcuts scattered** — no central registry | 40+ files using `ctx.input(|i| i.key_pressed)` | Conflict detection impossible; no help/discovery system |
| `Kbd` widget exists but **under-used** — only 3 files | `kbd.rs` | Many buttons use `.on_hover_text(...)` for shortcuts instead of inline keycaps |
| **Zero ARIA / screen-reader support** | n/a | Tooltips are mouse-only; no live-region announcements for fills/alerts |

### 14.4 Density / compact mode — **dead code**

The `Density` enum (`ui_kit/widgets/tokens.rs:54–66`) defines `Compact / Default / Comfortable` with `vscale()` returning `0.65 / 1.0 / 1.4`. **Zero call sites** invoke `.vscale()` — the enum is unused.

`compact_mode: bool` (gpu.rs:274) is checked in **exactly one place** — `render/pane.rs:1416` — to set chart top padding to `1.0` instead of `4.0`. It does not affect:
- Row heights (`Size::Sm.height()` is hardcoded `22.0`)
- Padding (`gap_*()` tokens are static)
- Font sizes
- Any panel rendering

**Decision needed**: either wire density through (Size::height() multiplied by `density.vscale()`, all `gap_*()` honor density) or delete the enum and the `compact_mode` flag. The current state is a half-built feature.

### 14.5 Layering / z-order

| Concern | Detail |
|---|---|
| Modal stack has 2 patterns | `Sheet` uses `Background` scrim + `Foreground` panel (correct). `Modal` uses `Middle` shadow + implicit `Window` order (works but inconsistent) |
| **No viewport-aware menu repositioning** | `context_menu.rs:110–130` positions below anchor with `gap_xs()` offset; clips at screen edge instead of flipping above |
| **Multiple `Foreground` items rely on draw order** | tooltips, dropdowns, context menus, hover cards, popovers, command palette all share `Order::Foreground` — last drawn wins |
| Toasts use raw `egui::Window` with implicit order | `top_nav.rs:2025–2045` — could be obscured by modals |

### 14.6 Selection / interaction feedback

| Concern | Detail |
|---|---|
| **Drawing multi-select uses hardcoded `Color32::WHITE`** | `render/pane.rs:3930+` — should use `t.accent` |
| Drag handles inconsistent | Drawing handles: white dots. Resize handles: implicit (no custom paint, only `Sense::drag`). Need shared `DragHandle` widget. |
| `SelectableRow` widget good, but **not used uniformly** | Watchlist uses it; orders panel and others paint their own row backgrounds |
| Pressed / hovered / active states | All three are distinct (snap / FAST / MED) — this part is good |
| Status-mode buttons snap on hover (no fade) | `button.rs:374–378` — `is_status` flag — intentional, kept |

### 14.7 Updated punch-list additions

Add to **Tier 1**:
- Replace 50+ Unicode glyphs in `chart/renderer/mod.rs` with Phosphor `Icon::*` constants
- Fix `Color32::WHITE` hardcoded hover at `render/pane.rs:6677`
- Fix `&THEMES[0]` at `chrome/pane.rs:1011`
- Replace drawing-selection hardcoded `Color32::WHITE` with `t.accent`

Add to **Tier 2**:
- Decide density: wire through `Size::height()` and `gap_*()` or delete the enum + `compact_mode` flag
- Apply `focus_ring()` consistently to all interactive widgets via `apply_interaction()`
- Build a central keyboard-shortcut registry; surface conflicts; auto-generate the help screen
- Add `DELAY_TOOLTIP / DELAY_HOVER_CARD / INSTANT` motion tokens; replace 4 hardcoded durations
- Add `prefers-reduced-motion` opt-out (`style_settings.animations_enabled`)

Add to **Tier 3**:
- Document the 50 chart-widget icon mappings as part of `docs/UI_TOKENS.md`
- Add accessibility section to `src-tauri/CLAUDE.md` (focus rings, keyboard, future a11y plans)

---

---

## 15. Second-pass audit (2026-05-10)

The first audit caught the obvious (THEMES[0], hardcoded shadows, font/spacing tokens). This deeper pass quantifies what wasn't yet visible: **how broadly the existing components are used vs. how often the same widgets get hand-rolled**.

### 15.1 Hand-rolled clickables — 75–85 sites

These build clickable widgets with `ui.allocate_rect(rect, Sense::click()) + painter.rect_filled + painter.text + response.clicked()` instead of using `Button`. They blow past keyboard accessibility, focus rings, hover animation, motion easing — every cross-cutting concern the design system handles.

| Pattern | Count | Examples |
|---|---:|---|
| `ui.allocate_rect(_, Sense::click())` + manual paint | ~30 | `dom_panel.rs:294,309,140,345`, `painter_pane.rs:650,750,776,822`, `pane.rs:10962,11062`, replay controls, alert/order row close buttons, `dom_action.rs:30,72,134,234` |
| `ui.button("...")` (egui's basic button) | 13 | `pane.rs:10194,10199,10203,10210,10221,10232,10244,10281,10342,10362,10371,10377,10384,10396,10401` (the right-click context menu builds **every entry as a raw `ui.button`**), `watchlist_panel.rs:112,116,122,138,142,148,566,588,597,1094,1116,1125`, `order_ledger_panel.rs:326` |
| `ui.add(egui::Button::new(...))` | 30 | `form.rs:1202,1275,1332` (EXT/MKT/Bracket buttons), `inputs/form.rs` (toolbar form buttons), `signals_panel.rs:43,83,91`, `drawing/properties_bar.rs:110,235,243,255,277` |
| `ui.selectable_label(...)` | 9 | `inputs/select.rs:95,195,433,723,789` (dropdown items), `object_tree.rs:257`, `drawing/properties_bar.rs:206,216,226` |
| `ui.put(rect, egui::Button::new(...))` | 1 | `style.rs:512` |

**Highest-leverage**: the right-click context menu in `render/pane.rs:10194–10711` is built as ~25 raw `ui.button()` calls. One pass converting that menu to `Button::new` + variants would single-handedly knock out 13 of the 13 `ui.button()` violations.

### 15.2 Native egui widget violations — 300+ sites in 83 files

Direct uses of `egui::TextEdit`, `egui::ComboBox`, `egui::DragValue`, `ui.checkbox`, `egui::Slider`, `egui::Frame`, `egui::Window`, `egui::Area` — bypassing the ui_kit equivalents.

| Native widget | Sites | Replacement | Top 3 offending files |
|---|---:|---|---|
| `egui::TextEdit` / `text_edit_*` | ~40 | `ui_kit::Input` (+ new `TextArea` for multiline) | `inputs/inputs.rs` (8), `inputs/form.rs` (7), `pane.rs` (6) |
| `egui::ComboBox` | ~16 | `ui_kit::Select` | `top_nav.rs` (3), `design_inspector.rs` (3), `inputs/select.rs` (6) |
| `ui.checkbox` | ~10 | `ui_kit::Checkbox` | `design_inspector.rs` (6) |
| `egui::Slider` | 3 | `ui_kit::Slider` | `design_inspector.rs` (2) |
| `egui::DragValue` | ~30 | **MISSING — need `Stepper` widget** | `indicator_editor.rs` (10), `settings_panel.rs` (9), `design_inspector.rs` (5) |
| `egui::Frame::*` | ~70 | `Card` / `Panel` / `Popover` | `frames_widget.rs` (31), `design_inspector.rs` (8), `frames.rs` (6) |
| `egui::Window::*` | ~17 | `ui_kit::Modal` / `Sheet` | `pane.rs` (4), `design_inspector.rs` (3) |
| `egui::Area::*` | ~17 | `Popover` / `Tooltip` / `ContextMenu` | (mostly in widget impls) |
| `ui.collapsing` | 0 found | `ui_kit::Disclosure` (currently underused) | n/a |

### 15.3 Button variant utilization — Chrome is the escape hatch

Histogram of `Button` variant usage (159 total Button calls):

| Variant | Count | % | Notes |
|---|---:|---:|---|
| **Chrome** | **75** | **59%** | 🚨 escape hatch — biggest signal of missing variants |
| Secondary | 31 | 24% | mostly appropriate |
| Ghost | 28 | 22% | good adoption |
| Primary | 5 | 4% | low — buy/sell constructors absorb most |
| Link | 2 | 2% | barely used |
| Danger | **0** | **0%** | 🚨 **dead code** despite 12+ destructive sites that need it |

Modifier histogram (the heavy hitters):

| Modifier | Count | Diagnosis |
|---|---:|---|
| `.fill(...)` | **224** | 🚨 by far the most-used escape hatch — variant fills aren't matching what callers want |
| `.corner_radius(...)` | **164** | shape system isn't covering needs |
| `.min_size(...)` | **159** | size system isn't covering needs |
| `.fg(...)` | 72 | text-color overrides — missing semantic-color variants |
| `.glyph_color(...)` | 48 | icon-color overrides — same |
| `.frameless(true)` | **37** | 🚨 missing `TextOnly` / `Link` variant |
| `.simple_treatment(true)` | 20 | known pattern |
| `.status(true)` | 21 | well-adopted for status icons |
| `.tint(...)` | 4 | barely used (buy/sell absorbed it) |
| `.hover_fill(...)` | 2 | almost dead |

### 15.4 Missing `Button` variants — 10 identified

Justified by 2+ call sites with stacked overrides:

| Proposed | Replaces stacked overrides | Sites | Files |
|---|---|---:|---|
| **`Variant::Chip`** | `Chrome + .fg(if sel { accent } else { dim*0.5 }) + .fill(if sel { accent_soft } else { TRANSPARENT }) + .corner_radius(r_xs) + .min_size((22,18))` | 20+ | `inputs/form.rs:942`, `inputs/inputs.rs:820`, `inputs/filter_pill.rs:60`, `inputs/nmf_toggle.rs:48`, `playbook_card.rs:58` |
| **`Variant::Tab`** | `Chrome + TRANSPARENT fill + frameless + fg(if sel { accent } else { dim*0.5 }) + min_size((0,22))` | 7+ | `analysis_panel.rs:90`, `chrome/pane.rs:125–129` |
| **`Variant::InlineClose`** | `Ghost + .glyph_color(dim*0.7) + min_size(splat(18)) + frameless(true)` for `Icon::X` | 5+ | `chrome/floating_pane.rs:182`, `chrome/pane.rs:1016`, `top_nav.rs:813`, `analysis_panel.rs:102`, `discord_panel.rs:193` |
| **`Variant::MutedIcon`** | `Ghost + .glyph_color(dim*0.5)` | 3+ | `top_nav.rs:810,813,1104` |
| **`Variant::NeutralAction`** | `Secondary + .fill(gray170) + .fg(BLACK)` for utility actions like FLATTEN | 3+ | `dom_panel.rs:405`, `chrome/pane.rs:834` |
| **`Variant::ToolbarTab`** | `Secondary + corner_radius(9.0) + active(...)` | 3+ | `chrome/pane.rs:125–130` |
| **`Variant::Destructive`** (or fix `Danger` to actually work) | `Secondary + .fill(order_cancel_bg) + .fg(order_cancel_fg)` | 2+ | `dom_panel.rs:421` (CANCEL), `chrome/pane.rs:824` |
| **`Variant::TextOnly`** | `Chrome + TRANSPARENT + frameless(true) + fg(c)` | 37+ | spread across the codebase |
| `Variant::Modal` (secondary muted) | `Secondary + simple_treatment + fg(dim)` | 2+ | `command_palette/render.rs:27`, `connection_panel.rs:40` |
| Segmented Control (3-variant: Left/Mid/Right) | per-segment corner-radius asymmetry + connected stroke | 2+ | `pills_widget.rs:112,122` |

**Result if all added**: `Variant::Chrome` usage drops from 75 → ~5; `.fill()`/`.corner_radius()`/`.min_size()` overrides drop ~70%.

### 15.5 Pill / chip / badge proliferation — 11 distinct implementations

For one visual concept ("small bordered/filled label/toggle"), the codebase has:

| Implementation | File | Form |
|---|---|---|
| `pill_button()` | `components/pills.rs` | free fn — **canonical** |
| `status_pill()` | `components/pills.rs` | free fn — legacy |
| `status_badge()` | `components/pills.rs` | free fn — legacy |
| `pill_btn()` | `components/pills.rs` | free fn — **DEPRECATED** |
| `notification_badge()` | `components/pills.rs` + `components/chips.rs` | free fn — **duplicated in two files** |
| `filter_chip()` | `components/chips.rs` | free fn — **DEPRECATED** |
| `display_chip()` | `components/chips.rs` | free fn |
| `removable_chip()` | `components/chips.rs` | free fn |
| `keybind_chip()` | `components/chips.rs` | free fn |
| `RemovableChip` | `components/pills_widget.rs` | builder — duplicates `removable_chip()` |
| `DisplayChip` | `components/pills_widget.rs` | builder — duplicates `display_chip()` |
| `FilterPill` | `inputs/filter_pill.rs` | widget |
| `Tag` | `ui_kit/widgets/tag.rs` | widget |
| `Badge` | `ui_kit/widgets/badge.rs` | widget |
| `ChipShell` | `foundation/shell.rs` | new builder (v4.5b) — **intended successor** |

**Resolution**: see Tier 1 punch list — delete deprecated, consolidate duplicates, migrate the rest to `ChipShell` once `ChipVariant` enum has Status/Keybind/Display/Removable variants.

### 15.6 List-row proliferation — 6 implementations, no shared frame

| Row impl | LOC | Pattern |
|---|---:|---|
| `SelectableRow` (ui_kit) | 168 | only used in dropdown menus |
| `RowShell` (foundation) | 150+ | only fully used by `OrderRow` |
| `WatchlistRow` | **792** | direct painter, no RowShell |
| `DomRow` | **677** | direct painter, no RowShell |
| `OptionChainRow` | 145 | direct painter |
| `AlertRow` | 152 | direct painter |
| `NewsRow` | 146 | half-uses CardShell |
| `OrderRow` | 146 | uses RowShell painter-mode ✅ |

The two giants — `WatchlistRow` (792) and `DomRow` (677) — duplicate ~80% of layout/interaction logic that `RowShell` already encapsulates. Migrating them is a **multi-day effort** but worth ~1500 LOC reduction.

### 15.7 Per-panel hand-roll hotspots

Top "should be a widget" patterns by panel:

| Panel | LOC | Worst pattern | Lines |
|---|---:|---|---|
| `dom_panel.rs` | 580 | Quantity stepper (custom rect+painter for −/qty/+); arm toggle; column-header layout | 283–317, 341–375, 126–227 |
| `plays_panel.rs` | 840 | R:R bar (painter calls); tag chip selector; pct stepper | 399–406, 414–452, 551–574 |
| `script_panel.rs` | 650 | Backtest stats grid (33 lines × 5 cards); trade table (95 lines); 3× hand-rolled dividers | 465–498, 510–604, 613/301/392 |
| `journal_panel.rs` | 266 | Trade card (56 lines); insight rows (16 lines × N); stat grid | 209–265, 191–207, 114–145 |
| `discord_panel.rs` | 300+ | **Guild avatar grid** (72 lines manual circle+initials+selection halo); `Color32::BLACK` hardcoded | 225–297, 223 |
| `heat_panel.rs` | 271 | Heatmap grid cells (73 lines); sector collapse toggle | 147–220, 248 |
| `orders_panel.rs` | 424 | Position cards (71 lines, 3 instances) | 80–151 |
| `settings_panel.rs` | 300+ | Font family cards (52 lines); style preset grid (15 lines per chunk) | 176–228, 119–134 |
| `object_tree.rs` | 250+ | Opacity picker (33 lines manual segments); **STILL has `fn ft()` THEMES[0] helper** | 75–108, hidden |
| `analysis_panel.rs` | 167 | Tab bar (manual button loop instead of Tabs widget) | 90–98 |

### 15.8 Components that should exist but don't — 12 to extract

By call-site count:

| New widget | Replaces | Sites |
|---|---|---:|
| `Stepper` (DragValue replacement) | `egui::DragValue` everywhere | 30+ |
| `MetricRow` (label/value pair with optional bar) | inline `ui.horizontal { label … value }` | 50+ |
| `StatBox` / `StatusLabel` | `painter.rect_filled + painter.text` for inline status | 24+ |
| `OpacityPicker` | manual segment rendering | 2 (object_tree, heat_panel) |
| `RiskRewardBar` | `painter.rect_filled` + `painter.circle_filled` | 1 (plays_panel — but compelling) |
| `HeatmapGrid` | 73-line cell loop | 1 (heat_panel) |
| `TradeCard` | 56-line painter card | 1 (journal_panel) |
| `GuildAvatarGrid` | 72-line discord guild selector | 1 (discord_panel) |
| `SegmentedControl` (or Tab + Chip variants on Button) | timeframe selector, tab strip, filter pills, NMF toggle | 4+ |
| `EmptyState` (promoted to ui_kit) | `empty_state_panel()` + `PanelEmpty` duplication | 2 implementations + N inline |
| `ConfirmDialog` (`Modal::confirm` constructor) | inline modal builders | 4+ panels |
| `LabeledSwitch` | `toggle_switch()` free fn | promote |

### 15.9 Updated punch list — Tier 1 (do next)

1. **Add `Variant::Chip`, `Variant::Tab`, `Variant::InlineClose`, `Variant::NeutralAction`, `Variant::TextOnly`** — paint logic in `paint_button()`. Justified by 70+ stacked-override call sites.
2. **Delete `Variant::Danger`** if not adopted, or **make it real** by giving it the `bear`-fill paint (currently 0 callers).
3. **Add `Stepper` widget** — replaces `egui::DragValue` in 30+ sites; biggest single win.
4. **Add `MetricRow` widget** — refactors `ui.horizontal { label … value }` patterns in 50+ sites.
5. **Delete deprecated free fns**: `filter_chip()`, `pill_btn()`. Consolidate `notification_badge()` (duplicated across `chips.rs` and `pills.rs`).
6. **Convert `render/pane.rs:10194–10711` right-click menu** — ~25 raw `ui.button()` calls in one block. Single PR opportunity.
7. **Fix `panels/object_tree.rs` `fn ft() -> THEMES[0]`** — last known on-purpose THEMES[0] in the panels.
8. **Fix `panels/discord_panel.rs:223` `Color32::BLACK`** — light-theme bug.

### 15.10 Updated punch list — Tier 2

9. **Migrate the right-click menu and other context menus** to `Button::menu()`/`ContextMenu`.
10. **Migrate `egui::DragValue` → `Stepper`** in `indicator_editor.rs` (10), `settings_panel.rs` (9), `design_inspector.rs` (5), `top_nav.rs` (3), `form.rs` (2), `scanner_panel.rs` (2).
11. **Migrate `egui::TextEdit` → `Input`** in `pane.rs` (6), `inputs/inputs.rs` (8), `inputs/form.rs` (7), `settings_panel.rs` (2). Add `TextArea` widget for multiline.
12. **Migrate `egui::Frame` proliferation in `frames_widget.rs`** (31 inline Frame builds) to `Card`/`Panel`/`Popover` builders.
13. **Migrate `egui::Window` → `Modal`/`Sheet`** in `order_entry_panel.rs`, `order_edit_dialog.rs`, `pending_order_toasts.rs`, `trendline_filter.rs`, `overlay_manager.rs`.
14. **Extract `Stepper`-shaped widgets in panels**: dom_panel quantity stepper (lines 283–317), plays_panel pct stepper (551–574), spread_panel +/− buttons.
15. **Extract `OpacityPicker`** (object_tree.rs:75–108).
16. **Extract `TradeCard`/`InsightRow`/`StatGrid`** for `journal_panel.rs` (currently 56+16×N+31 lines of inline rendering).
17. **Migrate `WatchlistRow` (792 LOC) and `DomRow` (677 LOC) onto `RowShell`** — biggest LOC-reduction opportunity in the codebase.
18. **Replace `analysis_panel.rs:90–98` manual tab loop** with `ui_kit::Tabs`.
19. **Replace `feed_panel.rs:99–102`, `script_panel.rs:613/301/392`, `news_panel.rs:59` painter dividers** with `ui.separator()` or a `Divider` widget.

### 15.11 Updated punch list — Tier 3 (newly surfaced)

20. **`ChipShell` migration** — extend `ChipVariant` with Status/Keybind/Display/Removable; migrate `pill_button`/`status_badge`/`keybind_chip`/`display_chip`/`removable_chip` to `ChipShell`; deprecate the free fns.
21. **Disclosure widget** — currently underused; sweep for `ui.collapsing` and structured disclosure patterns.
22. **PillSelector / SegmentedControl** unification — `TimeframeSelector`, `FilterPill`, `tab_strip`, `NmfToggle` all implement variants of the same pattern.
23. **Dead code review** — `Variant::Danger` (0 sites), `Variant::Link` (2 sites), `.tint()` (4 sites), `.hover_fill()` (2 sites). Either resurface them or remove.

---

## Appendix B — Grep recipes for follow-up work

```bash
# Theme threading violations
rg "&crate::chart_renderer::gpu::THEMES\[0\]"     # 56 hits
rg "fn ft\(\) -> &" --type rust                    # ~20 helpers

# Hardcoded shadows (light-theme bugs)
rg "from_rgba_unmultiplied\(0, 0, 0,"              # ~6 hits

# Hardcoded font sizes
rg "FontId::monospace\(\d"                         # ~250 hits
rg "FontId::proportional\(\d"                      # ~30 hits

# Hardcoded strokes
rg "Stroke::new\(\d"                               # ~260 hits

# `egui::Button` outside ui_kit
rg "egui::Button" -g '!ui_kit/**'                  # ~83 hits

# Raw menu buttons
rg "ui\.menu_button"                               # ~33 hits

# Color-dimming chains (missing helper)
rg "gamma_multiply\(0\.\d"                          # ~30 hits
```
