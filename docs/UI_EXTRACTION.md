# Apex Terminal — UI Kit Extraction Plan

Companion to `STATE_ROADMAP.md` (state architecture) and `AUDIT.md`
(application audit). This document is the plan to extract the design system +
widget kit into a reusable, app-agnostic surface — eventually shippable as a
separate `apex-ui` crate that a doc-writing app or any other UI app can adopt.

## Reality check

The earlier audit estimated **7 inverted imports**. The actual count is **29**:

| Pulled from `chart_renderer::*` | Where |
|---|---|
| `gpu::Theme` (concrete) | `panel_section`, `panel_list_row`, `panel_sub_section`, `panel_key_value_row`, `panel_loading`, `panel_error`, `table_header`, `pill_row`, `side_panel_shell`, `split_section_panel` |
| `ui::style::*` (token helpers) | `tooltip`, `context_menu`, `popover`, `hover_card`, `table`, `header` |
| `ui::components::frames_widget::{PopupFrame, BorderAlpha}` | `context_menu` |
| `chart_renderer::LineStyle` | `widgets/mod.rs` |
| `gpu::Watchlist` (god-object!) | `side_panel_shell`, `split_section_panel` |

And the `ComponentTheme` trait itself has `bull()` / `bear()` methods —
trading-flavoured, not portable to a doc app as-is.

The honest scope is **5–7 days of focused work**. Below is the phasing.

## Target

```
apex-ui/                      # workspace crate, depended on by any UI app
    src/
        tokens/               # font/gap/alpha/stroke helpers — pure primitives
        theme/                # ComponentTheme trait + portable Theme struct
        widgets/              # Button, MenuItem, ToolBarButton, Modal, …
        icons/                # generic Phosphor icons
        design_system/        # StyleSystem × ColorScheme axes, DTCG loader

apex-native/  (this trading app)
    src/chart_renderer/
        theme.rs              # TradingTheme = Theme + bull/bear/…; impl ComponentTheme
        icons.rs              # CURRENCY_DOLLAR, LADDER, etc.
        …                     # everything else stays
```

Chart canvas (the hot path) stays on its own path (direct field access on
`TradingTheme`); widgets keep using `&dyn ComponentTheme`. **No perf change.**

## Phases (each ships green)

### Phase 1 — Move portable tokens & primitives into `ui_kit`
- Create `ui_kit::tokens` with the pure helpers currently in
  `chart_renderer::ui::style` (font sizes, gaps, alphas, `color_alpha`,
  `color_dim`, stroke widths). State-bearing pieces (`STYLE_STORE`,
  `ACTIVE_STYLE`, `style_store()`) stay in `chart_renderer::ui::style`.
- Move `LineStyle` enum to `ui_kit`.
- Move `PopupFrame` / `BorderAlpha` (`frames_widget`) into `ui_kit::widgets`.
- `chart_renderer::ui::style` re-exports from `ui_kit::tokens` so the rest of
  the chart app keeps building unchanged.
- Update the ~15 ui_kit widgets that import `chart_renderer::ui::style::*`
  to use `crate::ui_kit::tokens::*`.

**Exit:** ~15 of the 29 inverted imports are eliminated. Build green; runtime identical.

### Phase 2 — Decouple `ComponentTheme` from trading colours
- Remove `bull()` / `bear()` from the core `ComponentTheme` trait (or give
  them default impls returning `accent()`).
- Introduce a sibling trait `TradingTheme: ComponentTheme` with the trading
  methods (`bull`, `bear`, others if needed). The chart app's widgets that
  need bull/bear take `&dyn TradingTheme` instead.
- Move `impl ComponentTheme for chart_renderer::gpu::Theme` OUT of `ui_kit`
  (it currently lives inside `ui_kit::widgets::theme`) into `chart_renderer`.

**Exit:** `ComponentTheme` is genuinely portable. ui_kit no longer references
`chart_renderer::gpu::Theme` in its trait machinery.

### Phase 3 — Portable `Theme` struct
- Define `apex_ui::Theme` (or `ui_kit::theme::Theme`) — a plain struct with
  the semantic tokens (`bg`, `surface`, `text`, `dim`, `accent`, `border`,
  `success`, `warning`, `danger`, …) and an `impl ComponentTheme`.
- `chart_renderer::gpu::Theme` becomes a *superset*: `{ core: ui_kit::Theme,
  bull, bear, … }`. It implements both `ComponentTheme` (delegating to
  `core`) and `TradingTheme` (its own fields).

**Exit:** A doc app can construct `ui_kit::Theme` directly and use every
widget that takes `&dyn ComponentTheme`.

### Phase 4 — Fix remaining inverted imports
- The widgets still importing `gpu::Theme` directly (`panel_section`,
  `panel_list_row`, etc., ~10 sites) get rewritten to take `&dyn
  ComponentTheme`.
- The widgets importing `gpu::Watchlist` (`side_panel_shell`,
  `split_section_panel`) are genuinely **not portable** — they need a god-
  object slice. Decision: either (a) keep them in `chart_renderer` (move out
  of `ui_kit`), or (b) pass them a generic data-source trait. Likely (a) is
  right — they're app-specific composites, not primitives.

**Exit:** `ui_kit` has **zero** imports from `chart_renderer`.

### Phase 5 — Extract to a workspace crate
- Add an `apex-ui` workspace member. Move `src/ui_kit/` and
  `src/design_system/` into it. Update `Cargo.toml`s. The trading app
  depends on `apex-ui`.

**Exit:** `apex-ui` can be referenced from any other workspace project.

### Phase 6 (optional) — Split icons
- Generic Phosphor icons stay in `apex-ui::icons`. Trading-specific
  (`CURRENCY_DOLLAR`, `LADDER`, etc.) move to `chart_renderer::icons`.

## Principles

- **Always green.** Every phase ends with the trading app building and tests
  passing. No big-bang.
- **Chart canvas untouched.** The hot paint path stays on direct
  `TradingTheme` field access — no trait dispatch added there.
- **Back-compat re-exports during transition.** When primitives move, the
  old path re-exports for a phase so we don't have to update every chart-app
  call site in the same PR.
- **One owner per file.** Same discipline as the state roadmap.

## Risk

Low–medium. This is a structural refactor of code we already understand —
no new behaviour, no money path, no hot-path math. Worst case: a missed
import gets caught by `cargo check`.

---

## Status (autonomous push 2026-05-23)

### Done
- **Phase 1a** — `LineStyle` moved to `ui_kit::LineStyle`. Re-exported
  from `chart_renderer` for back-compat. (commit `0c1633f1`)
- **Phase 1b** — `ui_kit::tokens` module re-exports the chart-app style
  helpers; 15 widget files swapped from `chart_renderer::ui::style::*`
  to `crate::ui_kit::tokens::*`. (commit `da04b7c5`)
- **Phase 1c** — `ui_kit::widgets::theme` + `ui_kit::widgets::frames`
  centralise every chart_renderer re-export in two bridge files. 12+
  widget files swapped to those bridges. (commit `5a4e8432`)
- **Phase 2** — `ComponentTheme` gains semantic `success()` /
  `danger()` methods (default impls delegate to `bull()` / `bear()`).
  Form-error widgets migrated to `.danger()`. (commit `b81c0374`)
- **Phase 2b** — `impl ComponentTheme for Theme` moved out of
  `ui_kit` into `chart_renderer::theme_impl` (correct dep direction).
  `active_theme()` follows it; `active_theme_idx()` stays portable.
  (commit `23fe3bdd`)
- **Phase 2c** — `ComponentTheme` gains 5 semantic surface methods
  (`surface_border`, `header_surface`, `section_header_surface`,
  `panel_surface`, `header_border`) with default impls so widgets can
  compute these without reaching for the concrete Theme. (commit
  `fb3c7310`)

**Inverted-import count: 29 → 8** (all 8 are now centralised in 4
bridge files: `ui_kit/mod.rs` (doc), `ui_kit/tokens.rs`,
`ui_kit/widgets/theme.rs`, `ui_kit/widgets/frames.rs`).

Builds clean (default + design-mode); 575 lib tests pass.

### Re-audit findings — what still blocks a true `apex-ui` crate

A read-only Sonnet audit ran against the post-Phase-2c state. Critical
blockers (citations in `docs/AUDIT.md` style):

1. **Direct field access on `&Theme`** — 8+ widget files (`shell_variants`,
   `side_panel_shell`, `split_section_panel`, `table_header`,
   `panel_toolbar`, `panel_list_row`, `panel_section`) hit
   `t.toolbar_border`, `t.toolbar_bg`, `t.dim` as struct fields rather
   than trait methods. They take `t: &Theme` (concrete) — extraction
   blocked because a doc app has no `Theme`.
2. **`pub(crate)` style helpers take `&Theme`** — `panel_surface`,
   `header_surface`, `header_border`, `section_header_surface` in
   `chart_renderer::ui::style`. Equivalents now exist as trait methods
   (Phase 2c) but widgets still call the chart-app versions.
3. **`active_theme()` ambient access** — 14+ widgets call
   `super::theme::active_theme(ctx)` to grab the trading Theme inside
   `Widget` impls that take no theme arg. A standalone crate needs an
   ambient-theme mechanism (e.g., `set_ambient_theme` stash in egui
   memory holding `Arc<dyn ComponentTheme>`).
4. **`Watchlist` coupling in `pane_aligned`** — `SidePanelShell` and
   `SplitSectionPanel` reach for `wl.pane_header_size.title_font()`
   and `pane_tabs_header_h(wl)`. Trading god-object — either delete
   `pane_aligned` from a portable build or feature-gate it.
5. **`tokens.rs` is a glob re-export** of `chart_renderer::ui::style`.
   The pure math helpers (~80 fns) need to physically move into
   `ui_kit/tokens.rs`. The `FRAME_TOKENS` / `ACTIVE_STYLE` /
   `STYLE_STORE` machinery stays in `chart_renderer`.
6. **`frames_widget` bodies call** `current()` and `active_theme()` —
   if extracted, those calls need to be replaced with explicit
   parameters.
7. Trading-domain widgets (`trade_card`, `risk_reward_bar`) should
   move OUT of `ui_kit` into a `chart_renderer::ui_kit_extensions`.

### Shortest path to a compilable `apex-ui` crate

In dependency order — each step ships green:

1. **Move pure token helpers physically.** ~80 `font_*`/`gap_*`/
   `color_alpha` / `alpha_*` / `stroke_thin`-style one-liners from
   `chart_renderer::ui::style` into `ui_kit::tokens` as owned code.
   Stop the glob re-export.
2. **Migrate widgets to semantic methods.** Replace `t.toolbar_border` →
   `t.surface_border()`, `panel_surface(t)` → `t.panel_surface()`, etc.
   Change widget signatures from `t: &Theme` to
   `t: &impl ComponentTheme`.
3. **Move frame types physically.** Copy `PanelFrame`, `CardFrame`,
   `PopupFrame`, `BorderAlpha`, `CompactPanelFrame` into
   `ui_kit/widgets/frames.rs` as owned code. Strip the `current()`
   dependency by taking explicit `corner_radius`/`shadow_alpha`.
4. **Solve the ambient-theme problem.** Introduce
   `set_ambient_theme(ctx, &dyn ComponentTheme)` /
   `get_ambient_theme(ctx)` using egui memory. Replace the 14+
   `active_theme(ctx)` calls inside `Widget` impls.
5. **Excise `pane_aligned`** (or feature-gate) on the panel shells.
6. **Move chart-app-coupled widgets out of `ui_kit`**: `trade_card`,
   `risk_reward_bar`, the panel-shell `pane_aligned` paths.
7. **Create the workspace crate.** `crates/apex-ui/`. Move
   `src/ui_kit/` and `src/design_system/` into it. Trading app
   depends on it. Bridge files in `chart_renderer::theme_impl` and a
   new `chart_renderer::icons` (trading icons) cover the remaining
   adapter surface.

Effort: realistically 3–5 days of focused work, mostly mechanical
migration in steps 1–2 and a small design move in step 4.
