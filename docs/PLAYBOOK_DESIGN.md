# Trading Playbook — Enriched Design & User Stories

Status: design proposal. Scope: turn the current single-user, in-memory play
*composer* into a durable, shareable, auto-graded, social **playbook** —
create → share → discover (feed/import) → overlay on chart → read/annotate →
fork & edit → track outcome.

Stories are written so we can BOTH **implement** them and **test** them with the
dev-inspector scenario harness (see `dev/SCENARIO_VOCAB.md`). Each story lists
acceptance criteria (testable) and a Harness note.

---

## 1. Vision & core loop

A **Play** is a structured, chart-anchored, shareable trade idea with a lifecycle:

> **Author** composes a play (entry zone, stop, target ladder, rationale,
> annotations) → **shares** it (link / file / image / Discord / feed) →
> **others discover** it (feed, import, DM) → it **overlays on their chart**
> (adapting to their timeframe) → they **read the annotations**, **fork & edit**
> their own copy → the play is **auto-graded** as price plays out (Won/Lost/
> Expired), building an accountable **track record**.

### Design pillars
1. **Durable** — a play is a first-class saved object, not transient UI state.
2. **Accountable** — outcomes are auto-graded; track records are real, not vanity.
3. **Legible** — the thesis lives ON the chart (anchored annotations), not buried in a notes box.
4. **Shareable** — one play, many transports (file, link, image, QR, Discord, feed).
5. **Forkable** — every shared play can be cloned, edited, and attributed.
6. **Integrated** — reuses apex's gamma walls, auto-chart levels, alerts, and paper order entry.

---

## 2. Current state (grounded — exists vs missing)

**Functional today** (`chart/renderer/ui/panels/plays_panel.rs`, `mod.rs:393-568`, `render/pane/core.rs:7924-8070`):
- Editor: 6 play types, direction, symbol, entry (click-to-set), T1/T2/T3 with %
  allocation, stop, live R:R, 8 preset tags + custom, notes.
- On chart: Entry/T1/T2/T3/Stop as **price-only** dashed lines + tinted zone
  bands + R:R box + badges + axis tags. **Fully draggable**, click-to-set,
  right-click context menu, **bidirectional sync** with the editor form.
- Cards: direction stripe, status pill, R:R bar, level rows, tags, notes;
  Delete / Activate / click-to-display.
- **Activate → `convert_play_to_orders`** (entry + OCO target/stop order levels).

**Missing (the whole vision):**
- **Persistence** — plays live on the in-memory `Watchlist` (`gpu.rs:5978`); the
  struct has no serde; **plays are lost on restart.**
- **Author** — `Play.author` never set. **Lifecycle** — only Draft→Active fires;
  `Published/Won/Lost/Expired/Cancelled` are dead; **no auto-grading**.
- **Sharing** — no export/import/link/QR/image; no play feed (Feed panel =
  News/Discord/Screenshots); Discord transport exists but is unwired to plays;
  no backend endpoint for plays.
- **Annotations** — a single flat `notes` string, shown only on the card, never
  on the chart. **Time anchoring** — none (price-only lines).
- **One play at a time** — no `active_play_id`; "display" clears & re-spawns.
- **Templates / spread legs / options contract** — model fields exist, no UI.

**Reusable foundations already in the tree** (don't rebuild these):
- `Play` + all sub-types derive `serde` → JSON-ready.
- `chart/state/annotations.rs` `Annotation { anchor:(ts_ns, price), title, body_md, color, asset_refs }` — canonical markdown, time+price anchored (not wired to the renderer yet).
- `DrawingKind::RiskReward { entry, stop, target, entry_time }` and `TextNote { price, time, text }`.
- Discord `send_message_bg` (`data/feeds/discord.rs:628`).
- `.xol` "share/export/import boundary" codec (`chart/state/codec/xol.rs`) — the pattern to copy.
- `TradePlanV2` wire-serialization precedent + round-trip tests (`trade_plan_panel.rs`).
- `chart_library_panel.rs` `TODO(community)` sharing seam.
- `convert_play_to_orders` (play → paper-safe bracket).

---

## 3. Enriched data model (proposed additions)

Extend `Play` (all serde-ready) and add supporting types. New fields marked ✚.

```
Play {
  // identity
  id, title, symbol, timeframe✚, created_at, updated_at✚, version✚,
  author✚ { handle, display_name, avatar_url }, origin✚ (Original | Fork{of_id, of_author}),

  // thesis
  direction, play_type, conviction✚ (1..5), horizon✚ (Scalp|Day|Swing|Position),
  catalyst✚ { kind, at_ts }, thesis_md✚ (markdown),

  // levels
  entry_zone✚ { low, high } (single price = low==high), stop_price, invalidation✚,
  targets: [PlayTarget { price, pct, label, note✚ }],  // pct must sum to 1.0

  // options / multi-leg
  contract, spread_legs: [SpreadLeg],

  // annotations (chart-anchored rationale)
  annotations✚: [PlayAnnotation { anchor:(ts,price), body_md, kind (Callout|Arrow|Label) }],

  // lifecycle & grading
  status, expiry✚, activated_at✚, resolved_at✚,
  grade✚ { realized_r, mfe_r, mae_r, targets_hit, time_to_first_target_s },
  linked_orders✚: [order_id],

  // social / organization
  book_id✚, tags, visibility✚ (Private|Unlisted|Public), template_name,
  stats✚ { views, forks, saves, reactions },
}
```

New: `Book✚ { id, name, description, owner, play_ids, visibility }` (a collection).
New: `PlayGrade✚`, `PlayAnnotation✚`, `Author✚` as above.

---

## 4. Epics & user stories

Format: **[ID] As a `<role>`, I want `<capability>`, so that `<value>`.**
AC = acceptance criteria (testable). Harness = how the scenario suite verifies it.

### EPIC A — Persistence & Library (P0 foundation)

**A1. As a trader, I want my plays to persist across restarts, so that I never lose work.**
- AC: create a play → restart the app → the play reappears with every field intact.
- AC: a corrupted/missing store degrades gracefully (empty book, no crash).
- Harness: add `PersistPlays`/reload probe; assert `playbook.play_count` and field
  round-trip survive a simulated reload. (New: `plays.json` or DB table; `Play` is serde-ready.)

**A2. As a trader, I want to search and filter my plays (symbol, status, tag, direction, R:R, date), so that I can find the right one fast.**
- AC: filtering by symbol/status/tag returns exactly the matching set; empty result shows an empty state.
- Harness: seed N plays; set a filter; assert `playbook.filtered` list matches the expected subset (recompute oracle, like `scanner_filter_correct`).

**A3. As a trader, I want to organize plays into named books/collections, so that I can group by theme (Earnings, 0DTE scalps).**
- AC: create book → move plays in/out → book membership persists; deleting a book doesn't delete its plays (configurable).
- Harness: `SeedBook`/`MovePlayToBook`; assert `playbook.books[i].play_ids`.

**A4. As a trader, I want to duplicate, rename, archive, and delete plays, so that I can manage my library.**
- AC: duplicate creates an independent copy (new id); delete removes; archive hides from default view but is recoverable.

### EPIC B — Rich Authoring (P1)

**B1. As an author, I want my identity (handle/name/avatar) stamped on plays I create, so that shared plays are attributable.**
- AC: a new play's `author` is set from the local profile; shown on the card and chart badge.
- Harness: assert `playbook.plays.0.author.handle` non-empty after create.

**B2. As an author, I want to define an entry ZONE (min–max), not just a single price, so that I can express a range entry.**
- AC: entry zone renders as a shaded band on the chart; a single price collapses the band; R:R uses the zone mid (or edge, configurable).
- Harness: `SetPlayField entry_low/entry_high`; assert `playbook.plays.0.entry_zone` and the on-chart band exists (capture band rect).

**B3. As an author, I want a target ladder (T1/T2/T3) with scale-out allocations that sum to 100%, so that partial exits are explicit.**
- AC: allocations are validated to sum to 1.0 (warn/normalize otherwise); each target has its own note.
- Harness: extend the existing target model; assert `sum(targets.pct)==1.0` oracle + per-target note round-trip.

**B4. As an author, I want to set conviction (1–5), horizon, and a catalyst with a date, so that the play's context is clear.**
- AC: fields persist and display; catalyst renders a time-anchored marker on the chart (see C5).

**B5. As an author, I want rich rationale — an overall thesis plus per-level notes ("why this stop") — so that readers understand my reasoning.**
- AC: thesis supports markdown (reuse `Annotation.body_md` restricted markdown); each level (entry/stop/each target) has an optional note.
- Harness: set thesis_md + per-level notes; assert round-trip + render (annotation present on chart, C2).

**B6. As an options trader, I want to attach a contract and build spread legs, so that options plays are first-class.**
- AC: contract + legs (side/strike/expiry/qty) editable; net debit/credit computed; legs shown on the card.
- Harness: `SetPlayContract`/`AddSpreadLeg`; assert `playbook.plays.0.spread_legs` + computed net.

**B7. As an author, I want to snap levels to key levels (swing highs/lows, round numbers, gamma walls, auto-chart trendlines), so that my levels are precise.**
- AC: a "snap" affordance offers nearby key levels; selecting one sets the price exactly. Integrates gamma (`gamma_call_wall`/`put_wall`) and auto-chart output.
- Harness: seed gamma via `SynthGamma`; snap target → assert target price == a gamma wall.

**B8. As an author, I want to create a play directly from the chart by drawing entry/stop/target, so that authoring is fast.**
- AC: a "new play from chart" mode captures the three drawn levels into a Draft play.
- Harness: `NewPlayFromLevels{entry,stop,target}`; assert a Draft play with those prices.

**B9. As an author, I want to save a play as a template and start new plays from templates, so that I reuse setups.**
- AC: "save as template" stores `PlayTemplate`; "new from template" prefills type/direction/rr/tags; `template_name` recorded on the play.
- Harness: wire the dead `PlayTemplate`; assert template create + apply.

**B10. As an author, I want an invalidation level and an expiry, so that the play self-cancels when wrong or stale.**
- AC: invalidation renders distinctly; expiry auto-transitions the play to Expired (see D3).

### EPIC C — On-Chart Visualization (P0/P1)

**C1. As a viewer, I want a play's full anatomy on the chart (entry zone, stop, target ladder, R:R zones, invalidation), so that I grasp it at a glance.**
- AC: extends the current `core.rs:7924-8070` overlay with the entry zone band + invalidation line + per-target labels with allocation %.
- Harness: display a play; assert each element captured (extend the play-line capture).

**C2. As a viewer, I want the author's annotations anchored to specific bars/prices, so that the rationale lives on the chart.**
- AC: wire `chart/state/annotations.rs` `Annotation` into the renderer; callouts/labels/arrows anchored to (ts, price); markdown body in a hover/expand.
- Harness: seed a play with annotations; assert on-chart annotation present at the anchor (capture annotation rects/anchors).

**C3. As a viewer, I want LIVE overlay metrics (distance to entry/stop/target, live R multiple, % to target), so that I can track a play as price moves.**
- AC: as the last price changes, the overlay shows current distance and live R; updates each frame.
- Harness: set a synthetic last price; assert the computed live-R equals `(price-entry)/(entry-stop)` (recompute oracle).

**C4. As a viewer, I want to show multiple plays at once and pick the active one, so that I can compare setups.**
- AC: introduce `active_play_id` + a per-play visibility toggle; multiple plays render with de-emphasis for inactive.
- Harness: display 2 plays; assert both overlays present + `active_play_id` switch.

**C5. As a viewer, I want time-anchored elements (entry-valid-until, catalyst marker, expiry countdown), so that timing is explicit.**
- AC: catalyst/expiry render at their timestamps on the x-axis; countdown label.
- Harness: set catalyst_at/expiry; assert time-anchored marker captured at the right bar.

**C6. As a viewer of someone else's play, I want it to overlay on MY chart and adapt to my timeframe, so that I can evaluate it in my context (ghost mode).**
- AC: importing/previewing a play renders its levels on the current chart regardless of the author's timeframe; a "ghost" style distinguishes it.
- Harness: import a play authored on 1d, view on 5m; assert levels overlay correctly.

**C7. As a viewer, I want a mini-chart thumbnail of the setup on each play card, so that I can scan visually.**
- AC: card shows a snapshot (reuse the screenshot capture path) or a sparkline of the setup.

**C8. As a viewer, I want to "walk through" a play step-by-step, so that I learn the thesis.**
- AC: a playback control steps through annotations/levels in author-defined order.

### EPIC D — Lifecycle & Auto-Grading (P0/P1 — the accountability core)

**D1. As a trader, I want a play to auto-activate when price enters the entry zone, so that tracking is hands-off.**
- AC: a monitor transitions Draft/Published → Active when last price ∈ entry zone; records `activated_at`.
- Harness: set price into the zone; assert `status==Active`.

**D2. As a trader, I want targets and the stop auto-marked as price crosses them, so that progress is tracked.**
- AC: crossing T1/T2/T3 marks each hit (with timestamp); crossing stop marks stop hit.
- Harness: sweep synthetic prices; assert `targets_hit` increments at the right thresholds.

**D3. As a trader, I want a play to auto-resolve to Won/Lost/Expired, so that outcomes are recorded without manual bookkeeping.**
- AC: final target → Won; stop → Lost; past expiry with no resolution → Expired; records `resolved_at`.
- Harness: drive price to target → assert Won; to stop → Lost; advance past expiry → Expired.

**D4. As a trader, I want per-play performance (realized R, MFE/MAE, time-to-target), so that I can review quality.**
- AC: `grade` computed from the price path between activation and resolution.
- Harness: replay a known price path; assert realized_r / mfe_r / mae_r match an independent recompute.

**D5. As a trader/author, I want an auto-computed track record (hit rate, avg R, by tag/type), so that my (and others') credibility is real.**
- AC: aggregate stats over graded plays; filterable by author/tag/type; drives a leaderboard (F3).
- Harness: seed graded plays; assert aggregate hit-rate/avg-R equal the recompute.

**D6. As a trader, I want to link a play to the actual orders/trades I took, so that plan-vs-actual is visible.**
- AC: `linked_orders` connects to the order system; card shows planned vs filled.

### EPIC E — Sharing & Distribution (P1/P2)

**E1. As an author, I want to export a play as a JSON file and import one, so that I can share offline.**
- AC: export writes a `.play.json`; import validates + adds to the book; malformed import fails gracefully.
- Harness: `ExportPlay`→`ImportPlay` round-trip; assert every field survives (serde is ready).

**E2. As an author, I want a shareable deep link / permalink, so that a play opens directly.**
- AC: `apex://play/<id>` (or URL) resolves to a play preview; copy-to-clipboard.
- Harness: generate link → resolve → assert it loads the same play.

**E3. As an author, I want an image card of a play (for Twitter/Discord), so that I can post it.**
- AC: renders a branded card image (reuse GDI capture path); includes levels, R:R, thesis, author.

**E4. As an author, I want a QR code to a play, so that others scan it in.**
- AC: QR encodes the deep link; scanning imports the play.

**E5. As an author, I want to share a play to Discord as a rich embed, so that my community sees it.**
- AC: wire `discord::send_message_bg` to a play → posts a formatted embed (symbol, direction, levels, R:R, link).
- Harness: mock the Discord transport; assert the embed payload contains the play fields.

**E6. As a recipient, I want to receive/import a play, preview it, then "Add to my book" or "Fork & edit", so that I can act on shared ideas.**
- AC: preview is read-only; Add copies as-is (attribution retained); Fork creates an editable copy linked to the origin (see G1).
- Harness: import → assert preview state; Add → assert in book; Fork → assert `origin=Fork`.

### EPIC F — Community Feed & Discovery (P2)

**F1. As a trader, I want a feed of published plays (community + followed), so that I discover ideas.**
- AC: add a "Plays" tab to the Feed panel (currently News/Discord/Screenshots); lists published plays as cards.
- Harness: seed a feed source; assert the feed renders the plays.

**F2. As a trader, I want to filter/search the feed (symbol, author, tag, status, performance, direction, timeframe), so that I find relevant plays.**
- AC: filters compose; results match; empty state when none.
- Harness: seed a feed; apply filters; assert the filtered set (recompute oracle).

**F3. As a trader, I want trending plays and a track-record leaderboard, so that I follow proven authors.**
- AC: ranks by realized-R / hit-rate over a window; ties broken deterministically.
- Harness: seed graded plays; assert leaderboard ordering equals the recompute.

**F4. As a trader, I want to follow authors and subscribe to books, so that my feed is curated.**
- AC: follow/subscribe persists; feed reflects follows.

**F5. As a trader, I want to comment on, react to, and bookmark plays, so that I engage and save.**
- AC: reactions/bookmarks update `stats`; comments thread on a play.

**F6. As an author, I want to publish a play (Draft→Published) with visibility controls, so that I choose who sees it.**
- AC: visibility Private/Unlisted/Public; publishing sets `status=Published` + `visibility`.
- Harness: publish → assert `status==Published`, `visibility` set.

### EPIC G — Fork, Collaborate & Version (P2/P3)

**G1. As a trader, I want to fork a play with attribution, so that I build on others' ideas.**
- AC: fork copies fields, sets `origin=Fork{of_id, of_author}`, increments the original's `forks`.
- Harness: fork → assert origin + original.stats.forks++.

**G2. As a trader, I want a diff of my fork vs the original, so that I see my changes.**
- AC: field-level diff (levels, thesis, targets) rendered.

**G3. As an author, I want to update a published play and notify followers, so that they track my adjustments.**
- AC: editing bumps `version` + `updated_at`; followers get a notification ("stop moved to X").

**G4. As a team, I want a collaborative book, so that we share plays.**
- AC: shared book membership; each play retains its author.

**G5. As a trader, I want per-level or threaded comments, so that discussion is contextual.**

### EPIC H — Actions & Integrations (P1/P2)

**H1. As a trader, I want to convert a play to a paper bracket order in one click, so that I can act on it. (NEVER live in tests.)**
- AC: reuses `convert_play_to_orders`; entry + OCO target/stop as DRAFT/paper orders; **`paper_mode` stays on; zero live submission**.
- Harness: convert → assert `no_live_orders` + order levels match the play; **never `PlaceAllDraftOrders`**.

**H2. As a trader, I want to set alerts from a play (entry-zone entered, target hit, stop hit), so that I'm notified.**
- AC: creates price alerts at the play's levels; firing marks the corresponding lifecycle event (ties to D1–D3).
- Harness: create alerts from a play; assert alerts exist at entry/target/stop.

**H3. As a trader, I want to turn an ApexSignals signal or chart pattern into a play, so that I capture ideas quickly.**
- AC: a signal/pattern seeds a Draft play with levels + a rationale referencing the source.

**H4. As a trader, I want plays integrated with the watchlist (per-symbol play badge/count), so that I see which symbols have plays.**
- AC: watchlist rows show a play indicator; clicking opens the symbol's plays.

**H5. As an author, I want gamma walls / auto-chart levels suggested as snap targets, so that authoring uses apex's intelligence.**
- AC: see B7; suggestions come from `SynthGamma` walls and auto-chart output.

### EPIC I — UX Polish & Bells/Whistles (P2/P3)

**I1.** Compact/expanded play cards + hover preview.
**I2.** Live R:R calculator while dragging levels (extends the existing live R:R).
**I3.** Empty states, onboarding, and a few sample plays on first run.
**I4.** Keyboard shortcuts + command-palette actions (new play, publish, share).
**I5.** Undo/redo on play edits.
**I6.** Toasts/notifications when a play triggers or hits a target.
**I7.** Accessibility (keyboard nav, screen-reader labels) + theming for cards/overlay.

---

## 5. Phasing

- **P0 (foundation, unblocks everything):** A1 persistence, B1 author, C1 full
  overlay, D1–D3 auto-grade lifecycle. Without persistence + grading, sharing is moot.
- **P1 (make it rich & actionable):** A2–A4 library, B2–B10 authoring, C2–C5
  annotations/live/multi-play, D4–D6 performance, E1–E3 export/link/image, H1–H2 actions.
- **P2 (make it social):** E4–E6 QR/Discord/import-flow, F1–F6 feed/discovery/publish,
  G1–G2 fork/diff, H3–H5 integrations.
- **P3 (collaborate & polish):** G3–G5 versioning/teams/comments, I1–I7 polish, backend sync.

## 6. Test strategy (how the harness proves these)

The dev-inspector suite (`dev/`) already drives + observes plays: `SeedPlay`,
`SetPlaybookPanel`, `ClearPlays`, `playbook.*` capture, and the `play_rr_correct`
oracle. Extend the same way per epic:
- **New driver commands:** `SetPlayField`, `AddSpreadLeg`, `ActivatePlay`,
  `SetPrice` (synthetic last price for grading), `ExportPlay`/`ImportPlay`,
  `PublishPlay`, `ForkPlay`, `SeedFeed`, `SeedBook`.
- **New capture:** persisted-reload probe, `status`, `grade`, `annotations`,
  `books`, `feed`, `filtered` sets, on-chart element rects.
- **Behavioral oracles (derive-and-compare, with teeth — our established bar):**
  allocation-sums-to-1, live-R recompute, realized-R/MFE/MAE recompute,
  filter/leaderboard recompute, JSON export→import round-trip equality,
  auto-grade thresholds (Won/Lost/Expired at exact prices).
- **Safety invariant (unchanged):** any play→order path asserts `no_live_orders`
  and never emits `PlaceAllDraftOrders`/`PlaceAllDraftAlerts`.

Every acceptance criterion above is written to be checkable this way — so each
story ships with its scenario, exactly as we did for DOM/scanner/RRG/gamma/
spreadsheet/auto-chart.
