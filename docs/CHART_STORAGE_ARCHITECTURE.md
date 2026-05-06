# Chart Storage Architecture

**Status:** Draft v0.1
**Owner:** Antoine Abdul-Massih
**Last updated:** 2026-05-05
**Related:** `XOL_FORMAT_SPEC.md`

## 1. Why this exists

A chart's "state" (symbol, viewport, drawings, annotations, indicator refs)
travels through four very different mediums in this product:

1. **Memory** — what the renderer touches every frame
2. **Cloud** — what we store in Postgres for cross-device sync
3. **Local disk** — what desktop users save/open as a project file
4. **Wire / file interchange** — what gets shared between users (XOL)

A single shared encoding is convenient but always wrong somewhere. JSON-zip
that's great for portability is awful for a 60fps redraw loop. A flatbuffer
that's great on disk can't be queried in SQL. So: **let each medium use the
representation that fits it best, and convert at the boundary.**

The cost is a codec layer. The benefit is that no medium pays for another
medium's constraints.

## 2. The four representations

| # | Medium      | Format                  | Optimized for                          | Lifetime     |
|---|-------------|-------------------------|----------------------------------------|--------------|
| 1 | Memory      | Native Rust structs     | zero-copy reads, mutation, rendering   | session      |
| 2 | Database    | Postgres normalized rows + binary columns | query, partial update, history | forever      |
| 3 | Local file  | Single binary file (`.apxchart`) | fast open, small, mmap-able   | forever      |
| 4 | Interchange | XOL (zip + JSON)        | portability, third-party readability   | per-share    |

Memory is the hub. Every other format converts to/from it.

```
       ┌────────┐                    ┌────────┐
       │  XOL   │                    │   DB   │
       │ (zip)  │                    │  rows  │
       └───┬────┘                    └───▲────┘
           │                              │
       xol_codec                       db_codec
           │                              │
           ▼                              │
       ┌────────────────────────────────────┐
       │     In-memory canonical model      │
       │     (apex-terminal::chart::state)  │
       └────────────────────────────────────┘
           ▲                              ▲
           │                              │
       native_codec                    (renderer reads
           │                             directly)
           │
       ┌───▼────┐
       │.apxchart│
       │ (binary)│
       └────────┘
```

## 3. Representation #1 — In-memory canonical model

The renderer reads from this every frame, so every design choice optimizes
for fast iteration and mutation, not serialization.

```rust
// apex-terminal/src/chart/state/mod.rs
pub struct ChartState {
    pub id: ChartId,                         // u64, not string
    pub symbol: Symbol,                       // arcstr::ArcStr — cheap clone
    pub timeframe: Timeframe,                 // enum, not string
    pub viewport: Viewport,
    pub drawings: SlotMap<DrawingId, Drawing>,
    pub annotations: SlotMap<AnnotationId, Annotation>,
    pub indicators: SmallVec<[IndicatorRef; 8]>,
    pub theme: ThemeOverrides,
    pub unknown_extensions: ExtensionBag,     // see §7
}

pub struct Drawing {
    pub kind: DrawingKind,                    // enum, not string
    pub points: SmallVec<[Point; 4]>,         // 4 inline covers most cases
    pub style: StyleId,                       // interned, not inlined
    pub flags: DrawingFlags,                  // bitflags: locked, visible, ...
    pub z: i16,
}

pub struct Point { pub ts_ns: i64, pub price: f32 }  // f32 is enough for screen
```

Key choices:
- **`SlotMap`** for drawings/annotations: stable IDs across deletes, O(1) lookup, dense iteration
- **Interned styles** in a `StyleTable`: most drawings reuse a few stroke/width combos
- **`SmallVec` for points**: trendlines have 2 points, fibs have 2, rectangles have 2 — inline storage avoids heap allocation for >95% of cases
- **u64 IDs**, not ULID strings, in the hot path
- **f32 prices**: native chart can't render to better than ~1px precision anyway; halves memory vs f64

The on-disk and on-wire formats can use ULID strings, f64 prices, etc. They
get converted at codec boundary.

## 4. Representation #2 — Database (Postgres)

Normalized schema, not a JSON blob, because we need to:
- Query "all charts using indicator X" (admin upgrade flow)
- Show user "your charts touching SPX between $4500–$4600"
- Partial-update a single drawing without rewriting the whole document
- Join to users / permissions / sharing tables
- Track revision history with reasonable storage cost

### Tables

```sql
CREATE TABLE charts (
    id              BIGINT PRIMARY KEY,
    user_id         BIGINT NOT NULL REFERENCES users(id),
    title           TEXT,
    symbol_canonical TEXT NOT NULL,
    asset_class     SMALLINT NOT NULL,
    timeframe       SMALLINT NOT NULL,
    theme           SMALLINT,
    viewport        BYTEA,                   -- packed: 4×i64 + 2×f32 + flags
    schema_version  INT NOT NULL,
    head_revision   BIGINT REFERENCES chart_revisions(id),
    created_at      TIMESTAMPTZ NOT NULL,
    updated_at      TIMESTAMPTZ NOT NULL
);
CREATE INDEX charts_user_symbol ON charts(user_id, symbol_canonical);

CREATE TABLE drawings (
    id              BIGINT PRIMARY KEY,
    chart_id        BIGINT NOT NULL REFERENCES charts(id) ON DELETE CASCADE,
    kind            SMALLINT NOT NULL,
    z               SMALLINT NOT NULL,
    flags           SMALLINT NOT NULL,
    style_id        BIGINT REFERENCES styles(id),
    points          BYTEA NOT NULL,          -- packed: see §4.1
    extras          JSONB                    -- kind-specific extras (fib levels, text body, …)
);
CREATE INDEX drawings_chart ON drawings(chart_id);

CREATE TABLE annotations (
    id              BIGINT PRIMARY KEY,
    chart_id        BIGINT NOT NULL REFERENCES charts(id) ON DELETE CASCADE,
    anchor_ts_ns    BIGINT NOT NULL,
    anchor_price    REAL NOT NULL,
    title           TEXT,
    body_md         TEXT,
    color           INT,
    asset_refs      TEXT[]
);
CREATE INDEX annotations_chart ON annotations(chart_id);

CREATE TABLE indicator_refs (
    id              BIGINT PRIMARY KEY,
    chart_id        BIGINT NOT NULL REFERENCES charts(id) ON DELETE CASCADE,
    ref_id          TEXT NOT NULL,
    ref_version     INT NOT NULL,
    param_schema_version INT NOT NULL,
    pane            SMALLINT NOT NULL,
    params          JSONB NOT NULL,
    style           JSONB
);
CREATE INDEX indicator_refs_ref ON indicator_refs(ref_id);

CREATE TABLE styles (
    id              BIGINT PRIMARY KEY,
    user_id         BIGINT NOT NULL REFERENCES users(id),
    stroke          INT NOT NULL,
    width_x100      SMALLINT NOT NULL,       -- 1.5 stored as 150
    dash            SMALLINT NOT NULL,
    fill            INT
);
-- styles deduplicated per user; same style across charts shares one row

CREATE TABLE chart_revisions (
    id              BIGINT PRIMARY KEY,
    chart_id        BIGINT NOT NULL REFERENCES charts(id),
    parent_id       BIGINT REFERENCES chart_revisions(id),
    author_id       BIGINT NOT NULL,
    diff            BYTEA,                   -- compact diff vs parent, not full snapshot
    full_snapshot_at_id BIGINT REFERENCES chart_snapshots(id), -- every Nth revision
    created_at      TIMESTAMPTZ NOT NULL
);
```

### 4.1 Packed `points` column

A trendline with 2 points stored as JSON is ~80 bytes. Packed it's 24 bytes
(2 × (i64 + f32) = 24). With delta encoding on timestamps and varints it
shrinks further. Across thousands of drawings per user this matters.

Format: tag byte for "encoding version", then either:
- v1: raw `[i64, f32]` pairs (for tiny point lists, ≤4 points)
- v2: count + delta-varint timestamps + raw f32 prices (for paths/polylines)

Decoder is in `db_codec::points::decode`.

### 4.2 Why `extras JSONB` for kind-specific fields

Drawings are heterogeneous: fib has `levels`, text has `body` and `font_size`,
pitchfork has 3 anchors. Two options:
- One table per kind → schema sprawl, painful migrations
- Wide nullable columns → ugly, doesn't scale to new kinds
- `extras JSONB` on a single `drawings` row → flexible, queryable with GIN, no migration to add a new kind

Picked option 3. The `kind` column is the discriminator; the codec validates
`extras` against the per-kind schema.

### 4.3 Revisions: diffs, not snapshots

Storing a full snapshot per save costs ~20× more than storing the diff. Strategy:
- Every save creates a `chart_revisions` row with a binary diff vs parent
- Every Nth revision (e.g., 50) also writes a full snapshot
- Restore = walk back to nearest snapshot, replay diffs forward
- Pruning: keep daily snapshots forever, hourly for the last week, every save for the last day

## 5. Representation #3 — Local file (`.apxchart`)

The desktop user's "Save" / "Open" file. **Not** XOL. XOL is for sharing.

Why a separate native format:
- 10–20× smaller than XOL zip — zstd over a packed binary beats deflate over JSON badly
- Opens in milliseconds, even with 10k drawings — mmap + lazy section parse, no JSON tokenization
- Can embed a price-data snapshot for offline replay (XOL ducks this in v1)
- Stable across desktop versions; XOL spec churn doesn't affect saved work

### File layout

```
[ magic: 8 bytes "APXCHRT\0" ]
[ version: u32 ]
[ header_len: u32 ]
[ header: flatbuffer ChartHeader { id, symbol, timeframe, viewport, theme, … } ]
[ section_table: array of { tag: u32, offset: u64, len: u64, codec: u8 } ]
[ section: drawings        (flatbuffer, zstd) ]
[ section: annotations     (flatbuffer, zstd) ]
[ section: indicators      (flatbuffer, zstd) ]
[ section: styles          (flatbuffer, zstd) ]
[ section: data_snapshot   (custom binary OHLC, zstd, optional) ]
[ section: thumbnail       (PNG, optional) ]
[ section: extensions      (CBOR, optional — unknown XOL fields preserved) ]
[ trailer: blake3 hash of everything above (32 bytes) + magic ]
```

### Why these choices

- **Magic + trailer hash**: detect truncation/corruption without parsing
- **Section table**: open file → read header + section table only → know where to find what without parsing megabytes
- **Flatbuffers per section**: zero-copy access; the renderer can read drawings directly out of the mmap'd buffer
- **Per-section zstd**: thumbnails/data don't compress further; drawings do
- **Lazy load**: `apxchart::open()` returns a handle. `.drawings()` decompresses + parses on demand. Charts with embedded price data don't pay the cost unless you ask for it.
- **No JSON anywhere** in the hot path

### Round-trip with cloud

Cloud → local:
- Server returns rows; backend or client builds the canonical model; `native_codec::write` produces `.apxchart`
- Optional: server can produce `.apxchart` directly for "Download" buttons

Local → cloud:
- `native_codec::read` → canonical model → `db_codec::upsert`
- All-or-nothing transaction; revision row created; head pointer flipped

## 5a. Templates are first-class, separate from charts

Apex Terminal's pane templates ("Day Trader", "Scalper", etc.) define a
reusable layout: which panes exist, how tall each is, and which indicators
populate each pane. Today indicators are managed *through* the template.

The canonical model treats them as orthogonal:

```rust
pub struct ChartState {
    // ...
    pub template_id: Option<TemplateId>,   // recipient's current template
    pub indicators: SmallVec<[IndicatorRef; 8]>,  // what's actually loaded
}

pub struct PaneTemplate {
    pub id: TemplateId,
    pub name: String,
    pub version: u32,
    pub panes: Vec<PaneSpec>,
    pub default_indicators: Vec<IndicatorRef>,  // what to load when applied
}
```

The chart owns its indicators independently of any template. A template can
be *applied* (which sets `indicators` and `template_id`), but the chart
remains valid if the template is later deleted — `template_id` just becomes
a dangling reference, harmless.

### DB schema additions

```sql
CREATE TABLE pane_templates (
    id              BIGINT PRIMARY KEY,
    user_id         BIGINT NOT NULL REFERENCES users(id),
    name            TEXT NOT NULL,
    version         INT NOT NULL,
    panes           JSONB NOT NULL,
    default_indicators JSONB NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL,
    updated_at      TIMESTAMPTZ NOT NULL,
    UNIQUE(user_id, name)
);

ALTER TABLE charts ADD COLUMN template_id BIGINT
    REFERENCES pane_templates(id) ON DELETE SET NULL;
```

`ON DELETE SET NULL` means deleting a template doesn't cascade-destroy
charts that referenced it.

### Import flow (three-tier)

When XOL is imported (see XOL spec §7a), one of three modes runs:

| Mode | What changes in DB |
|------|---------------------|
| Drawings only | Insert `charts` row + `drawings` + `annotations`. `template_id` left NULL. `indicators` empty. |
| Drawings + indicators | Above, plus `indicator_refs` rows from the XOL `indicators` block. Still `template_id = NULL`. |
| Apply full template | Above, plus a new `pane_templates` row from `template_hint`. `charts.template_id` set to it. |

### Edge case handling

- **Name collision on template apply.** Resolved by suffix: insert with
  `name = "<name> (n)"` where `n` is the smallest integer ≥ 2 making the
  unique-on-(user_id, name) constraint satisfied. The original template
  is never modified or replaced.
- **Missing indicators.** Each unresolved `ref_id` is recorded in a
  per-import `import_warnings` audit row (not a hard error). The
  `indicator_refs` row is still inserted with `installed_locally = false`
  flag; renderer shows the placeholder. When the user later installs that
  indicator, it picks up the saved params automatically.
- **Indicator newer than installed.** Same path as missing — placeholder +
  upgrade prompt; row preserved.
- **Indicator param invalid in current template.** Fall back to the
  indicator's own default for that field. Never inherit template-scoped
  params silently.
- **Template references a missing indicator.** Template applies; the slot
  shows a placeholder. Failing to apply over one missing indicator would
  surprise the user more than a placeholder does.

```sql
CREATE TABLE import_warnings (
    id              BIGINT PRIMARY KEY,
    chart_id        BIGINT NOT NULL REFERENCES charts(id) ON DELETE CASCADE,
    kind            SMALLINT NOT NULL,    -- 1=missing_indicator, 2=newer_indicator, ...
    ref_id          TEXT,
    detail          JSONB,
    created_at      TIMESTAMPTZ NOT NULL
);
```

### Native file (`.apxchart`)

Adds an optional `pane_template` section to the file layout (after
`indicators`). Holds an embedded `PaneTemplate` as flatbuffer + zstd. Same
three-tier semantics on local Open: a recovered local file may carry a
template hint that the user can choose to apply.

## 6. Representation #4 — XOL (interchange)

Already specified in `XOL_FORMAT_SPEC.md`. Summary of its role here:
- Used **only** at the boundary: Share, Export, Import, public links
- Never the runtime format, never the storage format
- Lossy-tolerant in both directions (see §7)

When a user clicks "Export as .xol", we run `xol_codec::write`. When they
double-click an `.xol` from another user, `xol_codec::read` runs and we
typically immediately also write a `.apxchart` cache so subsequent opens are fast.

## 7. The codec layer

Three codecs, each implementing the same trait:

```rust
pub trait ChartCodec {
    type Error;
    fn read(&self, src: &mut dyn Read) -> Result<ChartState, Self::Error>;
    fn write(&self, state: &ChartState, dst: &mut dyn Write) -> Result<(), Self::Error>;
}
```

Module layout:

```
apex-terminal/src/chart/
  state/                  representation #1 (canonical, in-memory)
    mod.rs
    drawings.rs
    annotations.rs
    indicators.rs
    style_table.rs
    extension_bag.rs       see "unknown extensions" below

  codec/
    mod.rs                 ChartCodec trait, common errors
    db.rs                  representation #2 ↔ canonical
    native.rs              representation #3 ↔ canonical
    xol.rs                 representation #4 ↔ canonical (delegates to xol crate)
    points_packing.rs      shared between db + native
```

### Unknown extensions

XOL is the only format that can be authored by third parties. A new XOL
writer might emit a drawing kind we don't yet know, or new fields on a known
kind. To preserve fidelity through round-trips:

- `ExtensionBag` on `ChartState` and on each `Drawing` holds raw CBOR-encoded
  data for unknown fields/kinds.
- `xol_codec::read` collects everything it can't map and stuffs it into the bag.
- `native_codec::write` and `db_codec::upsert` both serialize the bag as-is.
- `xol_codec::write` re-emits the bag's contents alongside known fields.
- Renderer ignores extensions for kinds it doesn't know but keeps them for save.

This is what makes a workflow like:
> User A (newer build) shares chart → User B (older build) opens, edits a
> drawing, re-shares → User A reopens
…not lose any data User B's build didn't understand.

### Lossy edges (acceptable)

- **DB → canonical**: we may upgrade old `schema_version` rows on read; lossless modulo upgrades
- **Canonical → DB**: lossless
- **Canonical ↔ native**: lossless by construction (native format is a 1:1 encoding of canonical)
- **Canonical → XOL**: lossless for everything XOL knows about; extensions written verbatim
- **XOL → canonical**: lossless modulo unknown kinds (preserved in bag)

## 8. Migration & versioning

Each representation has its own version axis:

| Layer       | Version field          | Migration runs in        |
|-------------|------------------------|--------------------------|
| Canonical   | (no version — code is the schema) | n/a |
| DB          | `schema_version` per row | DB migration scripts (Diesel/sqlx) |
| Native file | `version: u32` in header | `native_codec::migrations` |
| XOL         | `manifest.schema_version` | `xol::migrate` |

When canonical model changes:
- Add `From`/`TryFrom` impls between old and new shapes
- Each codec's read path produces canonical; codecs handle their own legacy formats internally
- Writing always emits the latest version of each format

Renderer never sees old shapes.

## 9. Performance targets

| Operation                                         | Target     |
|---------------------------------------------------|------------|
| Open `.apxchart` with 1k drawings                 | < 30 ms    |
| Open `.apxchart` with 10k drawings                | < 200 ms   |
| Cloud load chart by ID (cold cache, server side)  | < 50 ms    |
| Cloud save (autosave, partial)                    | < 100 ms   |
| Drawing mutation → render                          | < 1 ms     |
| Round-trip canonical → XOL → canonical (1k drawings) | < 500 ms |
| `.apxchart` size, 1k drawings, no data snapshot   | < 50 KB    |
| Equivalent XOL file                                | < 500 KB (10× larger is fine — different goal) |

If we miss any of these, fix the format choice for that medium, not the
canonical model.

## 10. Implementation phases

Phase order is dictated by what's load-bearing for users today:

### Phase 1 — Canonical model (no I/O changes yet)
- Build `ChartState` and friends.
- Refactor existing renderer to read from `ChartState` instead of whatever it currently reads.
- No format work yet; this is just the in-memory hub.

### Phase 2 — Native file (`.apxchart`)
- Implement `native_codec` end-to-end: read, write, mmap-friendly section access.
- Wire `File → Open` and `File → Save` in the desktop app.
- This unlocks the "save your work locally" UX without depending on cloud.

### Phase 3 — DB layer (cloud save)
- Migrations, codec, autosave loop, revision diffs.
- Cloud charts list UI.
- Now there's "your charts" across devices.

### Phase 4 — XOL codec
- Implement `xol_codec` against the spec doc.
- Hook up `File → Export as .xol` and drag-and-drop import.
- This is the user-facing sharing primitive.

### Phase 5 — Social + streaming layer (deferred)

**Not built yet.** Local file Save/Open via the system picker is the only
sharing path in v1. The full vision is in `XOL_FORMAT_SPEC.md` §12 — three
modes in increasing scope:

1. **Direct shares.** Send a chart to another user (inbox-style).
2. **Follow + browse.** Social graph; see what people you follow are charting; web XOL viewer for non-users.
3. **Live streaming.** Watch someone trade in real time. Like a game stream, but the medium is the chart. XOL deltas over WebSocket/WebRTC SFU, time-travel scrubbing, per-delta signatures.

**Why deferred:** none of this needs format changes — the wire format
already has stable per-primitive ids, anchor-based points, an
`ExtensionBag` for live-only metadata, and a signature slot. What's
missing is server work: account system, inbox, social graph, public
profiles, web viewer (JS/Wasm renderer), and the delta-streaming protocol
+ SFU.

When that work starts, the storage architecture from this doc is the
desktop side. The server will use its own datastore optimized for inbox /
feed / live streams; the desktop side just produces and consumes XOL.

### Phase 6 — Polish
- Embedded price-data snapshot in `.apxchart` for offline replay.
- Indicator registry / install prompts when XOL references unknown indicators.
- P2P share (deferred from XOL spec v2).

Each phase is independently shippable. Phase 1+2 alone is a usable product
("local-only desktop chart"). Phase 3 makes it cloud. Phase 4 makes it shareable.

## 11. Open questions

- **Data snapshot in `.apxchart`**: how large can it grow? If a user saves a
  1y intraday chart with 60M ticks, the file balloons. Options: cap at the
  visible viewport ± padding, or store aggregated bars only. Lean toward
  aggregated bars + viewport ticks.
- **Style table scope**: per-user or per-chart? Per-user dedupes harder but
  complicates "import an XOL" (which doesn't know your style table). Lean
  toward per-chart inline + a periodic dedup job.
- **Revisions retention**: keep forever vs prune at 1y? Storage matters at
  scale. Probably tier by user plan.
- **Single `chart_id` vs UUID**: u64 in DB is faster to index but UUIDs are
  needed for offline-created charts that haven't synced. Likely use UUID
  (v7 for time-ordering) and accept the index size.

## 12. References

- `XOL_FORMAT_SPEC.md` — interchange format
- `apex-terminal/src/chart/drawings/` — current drawing engine (will become
  the renderer, fed by `ChartState`)
- Outline doc: https://wiki.xllio.com/doc/xol-apex-open-layout-format-spec-v01-owoxQhzabF
