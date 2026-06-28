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

## Test-harness isolation fix (NOT an app bug)

> Correction: an earlier version of this doc framed the indicator accumulation as
> a "bug found & fixed." That was wrong — it is a test-isolation gap in the
> harness, not defective app behaviour. Indicator persistence (save_workspace/load)
> is deliberate and untouched.

### Indicators accumulated across the test session — harness `reset` didn't clear them
The dev-inspector `reset` (`do_reset`, a debug-only test fixture, never called on
startup) restored symbol/timeframe/flags but did not clear indicators. Because
~250 scenarios each call `AddIndicator` into one long-running process, they piled
up on pane 0 (observed 87 → 166) with nothing cleaning up between tests. This is
**test pollution**, not the app's saved-state persistence — `do_reset` does not
load or save a workspace, it just pushes a few commands. Notably the app did NOT
crash under 166 indicators (good robustness).
**Harness fixes (for test isolation only):**
- New `AppCommand::ClearIndicators { pane }` (commands.rs) — removes all indicators
  on a pane and restarts id numbering (a cleared pane behaves like a fresh one).
  Only invoked by `do_reset`; not wired to startup, save, load, or any UI, so it
  does not affect real indicator persistence.
- `do_reset` now clears indicators on both panes so each scenario starts isolated.
- Side observation: `AddIndicator` sets `editing_indicator`, which is why the
  indicator-editor panel stays open after adding — relevant to the toolbar-clip
  finding.

## Open findings (see dev/bug_report.md)
1. **Toolbar clipping** — `workspace_btn` / `indicators_btn` / `widgets_btn` clipped
   a few px; `timeframe_picker` is a 24px touch target (< 28px min). Low severity.
2. **Timeframe under a rapid storm settles on an earlier tf** — *possibly
   by-design, needs a product call.* After many rapid tf changes the pane can show
   an earlier timeframe (its label appears to follow the data load rather than the
   last request). If "label waits for bars to arrive" is intended, this is expected
   behaviour, not a defect; single switches are fine at ≥1.2 s settle. Flagging for
   a decision rather than asserting it's a bug.
3. **`indicator_add_remove_churn` count** — brittle absolute-count assertion in one
   power-user scenario (off by one). Test limitation, not an app bug.

## Reproduce
Launch the interactive build from the repo root, then POST the scenario list to
`/run-suite` (see `dev/AUTOMATED_TESTING.md`). The corpus is every
`dev/scenarios/*.json` with a numeric prefix ≥ 500. Regenerate the parametric set
with `python dev/gen_scenarios.py`.
