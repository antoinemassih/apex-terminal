# Automated Scenario Testing

The dev-inspector can run **user-story scenarios** against a live (or headless)
build of apex-terminal, assert invariants at every step, and emit a triage-ready
**bug report** listing every scenario that failed or crashed.

The goal: write *endless* user stories ("a trader rapidly switches symbols and
timeframes", "a trader stacks five indicators then flips to a quad layout",
"a trader opens the options overlay on an illiquid name"), run them all, and let
the harness surface the bugs for us to fix.

## The loop

1. **Launch a build with the inspector** (debug builds only — `#[cfg(debug_assertions)]`):

   ```bash
   # Headless (synthetic state machine — fast, CI-friendly, no GPU/window):
   ./src-tauri/target/debug/apex-native.exe --headless

   # Interactive (real window — widget/canvas/violation assertions are meaningful):
   ./src-tauri/target/debug/apex-native.exe
   ```

   IMPORTANT: run it from the **repo root**, not `src-tauri/` — `SCENARIO_DIR`
   (`dev/scenarios`) is resolved relative to CWD.

   The inspector serves on `http://127.0.0.1:7892`. Note: **supermodel** uses the
   same port — kill it first if `/health` answers with a foreign `{"error":...}`.

2. **Run the whole suite** (or a named subset):

   ```bash
   # everything:
   curl -s http://127.0.0.1:7892/scenario-list            # list available
   curl -s -X POST http://127.0.0.1:7892/run-suite \
        -H 'Content-Type: application/json' \
        -d '{"scenarios":["461_story_chart_invariants.json", "..."]}'
   ```

   `run-suite` returns `{total, passed, failed, bug_count, bug_report, results[]}`
   and writes **`dev/bug_report.md`** + **`dev/bug_report.json`**.

3. **Read `dev/bug_report.md`** — one checkbox entry per failing/crashing
   scenario with the exact failing step, the assertion delta (expected vs got),
   and the story tag. Reproduce any bug by re-running its named scenario file.

4. **Fix, re-run, repeat** until the report is clean.

## Writing a story scenario

Scenarios live in `dev/scenarios/*.json`. A story is just steps + assertions:

```json
{
  "name": "story_chart_invariants",
  "description": "A trader rapidly switches symbols, timeframes, indicators, layouts.",
  "story": "Chart/Invariants",
  "tags": ["story", "invariants", "chart", "smoke"],
  "steps": [
    { "action": "reset" },
    { "action": "wait_frames", "count": 3 },
    { "action": "cmd", "cmd": "SwapPaneSymbol", "args": { "pane": 0, "symbol": "QQQ" } },
    { "action": "assert", "assertions": [
      { "no_panic": true }, { "viewport_sane": true },
      { "canvas_all_finite": true }, { "fps_above": 5.0 }
    ] }
  ]
}
```

See `461_story_chart_invariants.json` for a full example.

## Invariant assertions (good defaults for any story)

These catch whole *classes* of bugs without needing to know the right answer —
ideal for fuzzing/endless stories:

| Assertion | Catches |
|-----------|---------|
| `no_panic` | any thread panic since the scenario started (Rust panic hook) |
| `viewport_sane` | collapsed/inverted viewport (`price_low >= price_high`, `px_per_bar <= 0`) |
| `canvas_all_finite` | NaN/Inf in bar OHLCV, screen coords, viewport, drawing anchors, indicator outputs |
| `fps_above: N` | render-loop stalls / runaway work |

Pair these with the targeted assertions (`active_symbol_equals`,
`canvas_indicator_value_in_range`, `widget_exists`, `no_active_violations`, …)
when a story has a specific expected outcome.

## How panic capture works

`dev_inspector::install_panic_hook()` (called from `init()`) chains the existing
panic hook and records `{message, location, thread}` into a global log. Each
`run_scenario` clears the log at start; `no_panic` reads it; the bug report
escalates any scenario with panics to **severity: crash**.

## Headless vs interactive — know the limit

Headless drives a *synthetic* state mirror: fast and deterministic, great for
invariant/smoke coverage, but `widget_tree` is empty and visual violations are
always cleared — it cannot find real layout/widget bugs. For those, run the
**interactive** build so assertions read the real egui frame.

## Comprehensive interactive suite (the real bug finder)

Driving the **interactive** build lets the harness do and see more than a user
can: inject hundreds of commands per minute and read the real widget tree (rects,
clip_rect, roles), the real canvas (bars with screen coords, indicators with
computed values, viewport), fps, and design-contract violations.

- `dev/gen_scenarios.py` — generates the re-runnable `5xx/6xx` suite: exhaustive
  sweeps (every symbol, timeframe, indicator, flag, theme, style, pane type),
  watchlist CRUD, alerts, multi-feature trader journeys, and adversarial stress.
  Edit + re-run it to extend coverage. Output uses only the verified vocab.
- `dev/SCENARIO_VOCAB.md` — the single source of truth for valid commands and
  assertions (interactive-verified). Read it before writing scenarios by hand.

Run it (from the repo root):
```bash
./src-tauri/target/debug/apex-native.exe &          # interactive window
# build the list of 5xx/6xx scenarios + post it:
curl -s -X POST http://127.0.0.1:7892/run-suite -H 'Content-Type: application/json' \
     -d "$(python -c 'import glob,os,json;print(json.dumps({"scenarios":[os.path.basename(f) for f in sorted(glob.glob("dev/scenarios/[56]*.json"))]}))')"
```

### Invariants that find real bugs without knowing the right answer
`no_panic`, `viewport_sane`, `canvas_all_finite` (render geometry only — indicator
last-values are intentionally excluded because some, e.g. Ichimoku chikou, are
legitimately NaN at the latest bars), `canvas_bars_monotonic`,
`canvas_bars_screen_ordered`, `canvas_bars_within_pane`, `canvas_bar_ohlc_valid`,
`fps_above`. Design/layout is a **global** invariant — assert `no_clipped_widgets`
/ `design_audit_clean` in ONE dedicated scenario (`572_design_audit_baseline`),
not on every functional story.

### Tips learned the hard way
- Run the binary from the **repo root** (SCENARIO_DIR is CWD-relative).
- Port 7892 collides with **supermodel** — kill it if `/health` looks foreign.
- After a symbol/timeframe change, the field updates only when the async load
  lands — settle ≥1.2 s before asserting an exact symbol/tf, or assert invariants
  only. The genuinely-rapid stress scenarios assert invariants, never exact state.
- A worker-thread panic (e.g. in indicator compute) can poison shared state and
  tear down the whole window — so one crash halts the suite; fix it and re-run.

See `dev/FINDINGS_2026-06-28.md` for the first run's results (3 bugs fixed,
2 open) and `dev/bug_report.md` for the live auto-generated failure list.

## Reliable full-corpus runner (contamination-filtered)

`python dev/run_corpus.py` runs every scenario (prefix ≥ 500) against a live build,
then **re-runs each failure in isolation** to classify it:
- passes in isolation → a back-to-back **contamination flake** (a previous
  scenario's late async load bled into the next; not a feature defect),
- fails in isolation → a **real** failure.

It writes `dev/bug_report.md` with only the real failures. This is the trustworthy
way to read the full corpus: back-to-back runs share one process and are
non-deterministically flaky at scenario boundaries, but every real correctness
signal survives the isolation re-run. (A fully deterministic back-to-back run would
need a monotonic load-generation guard on the bar-load path — deferred as it touches
business-critical loading.)
