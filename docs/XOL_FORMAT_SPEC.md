# XOL — Apex Open Layout Format

**Status:** Draft v0.1
**Owner:** Antoine Abdul-Massih
**Last updated:** 2026-05-05

## 1. Purpose

`.xol` is an open, portable file format for sharing chart state — drawings,
annotations, indicator references, and viewport — between Apex Terminal users.
Apex Terminal is the reference viewer/editor; the format itself is documented
and unencumbered so third parties can read and write it.

Goals:
- Round-trip fidelity: `Open → Save` produces a byte-identical file when nothing changed.
- Timeframe-independent: a `.xol` saved on 5m opens correctly on 1m or 1h.
- Safe by default: no executable code embedded; missing indicators degrade gracefully.
- Two storage paths, one schema: cloud save and local export use the same payload.
- Human-debuggable: JSON inside a zip, not a custom binary.

Non-goals (v1):
- Bundling indicator source code or compiled WASM.
- Encrypted P2P transport (deferred to v2).
- Backwards-compat with TradingView `.tv`, NinjaTrader workspaces, etc.

## 2. Container

A `.xol` file is a ZIP archive (PKZip, deflate) with the following layout:

```
manifest.json          required — schema version, author, app version
chart.json             required — symbol, timeframe, viewport, theme overrides
drawings.json          optional — drawing primitives
annotations.json       optional — text notes, callouts, attachments
indicators.json        optional — indicator references with params
assets/                optional — embedded images (PNG/JPEG, ≤2MB each)
  {uuid}.png
signature              optional — ed25519 signature of manifest.json
```

All JSON files are UTF-8, no BOM, LF line endings, sorted keys for diff stability.

Hard limits (validated on load):
- Total uncompressed size ≤ 50 MB
- Drawing count ≤ 10,000
- Annotation count ≤ 2,000
- Indicator count ≤ 64
- Asset count ≤ 32

Files exceeding limits are rejected with a structured error.

## 3. manifest.json

```json
{
  "schema_version": 1,
  "format": "xol",
  "app": {
    "name": "apex-terminal",
    "version": "0.x.y",
    "platform": "win32"
  },
  "author": {
    "id": "user_abc123",
    "display_name": "Antoine",
    "public_key": "ed25519:..."
  },
  "created_at": "2026-05-05T14:30:00Z",
  "modified_at": "2026-05-05T14:35:00Z",
  "title": "SPX gamma flip 2026-05-05",
  "description": "Optional free-text description",
  "tags": ["spx", "gamma", "0dte"],
  "files": {
    "chart.json":       { "sha256": "...", "size": 1234 },
    "drawings.json":    { "sha256": "...", "size": 5678 },
    "indicators.json":  { "sha256": "...", "size": 234 }
  }
}
```

`schema_version` is a single integer that bumps on any breaking change.
Migrations live in `apex-terminal/src/xol/migrations/`. Readers MUST refuse
files with `schema_version` newer than they support.

The `files` map is a content manifest: each referenced file's sha256 and
uncompressed size are recorded. On load, every entry is verified before parse.
This catches corruption and tamper-after-sign.

## 4. chart.json

```json
{
  "symbol": {
    "canonical": "SPX",
    "provider_hints": {
      "polygon": "I:SPX",
      "ib": "SPX",
      "tv": "SP:SPX"
    },
    "asset_class": "index"
  },
  "timeframe": "5m",
  "viewport": {
    "from_ts_ns": 1714900000000000000,
    "to_ts_ns":   1714920000000000000,
    "price_low":  4500.0,
    "price_high": 4600.0,
    "log_scale":  false
  },
  "theme": "dark",
  "overlays": ["session_bands", "vwap"]
}
```

`symbol.canonical` is the format's neutral name. `provider_hints` lets the
loading client map to its own data source. Asset class drives default behaviors
(e.g., session boundaries).

## 5. drawings.json

Anchoring rule: every drawing point is `{ ts_ns, price }`. Never bar index,
never pixels. This is what makes drawings survive timeframe and zoom changes.

```json
{
  "drawings": [
    {
      "id": "01J9Z...",          // ULID
      "kind": "trendline",
      "z": 100,
      "locked": false,
      "visible": true,
      "style": {
        "stroke": "#FFB800",
        "width": 1.5,
        "dash": "solid"
      },
      "points": [
        { "ts_ns": 1714900000000000000, "price": 4520.5 },
        { "ts_ns": 1714905000000000000, "price": 4555.0 }
      ],
      "extend_left": false,
      "extend_right": true
    },
    {
      "id": "01J9Z...",
      "kind": "fib_retracement",
      "points": [...],
      "levels": [0.0, 0.236, 0.382, 0.5, 0.618, 0.786, 1.0],
      "show_labels": true
    },
    {
      "id": "01J9Z...",
      "kind": "rect",
      "points": [...],
      "fill": "#FFB80022",
      "stroke": "#FFB800"
    },
    {
      "id": "01J9Z...",
      "kind": "text",
      "anchor": { "ts_ns": ..., "price": ... },
      "text": "gamma flip",
      "font_size": 12
    }
  ]
}
```

Drawing kinds in v1: `trendline`, `horizontal_line`, `vertical_line`,
`ray`, `rect`, `ellipse`, `fib_retracement`, `fib_extension`, `pitchfork`,
`text`, `arrow`, `polyline`, `path` (freehand).

Unknown `kind` → loader skips with warning, file still loads.

## 6. annotations.json

Annotations are first-class commentary, separate from drawings:

```json
{
  "annotations": [
    {
      "id": "01J9Z...",
      "anchor": { "ts_ns": ..., "price": ... },
      "title": "Earnings",
      "body_md": "Beat by 3¢, guide raised",
      "color": "#22C55E",
      "asset_refs": ["assets/01J9Z...png"]
    }
  ]
}
```

`body_md` is restricted Markdown — no HTML, no scripts, no remote images
(only `assets/...` refs). Renderer sanitizes on load.

## 7. indicators.json

Reference-only. No code, no params blob from arbitrary sources.

```json
{
  "indicators": [
    {
      "ref_id": "apex.vwap",
      "ref_version": 2,
      "param_schema_version": 1,
      "pane": "main",
      "params": {
        "session": "rth",
        "anchor": "session_open",
        "show_bands": true,
        "band_stddevs": [1.0, 2.0]
      },
      "style": {
        "line": "#3B82F6",
        "bands": "#3B82F633"
      }
    },
    {
      "ref_id": "thirdparty.acme.gex",
      "ref_version": 5,
      "param_schema_version": 3,
      "pane": "subpane_1",
      "params": { "...": "..." }
    }
  ]
}
```

Resolution on load:
1. Look up `ref_id` in local indicator registry.
2. If missing → render a "missing indicator" placeholder in `pane` with the
   ref_id, version, and an install link/CTA. Other indicators still load.
3. If present but `param_schema_version` is older → run param migrations
   shipped with the indicator. If newer than installed → placeholder.

Each indicator owns its own `param_schema_version`. The XOL schema_version
does not gate indicator param compatibility — they evolve independently.

## 7a. template_hint (optional)

Apex Terminal organizes indicators into reusable **pane templates** (e.g.,
"Day Trader" = main + volume + MACD). When a user shares a chart they may
also want to share their template, but recipients don't always want their
workspace overwritten. XOL separates the two:

- `indicators` (§7) is **always** the source of truth for what's loaded.
- `template_hint` is an **optional** block describing the sender's template
  for recipients who want to adopt it.

```json
"template_hint": {
  "name": "Antoine's Day Trader",
  "version": 3,
  "panes": [
    { "id": "main",      "height_pct": 70, "indicators": ["apex.vwap"] },
    { "id": "subpane_1", "height_pct": 15, "indicators": ["apex.volume"] },
    { "id": "subpane_2", "height_pct": 15, "indicators": ["apex.macd"] }
  ]
}
```

### Three-tier import

On import, the receiving client offers the user three modes:

| Mode | What lands | Default? |
|------|------------|----------|
| **Drawings only** | Trendlines, fibs, annotations. Indicators and template ignored. | Yes |
| **Drawings + indicators** | Above, plus the listed indicators are loaded into the recipient's *current* template's panes (best-fit). | No |
| **Apply full template** | Replaces the recipient's current pane template with the one in `template_hint`. | No |

Drawings-only is the safe default — the recipient's workspace is never
mutated unless they explicitly opt in.

### Edge cases

- **Same-named template already exists locally.** The imported template is
  saved with a suffix: `"Antoine's Day Trader" → "Antoine's Day Trader (2)"`.
  If `(2)` exists, increment to `(3)`, and so on. Never overwrite.
- **Missing indicators on the recipient.** Don't fail. Each unresolved
  `ref_id` is added to a `missing_indicators` list shown in the import
  dialog with an "install" CTA per item. The chart still loads; missing
  indicator panes show a placeholder (per §7 resolution rules).
- **Indicator present but `param_schema_version` is newer than installed.**
  Treat as missing — placeholder + upgrade prompt.
- **Indicator references a param valid only inside the sender's template.**
  Fall back to the indicator's own default for that param. Never inherit
  template-scoped params silently.
- **`template_hint` references an indicator the recipient doesn't have.**
  Template still applies (panes laid out as specified); the missing slot
  shows a placeholder. Don't refuse to apply the template just because
  one indicator is missing.

The import dialog summarizes everything before the user confirms:

```
Import "SPX gamma flip 2026-05-05.xol"

What to import:
  ☑ 47 drawings, 3 annotations           [drawings only — recommended]
  ☐ 2 indicators (apex.vwap, apex.macd)
  ☐ Pane template "Antoine's Day Trader" → will be saved as "Antoine's Day Trader (2)"

Missing on this machine (won't block import):
  ⚠ thirdparty.acme.gex                  [install]

                                          [Cancel]  [Import]
```

## 8. signature (optional)

`signature` is a 64-byte ed25519 signature over the canonical bytes of
`manifest.json`. The author's `public_key` is in the manifest. Verification:

1. Compute sha256 of every listed file; compare to manifest.
2. Verify ed25519(manifest.json, signature, author.public_key).
3. UI shows "verified author" badge only when both pass.

Files without signature are accepted, just unsigned.

## 9. Symbol canonicalization

Cross-broker symbol mismatch is the #1 source of "shared file looks wrong"
reports in competing tools. XOL stores:
- `canonical`: a neutral name picked by Apex (e.g., `SPX`, not `^SPX`)
- `provider_hints`: optional map of `{provider: native_symbol}`
- `asset_class`: `equity | etf | index | option | future | crypto | fx`

For options, canonical follows OCC: `SPXW   260516P04500000`.

The receiving client picks the best symbol for its active data provider, with
canonical as fallback.

## 10. PII / safety

Never write to a `.xol` by default:
- Account numbers, broker IDs, position sizes, P&L, order history
- Watchlist contents beyond the active symbol
- Any session token, API key, or auth material
- Local file paths, machine names

Export dialog has an "Include private data" toggle (off by default) that
unlocks position/P&L annotations in a separate `private.json` file. Recipients
see a clear warning when loading a file with private data.

## 11. Storage paths

**Cloud save** (default, autosave):
- POST `/api/charts` with the same JSON payload
- Server stores: `{user_id, chart_id, manifest_json, blobs[], created_at, updated_at}`
- Versioned: each save bumps a version row; user can restore prior versions

**Local export** (`File → Export as .xol`):
- User picks path; we write the zip
- No server round-trip required

**Cloud → Local** and **Local → Cloud** must be lossless round-trips.

## 12. Sharing — vision (deferred)

**Status: not built. Local file Save/Open is the only sharing path in v1.**
Captured here so future work can build to it without changing the format.

The endgame for Apex Terminal is a social/streaming layer, not just a "send
this chart" feature. Three modes, in increasing engineering cost:

### 12.1 Direct shares — "send chart to user"

Send a chart to another Apex Terminal user the way you DM someone in Discord:
- Recipient's app shows an inbox notification.
- Click → chart opens in their workspace (with the standard three-tier
  import dialog — drawings only by default, opt-in for indicators/template).
- Same XOL bytes on the wire as the local file format. Server is a
  store-and-forward relay; clients do the encoding/decoding.

### 12.2 Follow + browse — "see what they're charting"

Social graph on top of accounts:
- Follow other users; their published charts appear in a feed.
- Open any followed user's chart on demand. Same XOL bytes, fetched from
  their published list rather than a one-shot share.
- Public profile pages that list a user's charts. Optional discoverability.
- Web-based XOL viewer (JS/Wasm) so people without Apex Terminal can still
  open a shared chart in a browser, read-only.

### 12.3 Live streaming — "watch them trade"

Like a game stream, but the medium is a chart. The streamer keeps trading
and viewers see each tick, drawing, and decision in real time.

- One streamer (authoritative). Many viewers (read-only).
- Initial state: full XOL snapshot, fetched on join.
- Live deltas: drawings added/edited/deleted, viewport pan/zoom, indicator
  param changes, optionally bar updates if the streamer's data feed is the
  authoritative source. Wire protocol is XOL deltas (additions, mutations,
  deletions keyed by `id`), not full retransmits.
- Latency budget: < 200 ms streamer → viewer for "feels live" feel.
  WebSocket fan-out or WebRTC SFU; viewers don't connect P2P to streamer to
  cap streamer's upload.
- Time-travel: viewers can pause and scrub back through recent deltas
  (like a stream replay). Requires a server-side ring buffer of the last N
  deltas per stream.
- Privacy: streamer chooses what's visible. P&L, account size, position
  size are *never* in the live feed unless explicitly opted in (per §10).

### What this implies for the format

- **`id` per drawing/annotation/indicator must be stable across streamer
  edits** so viewers can match deltas to existing entities. Current spec
  uses ULIDs — already correct.
- **Drawings must be re-anchorable independently.** Already true (`ts_ns` +
  `price` per point). A streamer dragging a trendline produces a delta with
  just the new points.
- **`ExtensionBag` becomes the carrier for live-only metadata** (e.g.,
  per-delta sequence numbers, streamer's local timestamp) without bloating
  the static format.
- **Signatures (§8) extend to deltas.** A live stream is signed by the
  streamer's public key; viewers verify the signature on each delta to be
  sure they're seeing the real streamer and not an injected message.
- **Web viewer is read-only in v1.** Editing requires the desktop app.

### What the format already supports without changes

- Self-contained snapshots (no machine state assumed)
- Stable ids per primitive
- Forward-compat through `ExtensionBag`
- Signature slot in the manifest
- Hard size limits that protect viewers from malicious streamers

The format is ready. Server, accounts, social graph, web viewer, and the
delta-streaming protocol all need to be built — but the wire-format work
is done.

## 12a. Live streaming protocol (sketch)

**Status: design sketch. Not in the v1 build. Refine when the server work
starts.** Captured here so the static format and the streaming layer evolve
consistently.

The static XOL format alone is insufficient for live streaming — naïvely
sending one JSON delta per drag-frame at 60 Hz wastes bandwidth on framing
and per-message signatures. This section specifies the protocol that sits
*on top of* XOL to make streaming efficient.

### 12a.1 Roles and channels

- **Streamer** — authoritative origin of all chart state for the stream.
  One per stream.
- **SFU (selective forwarding unit)** — server that fans deltas out to
  viewers. Doesn't sign or modify; just forwards.
- **Viewers** — read-only consumers. May join late, scrub backward, and
  reconnect; never edit.

Two independent channels:

| Channel | Transport | Carries | Auth |
|---|---|---|---|
| **`chart`** | WebSocket (default) or WebRTC datachannel | All XOL state changes — drawings, annotations, indicators, viewport, template | Signed by streamer (batched) |
| **`market`** | Same SFU, separate stream | Optional: bars/quotes/trades from streamer's data feed | Unsigned. Viewers verify against their own data feed when possible. |

The channels are split because market-data rates (1000s/sec for tick
feeds) would otherwise crush the chart channel and force chart latency to
follow market-data backpressure. Most viewers fetch their own market data
and ignore the `market` channel entirely.

### 12a.2 Frame format

All frames are CBOR-encoded (not JSON — the static format keeps JSON for
debuggability, the live channel doesn't need it). Top-level frame:

```cbor
{
  "v": 1,                  // protocol version
  "t": "<frame_type>",     // see below
  "seq": <u64>,            // monotonically increasing per stream
  "ts_ns": <i64>,          // streamer's local time at emission
  "p": <type-specific>     // payload
}
```

Frame types:

| Type | Payload | Notes |
|---|---|---|
| `snapshot` | full XOL bytes | Sent on join + every keyframe interval |
| `batch` | array of `op` objects | A 16–33 ms window of edits, signed as one |
| `presence` | streamer status | "live", "paused", "ended" |
| `cursor` | `{x_ts_ns, y_price}` | Streamer's mouse position (optional, opt-in) |
| `selection` | array of drawing ids | What the streamer has selected — for "they're working on this" UI cues |

### 12a.3 Op types inside a `batch`

Each op targets one entity by stable id:

```cbor
{ "op": "drawing.add",      "id": <ulid>, "data": <DrawingJson> }
{ "op": "drawing.update",   "id": <ulid>, "patch": <partial DrawingJson> }
{ "op": "drawing.delete",   "id": <ulid> }
{ "op": "drawing.drag",     "id": <ulid>, "point_idx": <u8>, "ts_ns": <i64>, "price": <f32> }
{ "op": "annotation.add"    | "annotation.update" | "annotation.delete" }
{ "op": "indicator.add"     | "indicator.update" | "indicator.delete" }
{ "op": "viewport.set",     "viewport": <ViewportJson> }
{ "op": "template.apply",   "name": <string> }
```

The `drawing.drag` op is **intent replication**, not state replication —
it carries only the moving point, not the whole drawing. Viewers
reconstruct the full drawing locally from their last-known state. This
collapses 60 Hz drag traffic from "60 full drawings/sec" to "60 single
points/sec," about 8× smaller.

`drawing.update` carries a JSON-merge patch — only changed fields. A
re-color is ~30 bytes; a re-style is ~80 bytes.

### 12a.4 Batching

The streamer collects ops into a 16 ms window (one display frame at
60 Hz) before emitting a `batch` frame. Two consequences:

1. **Bandwidth** — at peak interaction (continuous drag + viewport pan),
   ~30 batched frames/sec instead of 120+ raw deltas/sec. Compressed
   payload: 3–8 KB/sec.
2. **Causality** — ops within a batch are applied atomically by viewers.
   No torn states like "drawing moved but viewport didn't follow."

Idle periods produce no frames (heartbeat is a separate `presence`
keepalive every 5 s).

### 12a.5 Signing

Per-delta ed25519 signing wastes ~64 bytes per tiny delta. Instead:

- Each `batch` frame carries a single `sig` field over CBOR-encoded
  `(seq, ts_ns, ops_hash)` where `ops_hash` is the BLAKE3 hash of the ops
  array.
- Snapshots and presence frames are signed individually.
- The server (SFU) doesn't sign anything — it can't, it doesn't have the
  streamer's private key. Viewers verify directly against the streamer's
  public key (fetched once at stream start).
- An attacker MITM'ing the SFU can drop frames but can't forge them.

Signing cost: one ed25519 sign per batch (~50 µs). At 30 batches/sec,
~1.5 ms/sec of streamer CPU. Trivial.

### 12a.6 Keyframes and time-travel scrubback

Like video, the stream alternates `batch` frames with periodic `snapshot`
frames:

- **Default keyframe interval**: 30 s. Tunable per stream.
- **Server stores**: the most recent keyframe + every batch since it. Ring
  buffer sized for N minutes of scrubback (configurable; ~10 min default).
- **Viewer scrubs back**: server replays the keyframe + all batches up to
  the target timestamp. Viewer client applies them in order.
- **Late join**: server sends the most recent keyframe + every batch
  since, then the live stream catches up.

Storage cost: keyframe is ~50–500 KB; batches are ~1 KB/sec. 10 min of
scrubback = ~600 KB per stream. Trivial.

### 12a.7 Reconnect

Viewers track the last `seq` they saw. On reconnect they send `{resume:
last_seq}`. Server replies with either:
- All batches from `last_seq + 1` (cheap path, gap small)
- A snapshot + all batches since (gap larger than the ring buffer's
  oldest batch — viewer was offline a while)

Viewer always knows which path the server took because the next frame is
either a `batch` or a `snapshot`.

### 12a.8 Bandwidth budget

Worst-case streamer (continuous drag at 60 Hz + pan + 1 indicator update/sec):

| Component | Per second |
|---|---|
| 30 batch frames × ~80 bytes payload + ~120 bytes framing/sig | ~6 KB |
| Permessage-deflate compression | ~3 KB |
| Total upload to SFU | **~3 KB/s** |
| 1,000 viewers downloading the same stream | 3 MB/s SFU egress |

Idle streamer (just looking at the chart):
- Heartbeat + occasional cursor moves: < 200 B/s.

### 12a.9 Privacy

Per spec §10, the static format never includes account IDs, position
sizes, or P&L by default. The live protocol inherits this:

- `private.json` block (if present in the snapshot) is **stripped before
  emission to viewers.** The streamer's app filters it locally; the SFU
  never sees it.
- Per-frame opt-in for cursor/selection visibility.
- Streamer can pause sharing without ending the stream — switches to
  `presence: "paused"`; viewers see the last frame frozen and a "paused"
  indicator. Useful for moments of "I'm placing an order, don't watch
  this."

### 12a.10 Abuse and rate limits

- **Per-streamer rate cap**: server drops frames over N batches/sec
  (configurable; default 60). Protects the SFU from runaway clients.
- **Frame size cap**: 256 KB per frame; larger frames rejected.
- **Snapshot size cap**: same as static XOL (50 MB).
- **Viewer cap per stream**: tiered by streamer plan; default 1,000.

### 12a.11 What's still open

- **CRDT vs LWW for edits**: the protocol assumes single-writer (streamer
  authoritative). If we ever add multi-writer collaborative editing, ops
  need a CRDT (Y.js or Automerge). For streamer→viewer this is overkill;
  flagged here for the eventual collab feature.
- **WebRTC vs WebSocket**: WebSocket is simpler and works through every
  proxy. WebRTC datachannels can be lower-latency on lossy links but add
  signaling complexity. Default to WebSocket; revisit if latency feedback
  demands it.
- **Web viewer parity**: the JS/Wasm web viewer needs a CBOR decoder, an
  ed25519 verifier (Web Crypto API has Ed25519 in modern browsers), and
  the same renderer logic as desktop. Effectively a port of the chart
  engine to Wasm — large project on its own.

The format itself is unchanged by all of this — every primitive we need
already exists in the static spec. This section is purely the wire
protocol for the live channel, isolated so it can ship later without
disturbing the on-disk format.

## 13. Validation

`xol-validate` CLI ships with apex-terminal:

```
xol-validate path/to/file.xol
  ✓ container ok (12 entries, 1.4 MB)
  ✓ manifest schema_version=1
  ✓ all file hashes match
  ✓ signature verified (author: Antoine)
  ✓ 47 drawings, 3 annotations, 2 indicators
  ⚠ indicator "thirdparty.acme.gex" not installed locally
```

Same validator runs on every load before any rendering. Validation failures
are structured errors, not panics.

## 14. Implementation plan (apex-terminal)

Module layout:

```
src/xol/
  mod.rs              public API: read/write/validate
  schema.rs           serde structs, schema_version constant
  container.rs        zip read/write, hash verification
  validate.rs         schema + size + count limits
  migrate.rs          per-version migrations (empty in v1)
  signature.rs        ed25519 sign/verify
  indicators.rs       resolve refs → loaded indicators or placeholders
  symbol.rs           canonical ↔ provider mapping
  cli/
    bin/xol-validate.rs
```

Phases:
1. **Foundations** — schema structs, container read/write, validator, CLI tool
2. **Drawing round-trip** — wire export/import to the existing drawing engine
3. **Indicators** — reference resolution, missing-indicator placeholder
4. **Cloud save** — backend table, autosave loop, version restore UI
5. **Sharing relay** — upload endpoint, short URL, recipient pull
6. **Signatures** — author keypair management, verified badge
7. **P2P (v2)** — WebRTC channel, NAT traversal

Each phase is independently shippable; phases 1–2 are the MVP.

## 15. Open questions

- Indicator registry: do we host a public registry (`apex-indicators.xllio.com`)
  for third-party indicators with semver, or just package IDs?
- Multi-symbol layouts: a "workspace" of N charts — is that a separate
  `.xolw` (workspace) format that bundles N `.xol`s, or a single file with
  a `charts: []` array? Lean toward `.xolw` to keep `.xol` per-chart.
- Replay: should `.xol` optionally embed a price-data snapshot for offline
  playback when the recipient lacks the data feed? Probably v2.

## 16. References

- Existing drawing engine: `apex-terminal/src/chart/drawings/`
- Interaction dispatch: see memory `interaction-dispatch.md`
- Symbol routing precedent: `^SPX` handling in apex-data
