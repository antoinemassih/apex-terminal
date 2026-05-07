# Component Library Port Plan

## Source attribution
- Repo: https://github.com/longbridge/gpui-component
- License: Apache 2.0 (and MIT — dual-licensed in repo root; LICENSE-APACHE confirmed)
- License obligation: Preserve copyright headers on any file ported verbatim; include NOTICE / Apache-2.0 attribution in `src-tauri/src/ui_kit/widgets/THIRD_PARTY.md`. Heavy paraphrases (idiomatic egui rewrites) still warrant attribution because the API shape and visual design are clearly derivative.
- Commit hash sampled: `42ae00839e24c10f55ea0fe88f547b5366a19404` (main, Feb 2026, v0.5.1 era)

> Note: gpui-component targets GPUI (Zed's framework). We are not porting code line-for-line — GPUI is retained-mode element trees, egui is immediate-mode. We are porting **API surface, visual language, variant taxonomy, and naming**. Treat the source as a reference spec, not a transliteration target.

## Inventory

| Name | gpui-component path | Variants | Apex equivalent today | Port priority | Effort | Notes |
|---|---|---|---|---|---|---|
| Button | `button/` | primary, secondary, ghost, danger, link; xs/sm/md/lg; icon-only, loading | `inputs/buttons.rs::IconBtn`, `TradeBtn`, `ChromeBtn`; `components/toolbar/mod.rs::ToolbarBtn`; `components/action_button.rs` | P0 | L | 4 overlapping button structs today — unification is UI improvement #4. Highest-value port. |
| Input (text) | `input/` | default, with prefix/suffix, password, clearable, sizes | `chart/renderer/ui/components/inputs.rs` | P0 | L | gpui's input is huge (LSP-grade). Port only single-line first; defer multiline / code editor. |
| Select / Dropdown | `select/` | single, multi, searchable | none — ad-hoc popovers | P0 | M | Needed for design-mode panel, symbol pickers. |
| Modal / Dialog | `dialog/` | alert, confirm, sheet | `chart/renderer/ui/chrome/modal.rs::Modal` | P0 | M | Replace ours; ours is functional but no size variants / no scroll-lock. |
| Tooltip | `tooltip.rs` | hover, instant, rich content | ad-hoc `on_hover_text`; deferred `WlTooltipData` in watchlist_panel | P0 | S | egui has built-in; port adds: delay control, rich content, deferred data pattern. |
| Tabs | `tab/` | line, segmented, closable, draggable | `components/tabs.rs` | P1 | M | Ours lacks closable + drag-reorder needed for chart tabs. |
| ContextMenu | `menu/` | nested, separators, kbd hints | `components/context_menu.rs` | P1 | M | Ours works but no nested submenus; add kbd shortcut display. |
| Toast / Notification | `notification.rs` | info/success/warn/error, action button | `components/status.rs::Toast` | P1 | S | We have it; add stacking + auto-dismiss queue. |
| Switch | `switch.rs` | sm/md, label-on-side | none (we use checkbox-ish) | P1 | S | Used heavily in design-mode panel. |
| Checkbox | `checkbox.rs` | tri-state | ad-hoc | P1 | S | |
| Radio | `radio.rs` | group | ad-hoc | P1 | S | |
| Slider | `slider.rs` | range, stepped, vertical | egui built-in used directly | P1 | S | Port wrapper only for theming consistency. |
| Tag / Badge | `tag.rs` + `badge.rs` | colors, dot, count | `components/chips.rs::Chip`, `pills_widget.rs::PillButton` | P1 | S | Reconcile Chip vs Pill vs Tag — pick one name. Recommend `Tag` (gpui) + keep `Pill` only for filter-style. |
| Label | `label.rs` | sizes, weights, muted | `components/labels.rs`, `semantic_label.rs` | P1 | S | Mostly a rename / token alignment. |
| DatePicker | `time/` | single, range | none | P2 | L | Needed for trade-history filters; depends on Calendar. |
| Calendar | `time/` | month, week, multi-month | none | P2 | M | |
| Tree | `tree.rs` | virtualized, expand/collapse | none (watchlist uses flat list) | P2 | M | Useful for symbol groups, file pickers. |
| Table | `table/` | virtual rows/cols, sortable, resizable cols | sortable_headers.rs + ad-hoc grids | P2 | L | Big lift. We already have sortable headers + virtualized watchlist; consolidate. |
| Resizable / SplitPane | `resizable/` | h/v, snap, collapse | bespoke in chart panel layout | P2 | M | Replace bespoke pane splitters. |
| Sidebar | `sidebar/` | collapsible, sections | `components/panels.rs` partially | P2 | M | |
| Popover / HoverCard | `popover.rs` + `hover_card.rs` | placement, arrow | partial (modal-as-popover hack) | P2 | M | **Risk:** real soft-shadow popovers need offscreen blur; egui can't easily. Use solid border + 1px drop. |
| Sheet (drawer) | `sheet.rs` | left/right/top/bottom | none | P2 | M | Useful for trade-ticket slide-in. |
| Breadcrumb | `breadcrumb.rs` | separator variants | none | P3 | S | |
| Pagination | `pagination.rs` | numeric, simple | none | P3 | S | Needed if we paginate large trade history. |
| Skeleton | `skeleton.rs` | text, rect, avatar | none | P3 | S | Pairs with async data loads. |
| Spinner / Progress | `spinner.rs` + `progress/` | linear, circular, indeterminate | ad-hoc | P3 | S | egui has spinner; port progress bar styling. |
| Stepper | `stepper/` | h/v, numbered | none | P3 | M | Onboarding/wizards only. |
| ColorPicker | `color_picker.rs` | swatches, hex, alpha | none (design-mode uses raw rgb fields) | P3 | M | Useful in design-mode panel. |
| Avatar | `avatar/` | image, initials, group | none | P3 | S | Low value for a trading terminal. |
| Accordion / Collapsible | `accordion.rs` + `collapsible.rs` | — | none | P3 | S | Probably skip — we use Tabs/Sidebar instead. |
| Alert | `alert.rs` | info/warn/error inline | ad-hoc | P3 | S | |
| Kbd | `kbd.rs` | inline keycap | none | P3 | S | Pairs with ContextMenu shortcut hints. |
| Link | `link.rs` | — | ad-hoc | P3 | S | |
| Separator | `separator.rs` | h/v, label | `components/hairlines.rs` | — | — | We already have it; just rename/align tokens. |
| Rating | `rating.rs` | — | none | — | — | **Skip** — no use case in trading terminal. |
| Markdown / HTML / Code Editor / Charts | various | — | none | — | — | **Skip** — out of scope; egui_commonmark + our chart renderer cover us. |
| ScrollArea | `scroll/` | — | — | — | — | **Skip** — egui's `ScrollArea` is excellent; don't reinvent. |
| VirtualList | `virtual_list.rs` / `list/` | — | watchlist uses egui windowing | — | — | **Skip wholesale port** — keep our custom virtualization; steal API surface only if needed. |
| TitleBar / WindowBorder | `title_bar.rs`, `window_border.rs` | — | Tauri provides | — | — | **Skip** — Tauri owns chrome. |

## Recommended port sequence

1. **Foundation pass (1 wk)** — Token reconciliation: map gpui's theme tokens onto our `ui_kit/theme.rs`. Define `ComponentTheme` trait. Set up `src-tauri/src/ui_kit/widgets/` module skeleton + `THIRD_PARTY.md`. No widgets yet — unblocks everything below.
2. **Milestone 1 — Primitives (3 days):** Button, Label, Tag, Separator, Kbd. Shared dependency: tokens + sizing scale (xs/sm/md/lg). Replaces 4 overlapping button structs.
3. **Milestone 2 — Form atoms (3 days):** Switch, Checkbox, Radio, Slider (wrapper). Shared input idioms; small API surface; immediately consumable by design-mode panel.
4. **Milestone 3 — Text input (3-5 days):** Input (single-line) with prefix/suffix/clearable. Larger because IME + selection nuance in egui.
5. **Milestone 4 — Overlays I (3 days):** Tooltip, Toast, Alert. Shared layering / z-order plumbing.
6. **Milestone 5 — Overlays II (4-5 days):** Popover, HoverCard, Modal (rebuild on Popover positioning), Sheet. Shared placement engine. **Highest risk milestone — see Risks.**
7. **Milestone 6 — Selection (4 days):** Select, ContextMenu (port menu engine once, both consume it). Depends on M5 popover.
8. **Milestone 7 — Navigation (3 days):** Tabs (closable + drag), Breadcrumb, Sidebar, Link. Replaces components/tabs.rs and panels.rs.
9. **Milestone 8 — Data display (5 days):** Table (sortable, resizable cols, virtual), Tree, Pagination, Skeleton, Spinner/Progress. Big consolidation of sortable_headers + watchlist patterns.
10. **Milestone 9 — Time (4 days):** Calendar, DatePicker (range). Self-contained.
11. **Milestone 10 — Polish & migration (1 wk):** ColorPicker, Stepper, Resizable. Migrate remaining call-sites off legacy structs; delete `inputs/buttons.rs::{ChromeBtn,TradeBtn,IconBtn}` and `chrome/modal.rs`.

Total calendar: ~7-8 weeks single dev, 4-5 with parallelism (M2/M3 + M9 are independent).

## Architectural notes

**Location.** Put the new library at `src-tauri/src/ui_kit/widgets/` (sibling to `theme.rs`, `icons.rs`, `symbols.rs`). Justification: `ui_kit` already owns visual primitives and is consumed by `chart_renderer` + `chart` + `watchlist`. A top-level `src-tauri/src/components/` would invert the dependency (widgets are cosmetic consumers of tokens, not peers). Keep `chart/renderer/ui/components/` for **chart-specific** widgets (frames, hairlines, perf_hud) — those don't graduate to the kit.

**Naming.** gpui uses unprefixed (`Button`, `Input`). We have `Modal`, `Toast`, `ContextMenu` already in `chart/renderer/ui` — direct collisions. Convention: **unprefixed names inside `ui_kit::widgets`**, accessed as `ui_kit::widgets::Button`. Migrate existing `Modal`/`Toast`/`ContextMenu` into the kit (they become *the* implementation), and re-export from old paths during migration with `#[deprecated]`. No `Apex`/`Ax` prefix — namespacing solves it.

**Motion.** `chart/renderer/ui/components/motion.rs` exposes easing + timing primitives. Widgets consume it via a `Motion` trait the kit defines, default-implemented to delegate to the existing module. This avoids a hard dep from `ui_kit` → `chart/renderer/ui`. If needed, lift `motion.rs` into `ui_kit/motion.rs` (low-risk move).

**Theme.** Today `crate::chart_renderer::gpu::Theme` is the source of truth. Don't make widgets take `&Theme` directly — that couples cosmetic widgets to GPU types. Define `ui_kit::widgets::ComponentTheme` trait with the slice of color/spacing tokens widgets need (~20 fields), and `impl ComponentTheme for Theme`. Widgets take `&dyn ComponentTheme` or generic `T: ComponentTheme`. Net effect: the kit could be extracted as its own crate later.

**Migration strategy.** Coexist, don't big-bang. Each milestone ships a kit widget AND migrates 1-2 known call-sites as proof. Old widgets get `#[deprecated(note = "use ui_kit::widgets::X")]` immediately. Final milestone is the deletion pass. This avoids a 7-week red branch.

## Risks

1. **egui can't do real soft shadows / true blur.** gpui-component popovers/sheets rely on it visually. Mitigation: spec a "flat shadow" style (1-2px solid offset + subtle border tint) in tokens; accept the visual delta and document it. Don't try to fake gaussian blur in shaders.
2. **Immediate-mode vs retained-mode API mismatch.** gpui's `RenderOnce` + entity model doesn't translate. Mitigation: treat source as visual/API spec only; build idiomatic egui builders (`Button::new(label).primary().sm().show(ui)`).
3. **Scope creep from "60+ components".** Many (Markdown, Code editor, Plot, Highlighter) are massive and unneeded. Mitigation: this plan explicitly skips them; require a written justification to add anything not in the inventory.
4. **Theme/token churn mid-port.** The R5 token promotion is recent (`r5-token-promotion-summary.md`); another churn would invalidate widgets. Mitigation: freeze token names at end of Milestone 0; defer additions to a single batched pass after Milestone 5.
5. **Existing call-site sprawl.** `IconBtn`/`ToolbarBtn`/`ChromeBtn`/`TradeBtn` are used widely. Grep before each milestone; migration commit per call-site cluster, not one mega-commit.

## What I'd do first

1. **Button** — unblocks the 4-way button mess (UI improvement #4); every other widget depends on its sizing/variant tokens.
2. **Tooltip** — trivial port, high daily use, formalizes the deferred-data pattern from `watchlist_panel.rs::WlTooltipData`.
3. **Modal** — already exists; reshaping it into the kit defines our overlay/z-order plumbing for Popover later.
4. **Input** — gates design-mode and any future trade-ticket work; longest-lead-time atom, so start early.
5. **Select** — paired with Input; needed everywhere we currently fake a dropdown with a popover hack. Forces the popover positioning decision before Milestone 5.
