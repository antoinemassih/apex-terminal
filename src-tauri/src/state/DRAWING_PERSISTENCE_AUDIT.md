# Drawing Persistence Audit (Wave 5)

Read-only audit of the three places drawings live and the sync gaps
between them. **This wave does not consolidate anything.** It documents
the current state so a follow-up wave can act on it.

---

## The three stores

### 1. In-memory `SlotMap` per chart

- **Where:** `chart::renderer::Chart` — each pane owns a
  `SlotMap<DrawingId, Drawing>`-shaped store of in-flight drawings
  (search `gpu.rs` for the `drawings:` field on `Chart`).
- **Lifetime:** dropped when the pane is closed; persists across frames
  but not across app restarts on its own.
- **Authority:** treated as the in-frame source of truth — render &
  hit-testing read from here exclusively.

### 2. Postgres via `crate::drawing_db`

- **Where:** `src-tauri/src/persistence/drawing_db.rs`
  - `init(pool)` — wires the singleton on app start; only fires if the
    PG pool came up successfully (see `lib.rs` `setup` closure).
  - `load_symbol(symbol) -> Vec<DbDrawing>` — blocking, 5 s timeout.
  - `save(&DbDrawing)` — fire-and-forget over an mpsc channel to a
    background worker thread.
  - `remove(id)` — fire-and-forget.
  - `load_groups()` / `save_group(...)` / `remove_group(id)` — same
    fire-and-forget pattern for drawing-color-groups.
- **When it fires:**
  - On every mutation through the property bar (8 sites in
    `chart/renderer/ui/tools/drawing/properties_bar.rs`).
  - On every mutation through the Object Tree
    (`chart/renderer/ui/panels/object_tree.rs` — ~15 sites).
  - On *symbol load* (`chart/renderer/io/fetch.rs:968`) to repopulate
    the in-memory SlotMap for the newly-active symbol.
- **Failure mode:** silent. The worker logs errors via tracing but the
  caller has already returned.

### 3. `.xol` files (XOL codec, the on-disk archive format)

- **Where:** `src-tauri/src/chart/state/codec/xol.rs` +
  `src-tauri/src/chart/state/file_io.rs` (the `save_xol_to_path` /
  `load_xol_from_path` public surface). Drawings serialize through
  `drawings_to_json` into a `drawings.json` entry inside the ZIP-shaped
  `.xol` archive. (Note: the legacy `.apxchart` extension is gone — the
  file_io module is explicit that `.apxchart` is deferred and only `.xol`
  is wired today.)
- **When it fires:** *only* when the user runs `File → Export chart` or
  the matching Tauri command (`save_chart_dialog`,
  `commands::export_chart_xol`, `commands::save_chart_to_file`). No
  automatic save on quit, no per-mutation write.
- **Authority on load:** when a `.xol` is imported, the in-memory
  drawings list is replaced wholesale; Postgres is *not* consulted, and
  the imported drawings are *not* written back to Postgres unless the
  user subsequently edits them (which then triggers `drawing_db::save`).

---

## Sync gaps observed

### Gap A — Postgres write failure is invisible

`drawing_db::save` returns immediately; the actual SQL write happens on
a worker thread and only logs to stderr on failure. The UI thinks the
drawing was persisted. After restart (which repopulates from PG), the
drawing is silently missing. The SlotMap was the only winner.

**Severity:** medium. PG outages are rare but recovery requires the
user to redraw or re-import a `.xol`.

### Gap B — `.xol` import does not write through to Postgres

Importing a `.xol` populates only the SlotMap. The PG row for that
drawing id (if any from a prior session) stays orphaned, and the
freshly-imported drawings vanish on next symbol switch (which reloads
from PG). The user perceives this as "my imported drawings disappeared
when I switched symbol and came back."

**Severity:** high — the failure is silent and the user has no recourse
without re-importing.

### Gap C — Postgres load can race with first-frame render

`load_symbol` is blocking with a 5 s timeout (`drawing_db.rs:97`). If
the PG worker is busy or PG is unreachable, the first frame after a
symbol switch renders zero drawings while the timeout elapses. There's
no "loading drawings…" indicator and no retry.

**Severity:** low — the indicator gap is mostly cosmetic, but the lack
of retry means a single timeout permanently strips drawings until the
next symbol-load fetch.

### Gap D — No transactional grouping between drawing + group

`save_group` and `save` are independent fire-and-forget messages on
different ops in the same channel. If the group write fails but the
drawing referencing it succeeds (or vice versa), the in-memory state
references a non-existent group id. No reconciliation runs on app
start.

**Severity:** low — groups are mostly cosmetic (color tinting), but
"drawing references missing group" is the kind of orphan that snowballs
over months.

### Gap E — `.xol`'s `drawings.json` has no schema version of its own

The XOL archive has a top-level `schema_version` (see
`xol.rs:SCHEMA_VERSION` checks), but the embedded `drawings.json`
relies on the archive version to know its shape. If we ever change
*just* the drawing payload shape, we must bump the whole archive
version, breaking backward compatibility for unrelated entries.

**Severity:** low until the first drawing-shape change.

---

## Recommended consolidation strategy

Postgres should win. The XOL archive is for export/import (sharing,
backup); the SlotMap is for the frame. Concretely:

1. **Postgres becomes the only authoritative store.** SlotMap is a
   read-through cache populated on symbol load. `.xol` export writes
   a snapshot from PG, `.xol` import writes through to PG and then
   re-populates the SlotMap from PG.

2. **All `drawing_db::save` calls flip to `save_with_ack`**, where the
   reply channel tells the UI whether the write made it to PG. The UI
   shows a toast on persistent failure and queues the write for retry.

3. **Add a `drawings.schema_version: u32`** inside the `drawings.json`
   blob in `.xol`, independent of the archive's outer
   `SCHEMA_VERSION`. The migration hook on read can normalize older
   shapes without forcing an archive-wide bump.

4. **Reconcile groups + drawings on app start.** Walk the PG drawings
   list, drop any group reference that doesn't resolve, demote to
   `default` group, log to stderr (or surface via the diagnostics
   panel) so the user knows.

5. **Replace the blocking 5 s `recv_timeout` on `load_symbol` with
   an async load + spinner.** Use the new `InFlightRegistry`
   (`state::inflight::InFlightKind::Other("drawings_load")` or a
   new variant) so the UI can show a placeholder and retry.

None of these land in Wave 5. They are the work this audit unblocks.
