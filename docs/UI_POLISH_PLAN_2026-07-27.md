# Apex Terminal — UI Polish Execution Plan
**2026-07-27 · derived from `UI_AUDIT_2026-07-27.md` · item-by-item, for agents to execute**

## Guiding principle (read first)

**One design system, zero call-site escape hatches.** Apex's custom `ui_kit` + `design_system` is the asset — keep and finish it; do **not** migrate to egui standard widgets. egui is the *substrate* your widgets are built on, not something call sites reach for directly. Every finding below is about routing production code through the components you already built, deleting the dead duplicates, and then layering a signature identity on top.

**The one native exception:** context menus. The app already standardized on native `egui::Response::context_menu()` + `MenuItem`; the custom `ContextMenu` widget (507 LOC) is dead and is retired in U0, not adopted.

**Rules for every code-touching item:**
- Route through `ui_kit`/`design_system`; never introduce a new raw `egui::Window`/`ui.small_button`/`RichText::new`/`Color32::from_rgb`/hand-painter call at a feature call site.
- Corpus-gate before "done": `python dev/run_corpus.py` from repo root → 1067/1067, 0 real (these touch render paths). Run `python dev/quality_gate.py` too.
- Prefer pure, testable helpers for any new logic (matches the codebase's audit-era discipline).
- Full evidence/`file:line` for each item lives in `UI_AUDIT_2026-07-27.md`; only anchors repeated here.

Waves are ordered by leverage×trust: clean/wire first (removes landmines, makes later work land on real surfaces), then correctness, then the visible identity win, then consolidation, then chrome/motion.

---

## WAVE U0 — Foundation: delete dead duplicates + wire the orphaned components
*Highest leverage, lowest risk. Resolves a slice of every dimension at once by making production actually use the kit.*

### U0-1 · Delete dead/duplicate widgets  [S]
Remove code with zero production callers (grep-verified in the audit). Deleting removes landmines (engineers keep importing the wrong `PanelSection`).
- `panels/kit.rs:584-767` — dead `PanelSection`/`PanelEmpty` shadow structs (real ones live in `ui_kit/widgets/`).
- `ui_kit/widgets/shell_variants.rs` — `ButtonVariant`/`ChipVariant`/`InputVariant` (0 callers; keep `CardVariant`/`RowVariant` which are live).
- `ui_kit/widgets/context_menu.rs` — the 507-LOC `ContextMenu`/`MenuBuilder`/… (superseded by native `resp.context_menu()` + `MenuItem`; also lacks Esc/click-outside so it'd be a regression if adopted).
- `ui_kit/widgets/tree.rs` — `Tree` (its one consumer migrated OFF, `object_tree.rs:17-21`).
- `pane_grid.rs:591-968` — the render/chrome/context-menu half (KEEP the data-model half `PaneState`/`Node`/`split_rect`/`Axis`, which `pane_layout.rs` uses). Split the file.
- `button.rs` — zero-caller constructors `icon_toolbar`/`icon_panel_header`/`icon_tab_close`; and `Icon::button/_colored/_large` in `icons.rs:159-175`; `button_style.rs` 5 zero-caller impls.
**Accept:** `cargo build` + `cargo test --lib` clean; grep confirms no remaining callers; `quality_gate.py` dead-code ratchet drops; corpus 1067/1067 (pure removals must not change render).

### U0-2 · Collapse 3 toast systems → `ui_kit::Toast`  [M]
Today: "Toast 2.0" (`top_nav.rs:1535-1846`, hand-rolled `egui::Window`), `pending_order_toasts.rs` (`OutlinedBox`), and the real `ui_kit::widgets::toast.rs` (used only in storybook). `toast.rs:86-88` was already extended (`.border_color()`) to absorb the pending-order site.
- Migrate `pending_order_toasts.rs` onto `Toast`.
- Migrate/retire "Toast 2.0" onto `Toast`, or document why chart-anchored vs screen-anchored must diverge and share chrome (radius/shadow/severity color) either way.
- Give `Toast` a real exit fade using `Modal`'s `closing_t` pattern (`modal.rs:263-279`).
**Accept:** one toast implementation; severity→color from `NotificationSeverity`; corpus.

### U0-3 · Wire `StatusPill` into the status surfaces  [S/M]
Route the 4 hand-rolled connection/data vocabularies through the built-but-unused `StatusPill`: `dom_panel.rs:271-313` (LIVE/SIMULATED), `bottom_dock.rs:329-331` (LIVE/OFFLINE), `portfolio_pane.rs:156-160` (OK/TRIPPED/HALTED/DISCONNECTED). One tone table, one shape.
**Accept:** LIVE/SIMULATED/PAPER/OFFLINE render identically everywhere; corpus (DOM badge is the reference — keep its behavior).

### U0-4 · Wire `PanelError`; stop misusing `PanelEmpty` for errors  [S/M]
`PanelError` has 0 adopters. Wire it into the network-fetch panels (scanner, screener, connection) and convert the ~6 "…feed not connected" sites currently using `PanelEmpty` (`research_panel.rs:101,189,232,257,287`, `bottom_dock.rs:323`, `portfolio_pane.rs:172-173`) so an outage reads as an error (icon+tint+retry), not a shrug.
**Accept:** disconnection visibly distinct from empty; corpus.

### U0-5 · Wire `PanelLoading` into the ad-hoc loaders  [S/M]
Adopted in only 3 panels; ~17 others use scattered spinner/skeleton/"Loading…". Replace those with `PanelLoading`.
**Accept:** one loading treatment app-wide; corpus.

### U0-6 · Wire `ConfirmDialog` for destructive actions  [S/M]
`ConfirmDialog` has 0 production callers; destructive actions use ad-hoc `Variant::Danger` flows. Route flatten/cancel-all/reverse/clear-history/paper→live etc. through it.
**Accept:** destructive confirms share one preset; corpus (respect the W0-10/W0-11 two-step arm behavior already shipped).

### U0-7 · Cheap doc/label fixes  [S]
`scanner_panel.rs:477`+`:90-91` double-title; stale `SplitSectionPanel` doc comments in `analysis_panel.rs:3-7` and `signals_panel.rs:3-6`; `top_nav.rs:1537-1542` stale byte-prefix toast doc; `header_buttons.rs:11` stale comment.
**Accept:** no behavior change; prevents copying stale patterns as "references."

---

## WAVE U1 — Correctness & accessibility
*Fast, high-trust. Real bugs, several theme/a11y-breaking.*

### U1-1 · Run the WCAG validator on shipped themes; fix failing contrast  [S/M]
`theme_pack::validate::check_accessibility` (`validate.rs:423-467`) already exists but is never run against `builtin_color_schemes()` or the two hand-written defaults. Add a `#[test]` over all of them; fix the 9 failing `dim`/`bg` pairs to ≥4.5:1 (Kanagawa 2.55, Midnight 3.45, Rosé Pine 3.42, Lucid 3.13, Bauhaus/Peach/Ivory/Newsprint, + `default_dark`/`builtin_light`).
**Accept:** contrast test green over every built-in theme; no `dim` regressions.

### U1-2 · Kill the parallel hardcoded palette (P&L color bug)  [S/M]
`chart/renderer/ui/style.rs:764-810` (`COLOR_PROFIT_GREEN`/`COLOR_LOSS_RED`/`TEXT_PRIMARY`/…) consumed at `render/pane/core.rs:35,10770,10782` etc. make P&L labels ignore the active theme. Delete; route through `t.bull()/t.bear()`/`palette_ct(t)`.
**Accept:** P&L color follows theme under Monokai/etc.; corpus.

### U1-3 · Fix hardcoded white chart-axis text (light-theme contrast bug)  [S]
`core.rs:6991-7039` paints crosshair/time-axis text in literal `Color32::WHITE` on a theme-variable fill. Use `t.text`/`contrast_fg(t.toolbar_bg)`. (Leave selection-handle white strokes — those are intentional universal-contrast markers.)
**Accept:** axis readouts legible under the light theme; corpus.

### U1-4 · Close focus-ring + disabled-border gaps  [S/M]
Extend the existing `st::cursor::focus_ring` to the ~11 keyboard-operable widgets missing it (`text_area`, `tag_input`, `segmented_control`, `toggle_group`, `tabs`, `slider`, `range_slider`, `number_stepper`, `stepper`, `menu_item`, `search_input`). Add disabled-border dim to `input.rs`/`text_area.rs`/`select.rs`. Fix `Size::Md` rendering two point sizes (`input.rs:240`, `number_stepper.rs:76` vs `tokens.rs:74`).
**Accept:** keyboard focus visible on every interactive widget; disabled states uniform; corpus.

### U1-5 · Tab bar: fix close-on-inactive + add overflow  [M]
`chart.tab_hovered` is never set, so inactive tabs can't be closed without activating first (`painter_pane.rs:791-792`, `gpu.rs:2565`). And there's no overflow strategy — many tabs overlap the action cluster (`painter_pane.rs:683-819,1020`). Fix hover-close; add overflow (scroll/"+N"/clip-fade). (Note: U4-1 may supersede by moving onto `chrome/pane.rs` — coordinate.)
**Accept:** any tab closable; many-tab pane doesn't collide chrome; corpus with a many-tab scenario.

### U1-6 · Correctness batch (small, isolated)  [S]
- Option badge painted across unrelated stock tabs sharing a pane (`core.rs:536-538`/`painter_pane.rs:772-778`) — gate on the tab's own instrument.
- Toast pin icon both-branches-identical (`top_nav.rs:1777`).
- "Suites" submenu lying-buttons (`chart_controls.rs:738-739`) — hide or tooltip "coming soon".
- `Label::truncate(true)` wraps instead of eliding (`label.rs:116-124`) — implement ellipsis via `layout_no_wrap` (mirror `date_picker.rs:253`).
- 3 toolbar icon collisions (`top_nav.rs:713/1135, 1103/1228, 1267/1268`) — unique glyph each. IBKR button hand-built glyph → `Icon::`.
**Accept:** each fixed + a unit test where pure (truncate, badge-scoping); corpus.

---

## WAVE U2 — Signature identity
*The visible "wow." Converge on the existing `ApexTerminalThemes` React reference (`DESIGN_PORT_HANDOFF.md` → `FIDELITY-AUDIT.md`/`PRIMITIVE-PARITY.md`).*

### U2-1 · Ship ONE signature Apex palette as the flagship default  [M]
Design a bespoke Apex identity palette (its own gravity — the way TV=blue, Bloomberg=amber, Bookmap=heat). Make it the default `ColorScheme`; demote the editor-named themes (Nord/Dracula/Monokai/…) to a secondary "Classic/Developer" tier. Validate against the React reference.
**Accept:** default launch look is distinctively Apex, passes U1-1 contrast; corpus.

### U2-2 · Curate the Theme×Style settings  [S/M]
Today: two orthogonal full grids = 135 uncurated combos (`settings_panel.rs:131-170`). Replace with curated "Looks" (pre-paired theme+style with a recommended default badge); put "mix your own" behind Advanced. Cut or bring-to-parity the 3 unaudited style systems (Octave/Relay/Glass) vs the React reference's 6.
**Accept:** first-run picks a flagship Look, not a slot machine; corpus.

### U2-3 · Typography adoption sweep  [M]
- Wire `line_height_factor` into `TextStyle::as_rich()` (`text_style.rs:65-71`) — a fully-designed, zero-effect layer today.
- Migrate the worst off-scale sizes (watchlist hover 18/12, journal 34, icons 24, theme_studio literals) onto `TextStyle`/`font_*()`; begin the broader 406-raw-`RichText` migration in the highest-traffic panels.
- Prune dead tiers (`Display`/`HeadingLg`/`NumericHero`); fix `font_2xl()==font_lg()` alias.
- Hero-number font: pin to tabular JetBrains Mono (stop the serif swap that makes NAV digits jiggle, `style.rs:2731-2737`); fold `font_hero:36` into the scale.
**Accept:** vertical rhythm improves via line-height; off-scale sizes gone in touched panels; corpus.

### U2-4 · Iconography unification  [M]
- Replace raw `✕/✗` → `Button::close()`; `⚠/✓/▲▼/▾▸/●○` → `Icon::*`; emoji (`🔍📋📎✦👁📖`) → existing `Icon::MAGNIFYING_GLASS/SPARKLE/EYE/BOOK_OPEN`.
- Extend the `Icon` catalog for the honestly-admitted gaps (halt/dividend/split/news/droplet/briefcase) so `watchlist_badges.rs` + the `mod.rs` overlay map stop needing emoji.
- Fix `Icon::ARROW_LEFT/RIGHT` to use `ph::` constants (`icons.rs:38-41`).
**Accept:** ~153 raw glyph/emoji usages driven toward 0 in production; one icon per concept; corpus.

---

## WAVE U3 — Component consolidation
*Collapse the 12 duplicate clusters onto single primitives. Reduces surface the identity work must maintain.*

### U3-1 · Chip/pill/tag → one `Chip` + one tone enum  [M]
Merge `Badge`/`StatusPill`/`CountChip`/`Tag` (+ `tag.rs` internal `paint_pill`/`paint_badge`) onto one `Chip` primitive; bridge all tones onto `sx::Tone` via `TagTone::to_tone()`. (Keep `Indicator` separate — dot/pulse only.)
**Accept:** one corner/padding treatment for the chip family; corpus.

### U3-2 · Card/panel container family → one primitive  [M]
Merge `PanelCard`/`OutlinedBox`/`card_slots`/`tiles` frames/`TradeCard`/`ThemePreviewCard` onto one configurable card; make `script_panel.rs:399-404` and the other hand-rolled tone cards use it.
**Accept:** one card chrome; corpus.

### U3-3 · Merge the overlay pairs  [M]
`Popover`+`ToolPopover` (identical purpose, diverging shadow/radius/animation — give ToolPopover the shared `ShadowSpec` + motion) and `Tooltip`+`HoverCard` (differ only by delay + hover-persist). Document one elevation/corner-radius rule for all floating cards (4 undocumented tiers today).
**Accept:** one radius/shadow/motion contract for overlays; corpus.

### U3-4 · Collapse the color-role + size enums  [S/M]
Six color-role enums (`TagTone`/`IndicatorTone`/`panel_section::Tone`/`CountChipTone`/`AlertVariant`/`metric_row::Tone`) → `sx::Tone`. Four numeric-widget size tables (`stepper`/`slider`/`range_slider`/`segmented_control`) → `tokens::Size`. Fix SegmentedControl's byte-identical connected/separated radius + dead `Size::Xl`.
**Accept:** one tone enum, one size scale; corpus.

### U3-5 · Prune `button.rs` API + pick one color-tint system  [M]
Reduce button's 17 constructors + 5 show-paths toward `Variant`+`Size` composition; finish the `Variant::Chrome` escape-hatch cleanup (23 sites). Pick ONE canonical color-tint path (`palette_ct`/`shade` recommended) and finish or formally freeze `color_alpha` (~524 legacy sites) as legacy-only.
**Accept:** button API surface shrinks; a documented canonical tint path; corpus.

### U3-6 · Fold per-preset dims onto the global tier  [S/M]
`StyleSettings` row/button/tab heights + card padding restate raw px per preset; derive them from `gap_*`/`row_height_*` via the existing multiplier pattern (`SpacingScale`/`CornerScale`) so a density change propagates. Add `row_height_header`/`row_height_toolbar` tokens and rewire the 5 pane-chrome height constants.
**Accept:** one density knob moves pane headers/rows/cards together; corpus.

---

## WAVE U4 — Chrome & motion refinement
*The final feel pass. Retire the last hand-rolled surfaces onto the kit; add the motion language.*

### U4-1 · Retire hand-rolled pane header onto `chrome/pane.rs`  [L]
The polished `PaneHeaderBar`/`PaneStatusStrip`/`PaneTimeframeBadge` family (1076 LOC) is unused; the shipped header is hand-rolled `painter_pane.rs` (where tab bugs, missing tooltips, faux-bold, option-badge bleed all live). Migrate the live header onto the kit family in one coordinated pass (supersedes several U1-5/U1-6 symptoms — sequence accordingly). Add tab overflow here if not done in U1-5.
**Accept:** pane header uses the kit; tooltips present; tab overflow works; corpus with many-pane/many-tab scenarios.

### U4-2 · Toolbar hierarchy pass  [M]
Break the two mega-clusters (10-button panel-toggle row `top_nav.rs:1225-1312`; 6-feature `tools_box` `chart_controls.rs:152-921`) into named sub-groups with real dividers/spacing tiers (primary=symbol/timeframe/order, secondary=panel toggles, tertiary=window chrome). Give panel-toggle nav items icons. Add tooltips to window controls. Bring `alert_feed.rs`/`window_controls.rs` onto `KitButton`/`Icon`/`Tooltip`.
**Accept:** glanceable primary/secondary/chrome hierarchy; every control tooltip'd; corpus.

### U4-3 · Chart chrome refinement  [M/L]
The chart is on-screen 100% of the time. Refine axis type scale, gridline weight/color, crosshair/OHLC readout styling, and a distinctive candle/wick treatment; collapse the 4 on-canvas corner-radius conventions to one token; replace fake-bold 0.5px double-draw with a bold `FontId`.
**Accept:** chart reads as premium + distinctly Apex; corpus (candle geometry unchanged — chrome only; do NOT touch candle/axis core geometry per standing constraint).

### U4-4 · Motion language + micro-interactions  [M]
Extend the rail's easing vocabulary to panel open/close, tab switches, modal entry (one physics system, not snap-in-some-places). Add: fill-confirmation flash on `order_row.rs`/`alert_row.rs` (reuse existing `price_flash_tint`), toggle overshoot (`Curve::EaseOutBack`, already built), divider hover accent. Route every pane `set_cursor_icon` through `ui_kit::cursor` so the design inspector stops fighting the live cursor (blocks reliable tooling otherwise).
**Accept:** consistent motion across surfaces; inspector cursor gate honored; corpus.

---

## Sequencing & sizing summary
- **U0** (7 items, mostly S/M) — do first; unblocks and de-risks everything.
- **U1** (6 items, S/M) — fast trust batch; several are real bugs shippable immediately.
- **U2** (4 items, S–M) — the visible identity win; depends on U1-1 (contrast).
- **U3** (6 items, S–M) — consolidation; independent of U2, can interleave.
- **U4** (4 items, M–L) — final feel; U4-1 is the big one and supersedes some U1 tab work.

Total ~27 items. Each is its own corpus-gated commit, matching the world-class-audit execution cadence. Nothing here migrates to egui standard widgets — it all finishes and unifies the custom system, then gives it a signature identity.
