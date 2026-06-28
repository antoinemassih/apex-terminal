# Full-corpus automated-testing run — 2026-06-28

Scaled the harness to a large, varied corpus and ran every scenario one-by-one
against the **real interactive window**.

## Corpus (252 interactive scenarios)
- **Parametric generator** (`dev/gen_scenarios.py`): exhaustive sweeps — per-symbol
  load + sanity, per-symbol VWAP+RSI studies, timeframe sweeps, every indicator,
  display-flag toggles + pair-combinations, themes/styles, pane types, watchlist
  CRUD, alerts, multi-pane (pane-1) independence, RSI-on-each-symbol, adversarial
  stress, negative tests.
- **Real-world narratives** (6 background agents, ~120 scenarios): day-trading
  (660–679), swing/position (680–699), options workflows (700–719), scanning &
  watchlist (720–739), charting/TA (740–759), power-user/edge/recovery (760–779).

## Result: 249 / 252 passed
Run in 9 chunks (≈30/chunk) against one live build; the app survived the entire
corpus (no crashes). The 3 open findings are in `dev/bug_report.md`.

## Bugs found & fixed in this run

### Indicators accumulate across sessions — `reset` never cleared them
The dev-inspector `reset` (`do_reset`) restored symbol/timeframe/flags but never
cleared indicators, so across a long shared session they piled up on pane 0
(observed 87 → 166). This broke per-pane indicator-count assertions and degraded
the chart. The app did NOT crash under 166 indicators (good robustness), but the
emergent accumulation is exactly the kind of cross-session state a single scenario
never reveals — it only showed up running 250 scenarios back-to-back.
**Fixes:**
- New `AppCommand::ClearIndicators { pane }` (commands.rs) — removes all indicators
  on a pane and restarts id numbering (a cleared pane behaves like a fresh one).
- `do_reset` now clears indicators on both panes for true test isolation.
- Side note surfaced: `AddIndicator` sets `editing_indicator`, which is why the
  indicator-editor panel stays open after adding — relevant to the toolbar-clip
  finding.

## Open findings (see dev/bug_report.md)
1. **Toolbar clipping** — `workspace_btn` / `indicators_btn` / `widgets_btn` clipped
   a few px; `timeframe_picker` is a 24px touch target (< 28px min). Low severity.
2. **Timeframe convergence under a rapid storm** — after many rapid tf changes, the
   pane can settle on an earlier tf (slow async load wins). Single switches are
   fine. Medium severity; fix touches async load coordination.
3. **`indicator_add_remove_churn` count** — brittle absolute-count assertion in one
   power-user scenario (off by one). Test limitation, not an app bug.

## Reproduce
Launch the interactive build from the repo root, then POST the scenario list to
`/run-suite` (see `dev/AUTOMATED_TESTING.md`). The corpus is every
`dev/scenarios/*.json` with a numeric prefix ≥ 500. Regenerate the parametric set
with `python dev/gen_scenarios.py`.
