# `pane.rs` Split Plan — **DEFERRED INDEFINITELY**

## Decision

`pane/core.rs` is **sacred**. It contains the GPU-optimized chart paint
pipeline — hot code with measurable per-frame cost. Refactoring it
risks function-call overhead, lost inlining, parameter passing instead
of local-variable access, and cache locality regressions that would
manifest as frame drops on the surface users stare at most.

The organizational wins from splitting it (parallel agent ownership of
indicator / drawing / order renderers) are not worth measurable
performance regression. **Waves 1–5 are not happening.**

## What's still shipped

- **Wave 0** (the directory restructure: `pane.rs` → `pane/core.rs` +
  `pane/mod.rs` re-exporting the public entry points) stays. It's
  purely a file rename with no code change and no hot-path impact.
- The `pane/` directory now exists as a destination for any genuinely
  non-paint helpers we ever want to extract — but the bar is "this is
  not in the per-frame render path AND has independent benchmark
  evidence of zero regression."

## Rules for `pane/core.rs` from here forward

1. **No mechanical refactors.** Don't extract functions "for cleanliness."
   Each extraction must have a benchmark gate.
2. **No design-system sweeps inside this file.** Token compliance, button
   migration, cursor helpers — the rest of the codebase gets those.
   `core.rs` keeps its current literals because regression risk > token
   purity.
3. **Performance-conscious owners only.** Edits to this file should be
   by someone who can profile a frame and confirm no regression. Not
   by parallel agents doing sweeps.
4. **Visual changes still allowed**, but they're the only allowed
   changes — color/spacing/layout tweaks the user explicitly requests,
   landed by a single owner who can verify in the running app.

## Multi-agent impact

The original plan envisioned agents F–J each owning a `pane/` sub-module
(indicators, drawings, orders, options). **Those agents don't exist.**

Agents A–E from the wider design-system plan (panel sweeps + widget
hardening + drawing tools) are unaffected — none of their territory
sits inside `pane/core.rs`:

| Agent | Owns | In pane/core.rs? |
|---|---|---|
| A — Trade panels | `panels/watchlist_panel`, `panels/plays_panel`, `panels/portfolio_pane`, `lists/cards/play_card` | No |
| B — Order panels | `panels/orders_panel`, `panels/signals_panel`, `panels/spreadsheet_pane`, `panels/scanner_panel`, `panels/settings_panel` | No |
| C — Analysis panels | `panels/indicators_panel`, `panels/object_tree`, `panels/script_panel`, `panels/news_panel`, `panels/discord_panel`, `panels/feed_panel`, `panels/connection_panel` | No |
| D — Widget hardening | `ui_kit/widgets/` | No |
| E — Drawing tools | `tools/drawing/`, `tools/order_entry_panel`, `tools/order_edit_dialog`, `tools/template_popup`, `tools/option_quick_picker`, `tools/indicator_editor` | No |

Chart paint stays one owner, one file, one decision-maker at a time.
That owner is whoever needs to land a specific visual change, with the
performance contract above.
