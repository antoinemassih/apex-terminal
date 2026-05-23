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
