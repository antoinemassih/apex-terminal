# Command-Bus Migration (WS-E E1)

Status: **ratchet active**. Baseline `ui_direct_mutation = 534` (see
`dev/quality_baseline.json`), enforced fail-on-increase by `dev/quality_gate.py`
in the `quality-gates.yml` CI job. (Started at 536; the `SetDensityOverride`
reference migration below took the first −2.)

## Why

The AppCommand bus (`chart/renderer/commands.rs`) is a well-designed central
reducer — 100 documented variants, a `push()` queue drained once per frame by
`drain_and_dispatch(panes, watchlist)`, and a `request_gen` staleness guard. But
it is **~3% adopted**: UI code overwhelmingly mutates the `Chart` / `Watchlist`
god-objects directly (`watchlist.foo = x`) instead of dispatching. The reducer is
mostly decorative.

This blocks the architecture work: **you cannot safely split a 258-field
`Watchlist` (E3) while 536 UI sites mutate its fields directly.** Routing
mutations through the bus first gives one choke point, which is the precondition
for the state decomposition. The bus is also where undo/redo and staleness
guards live, so adoption pays off beyond the split.

## The ratchet

`ui_direct_mutation` counts `watchlist.* = ` / `wl.* = ` / `chart.* = `
assignments in `chart/renderer/ui/` + `gpu.rs`. **It may only ever decrease.**
Adding a new direct mutation fails CI; migrating one to the bus lowers it. After
a reduction, run `python dev/quality_gate.py --update` to lock the gain in.

This is a ratchet, not a hard ban, because the migration is incremental and
threaded through E2–E4 (state moves anyway when the structs are extracted). The
number trending to 0 *is* the adoption metric.

## The pattern (canonical exemplar: `SetChartFlag`)

The reference implementation already lives in-tree —
`ui/components/toolbar/chart_controls.rs` dispatches instead of mutating:

```rust
// BEFORE (direct mutation of the god-object):
chart.show_volume = new_value;

// AFTER (dispatch through the bus):
commands::push(AppCommand::SetChartFlag { pane: ap, flag: ChartFlag::ShowVolume, value: nv });
```

Three steps to migrate a mutation:

1. **Add a variant** to `enum AppCommand` (`commands.rs`). Prefer a *generic*
   variant with a `kind` enum over one-variant-per-field when the fields are a
   family (see `SetChartFlag { flag: ChartFlag, .. }`). A single field-set uses a
   named-struct variant like `SetAccountRisk { account, risk_pct }`.

2. **Add a reducer arm** in `fn dispatch(panes, watchlist, cmd)` (`commands.rs`,
   the `match cmd`). Do the field write here — plus any side-effect the UI used
   to do inline (e.g. the settings overrides also call
   `style::set_density_override(...)`; that call moves into the arm so the whole
   mutation is atomic in the reducer):

   ```rust
   AppCommand::SetAccountRisk { account, risk_pct } => {
       watchlist.account_size = account.max(0.0);
       watchlist.risk_pct = risk_pct.clamp(0.0, 1.0);
   }
   ```

3. **Replace the UI mutation** with `commands::push(AppCommand::X { .. })`.

## Timing semantics — READ THIS

`push()` **queues**; the mutation applies when `drain_and_dispatch` runs later in
the frame, not at the call site. Two consequences:

- **Safe to migrate:** click handlers that set a value read next frame (panel
  toggles, settings, order-edit commits). The 1-frame delay is imperceptible.
- **Do NOT migrate (yet):** a mutation whose *new value is read again in the same
  render pass* (e.g. compute-then-immediately-render within one closure). Those
  need the read site restructured first, or they belong to a later phase. When in
  doubt, leave it and let the E3 struct extraction carry it.

## What NOT to route through the bus

- Transient per-frame UI scratch state that never outlives the frame (hover,
  drag-in-progress pixel deltas) — not domain state, no benefit.
- Reads. The bus is for mutations only.

## Sequencing

Migrate opportunistically as each file is touched by E2 (gpu.rs extraction) and
E3 (Watchlist split). Do not attempt a 536-site big-bang — that reintroduces the
exact dual-state risk the audit flagged. Each PR lowers the ratchet; the number
is the scoreboard.
