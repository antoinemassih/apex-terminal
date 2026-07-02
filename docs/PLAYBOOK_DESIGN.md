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

---

## 6b. Play anatomy & authoring UX (the parameter space + how you add it)

Guiding principle: **direct manipulation first, forms last.** Dragging a level is
intuitive because it happens in price space. *Every* parameter should be added the
same way — a gesture on the chart, a one-tap chip, or an inline expression — never
a wall of form fields. Progressive disclosure keeps a beginner at "3 lines you
drag" while a pro composes a scenario tree with a hedge and a payoff curve on the
same canvas.

### 6b.1 The parameter taxonomy — what a play can consist of

1. **Levels (spatial skeleton):** entry (price → zone → scale-in ladder w/ weights);
   stop (fixed → trailing $/%/ATR → structure "below the line" → time-stop); targets
   (T1/T2/T3 ladder w/ scale-out %); invalidation (distinct from stop); breakeven rule;
   pyramid/add levels.
2. **Time:** valid-from/until (entry window); catalyst marker (date on x-axis); expiry;
   time-stop; session rules (RTH-only, skip-open).
3. **Triggers & conditions:** entry style (limit/stop/on-close/on-retest/market);
   if-then branches (scenario tree); confirmations (volume/indicator/gamma/other symbol);
   per-level alerts.
4. **Structure/style:** long/short; type (directional/breakout/mean-rev/scalp/swing/
   position/event); hedge leg; pairs (long A/short B); options (single→vertical→calendar
   →straddle→condor).
5. **Sizing & risk:** size as shares/contracts/$/%-account/risk-based; risk per trade
   ($ or R); R:R (derived); max concurrent risk; scale weights.
6. **Narrative:** thesis (markdown); per-level notes; anchored callouts; play-owned
   drawings; tags; conviction; horizon; frozen context (gamma/day-type/IV at authoring).
7. **Auto/derived:** snap-to-key-levels (gamma walls, auto-chart trendlines, swing H/L,
   round numbers, VWAP, PDH/PDL, fibs); auto-stop from R:R; auto-target from ATR/structure;
   suggested size from risk.

### 6b.2 The authoring mechanics (build ALL of these)

- **Draggable everything; handles spawn handles.** Drag target → ghost "+T2" appears;
  reward/risk box grows; scale-out % is a ribbon you split by dragging.
- **Magnetic snap + live HUD.** While dragging, a floating readout shows price·distance·
  live-R and the line snaps to key levels with a labeled tick ("call wall 455"). You
  *feel* the levels. (Snap sources already computed by apex: gamma, auto-chart, VWAP.)
- **Type morphing on the badge.** Click a line's badge to cycle Entry→Stop→Target→
  Invalidation; long-press for a radial (limit/stop/trailing/time). One object, many roles.
- **Quick-add chips.** `+target +scale-in +hedge +option +trigger +note +alert` — one tap
  drops the element with sensible defaults you then drag.
- **Triggers you draw.** Break line + arrow = breakout trigger; two lines = range with
  if-above/if-below branches (the scenario tree is drawn spatially).
- **Sketch-to-structure.** Rough-draw a trendline/zone; it snaps to structure and offers
  "use as stop reference / entry zone."
- **Inline math in any level.** `=3R` places the stop at 3:1; `ATR`, `vwap`, `pdh`,
  `callwall` resolve to those. Levels are expressions, not just numbers.
- **R:R box as a first-class object.** Grab the whole reward/risk rectangle and reshape it.
- **Hedge/spread as a dimmed second track;** options via a mini strike ladder on the price
  axis + a live payoff curve (tap strikes, not a form).
- **Annotations by pointing or voice.** Click a bar → type → anchored callout; or hover +
  say "note: breakout retest here" (voice bridge).
- **Templates lay out the whole skeleton** + auto-levels; drag to fit.
- **Progressive disclosure:** default = today (entry/stop/target you drag); everything
  else behind one "Advanced" reveal + the chips. **Numeric + gesture parity:** every
  draggable level also has a tiny inline number + keyboard nudge.

### 6b.5 Multi-instrument plays = multi-pane authoring

Some ideas span more than one instrument — a **hedge** on a different symbol, a
**long-A/short-B pair**, a **basket**. You can't author those on one chart: each
leg needs its own pane where you drag *its* levels. This maps directly onto
apex's existing multi-pane substrate (`panes: Vec<Chart>`, the `Layout` enum,
pane splits, per-pane `SwapPaneSymbol`).

**Model.** A play becomes a set of **legs**, each with its own symbol + level set,
plus a remembered **layout**:
`Play { …, legs: [PlayLeg { symbol, role (Primary|Hedge|PairShort|BasketMember),
direction, entry_zone, stop, targets, weight, notes }], layout: LayoutSpec }`.
`LayoutSpec` = the pane arrangement + which leg maps to which pane slot (e.g. pair →
vertical 2-split, leg0→pane0 / leg1→pane1; basket → grid). The single-symbol play is
one Primary leg with a 1-pane layout (backward-compatible).

**Two symmetric flows — authoring and opening produce the *same* multi-pane view:**

*Authoring (auto-open on add).* When you `+ hedge` / `+ pair leg` / `+ instrument`
and pick a symbol:
1. the view **auto-splits** and the new instrument opens in a fresh pane
   (`SwapPaneSymbol` on the next slot); the play's `layout` records the slot,
2. that leg's levels render **on that pane**, ready to drag — you apply
   entries / exits / TP / stop / parameters **in each pane independently**,
3. removing a leg collapses its pane back.

*Opening/reviewing a saved play (restore).* Selecting a saved multi-instrument
play **reconstructs its whole view**: apply its `LayoutSpec` (split the panes),
`SwapPaneSymbol` each pane to its leg's symbol, and re-spawn each leg's level set
on its pane — so you land exactly where the author left it and can edit any leg's
parameters in its own pane. Same code path as auto-open, driven from stored state
instead of a live add. (Today's single-pane "display" already re-spawns one leg's
lines on the active pane; this generalizes it to N panes + a layout.)

**Per-pane binding + ghost/detach.** Each leg binds to whichever pane shows its
symbol; drag leg-B's stop in pane 1 → the play updates → the card reflects it.
If a pane's symbol changes or the pane closes, that leg's levels **detach**
(still stored on the play) and re-appear as a ghost overlay when the symbol is
shown again — same mechanism as viewing someone else's play (C6).

**Cross-leg intelligence.** The card summarizes all legs with **combined
exposure, net R:R, and correlation/heat** (ties to the portfolio-playbook idea):
"long SPY / short QQQ, net beta ~0, correlated 0.9 — this is a spread, not two bets."

**Options note.** Options *legs* share one underlying, so they author on a single
pane via the strike-ladder + payoff curve (Phase 4) — no extra panes. Multi-pane
is specifically for multi-*instrument* legs (Phase 3 hedges/pairs/baskets).

**Harness-testable:** add a hedge leg on symbol B → assert pane 1 shows B and
leg-B levels are present on pane 1 (per-pane play-line capture, which the suite
already does for pane-1 gamma/DOM). Layout restore + detach/ghost are assertable
via pane_type/symbol + play-line presence per pane.

### 6b.3 Styles of ideas = one-tap templates

directional · scale-in/pyramid · bracket/OCO · range (if-above/if-below) · mean-reversion ·
breakout-with-retest · trailing runner · hedged (core + protection) · pairs (long/short) ·
options single/vertical/calendar/straddle/condor · event/catalyst · time-boxed scalp.

### 6b.4 Phased roadmap (everything, phased — user-chosen)

- **Phase 1 — Rich levels + intelligent drag** (the core everyone touches):
  entry zone, scale-in/out ladders w/ weights, trailing/time stops, invalidation,
  per-level notes + alerts; **magnetic snap + live HUD**; quick-add chips; badge morphing;
  inline `=3R`/`ATR`/`vwap` expressions. Snap engine is a pure function → behavioral-oracle
  testable (snap near a gamma wall → lands on the wall exactly).
- **Phase 2 — Time & triggers:** entry styles, if-then scenario branches (draw-a-trigger),
  valid-until/expiry (expiry grading already exists), catalyst markers, confirmations.
- **Phase 3 — Hedges & pairs:** paired protective leg + long-A/short-B as a dimmed second
  track; combined risk/heat.
- **Phase 4 — Options:** legs via strike-ladder + live payoff curve; greeks/IV context;
  DTE management.

Each phase ships behind progressive disclosure, extends the existing `PlayLine` drag +
`convert_play_to_orders` (paper-safe), and lands with harness commands + behavioral-oracle
scenarios — snap-exactness, allocation-sums-to-1, expression resolution, trigger logic,
payoff math — same bar as the rest of the suite. `no_live_orders` stays on every order path.

## 7. Build-on ledger — status of every story vs what already exists

Legend: **DONE** = already functional, reuse as-is · **ENHANCE** = partly exists,
extend it · **NEW** = build from scratch (scaffold may exist). File anchors point
at the existing code to build on.

| Story | Status | What already exists / what to add |
|---|---|---|
| A1 persist | **NEW** | `watchlist.plays` in-memory only (`gpu.rs:5978`), no serde on `Watchlist`. `Play` IS serde-ready → add `plays.json`/DB. |
| A2 search/filter | **NEW** | Card list render exists (`plays_panel.rs:663`); no filter/search UI. |
| A3 books | **NEW** | No collection concept. |
| A4 dup/rename/archive/delete | **ENHANCE** | **Delete DONE** (`plays_panel.rs:91`); dup/rename/archive new. |
| B1 author | **NEW (small)** | `Play.author` field exists, never set (`mod.rs:562`). Add a profile + stamp on create. |
| B2 entry zone | **ENHANCE** | Entry as single price line + click-to-set **DONE**; widen to a low/high zone. |
| B3 target ladder + allocation | **DONE→ENHANCE** | **T1/T2/T3 with per-target % allocation stepper already in the editor** (`plays_panel.rs:290-386`, `add_target_line`). Add: validate Σpct=1.0 + per-target `note`. |
| B4 conviction/horizon/catalyst | **NEW** | none of these fields exist. |
| B5 rich rationale + per-level notes | **ENHANCE** | flat `notes` string **DONE** (`plays_panel.rs:450`); add markdown (reuse `Annotation.body_md`) + per-level notes. |
| B6 options contract/legs | **ENHANCE** | `contract` + `spread_legs` **model exists** (`mod.rs:441`), no editor UI → build the UI. |
| B7 snap to key levels | **NEW** | no snap; but gamma walls (`SynthGamma`) + auto-chart output already available to snap to. |
| B8 new-from-chart | **ENHANCE** | click-to-set + drag lines + `spawn_play_lines` **DONE** (`plays_panel.rs:139`); add a thin "capture drawn levels → Draft" wrapper. |
| B9 templates | **NEW (on scaffold)** | `PlayTemplate` + `template_name` + `watchlist.play_templates` exist but **inert** → add save/apply. |
| B10 invalidation/expiry | **NEW** | no such fields. |
| C1 full on-chart anatomy | **DONE→ENHANCE** | **Entry/T1/T2/T3/Stop lines + tinted zone bands + R:R box + badges + axis tags all render** (`core.rs:7924-8070`). Add: entry-zone band, invalidation line, per-target allocation labels. |
| C2 anchored annotations | **NEW (wiring)** | `Annotation{anchor(ts,price),body_md,...}` **model exists** (`state/annotations.rs`), **not wired to renderer** → wire a draw path. |
| C3 live overlay metrics | **ENHANCE** | live R:R box **DONE** (`core.rs:7962`); add live distance / live-R-as-price-moves. |
| C4 multiple plays / active | **NEW** | one play at a time; no `active_play_id` (`play_lines` is a flat singleton-per-kind vec). |
| C5 time-anchored elements | **NEW** | `PlayLine` is **price-only, no time anchor** (`mod.rs:480`). |
| C6 ghost mode (cross-tf overlay) | **ENHANCE** | price-only lines **already overlay on any timeframe**; "display" re-spawns them (`plays_panel.rs:104`). Add ghost styling + import-preview. |
| C7 thumbnail | **NEW** | reuse the GDI screenshot capture path. |
| C8 walkthrough | **NEW** | none. |
| D1 auto-activate | **NEW** | only the **manual "Activate" button DONE** (`plays_panel.rs:94`). No price monitor. |
| D2 auto-mark hits | **NEW** | order-fill logic exists for `OrderLevel` but is **decoupled** from `Play`. |
| D3 auto-resolve Won/Lost/Expired | **NEW** | those statuses exist as enum variants but are **never set**. |
| D4 performance metrics | **NEW** | only `risk_reward` is computed (once). |
| D5 track record | **NEW** | none. |
| D6 link play↔orders | **ENHANCE** | **`convert_play_to_orders` DONE** (`plays_panel.rs:612`); store the linkage. |
| E1 export/import JSON | **NEW (small)** | `Play` serde-ready; copy the `.xol` codec pattern (`state/codec/xol.rs`). |
| E2 deep link | **NEW** | none. |
| E3 image card | **NEW** | reuse GDI capture + card renderer. |
| E4 QR | **NEW** | none. |
| E5 Discord embed | **ENHANCE** | **`discord::send_message_bg` transport DONE** (`discord.rs:628`), unwired to plays → format an embed. |
| E6 receive/import flow | **NEW** | none. |
| F1–F6 feed/discovery/publish | **NEW** | Feed panel exists (News/Discord/Screenshots) → add a Plays tab; `signals_feed` is the inbound-transport pattern; `TODO(community)` seam in `chart_library_panel.rs`. |
| G1–G5 fork/diff/version/collab | **NEW** | none. |
| H1 play→paper bracket | **DONE** | **`convert_play_to_orders` (entry+OCO) DONE**; just assert `no_live_orders`. |
| H2 alerts from play | **ENHANCE** | `AddPriceAlert` **DONE**; wire play levels → alerts. |
| H3 signal→play | **NEW** | `signals_feed` inbound exists; add a "signal → Draft play" action. |
| H4 watchlist play badge | **NEW** | none. |
| H5 gamma/auto-chart snap | **NEW** | see B7; source data exists. |
| I1 card states | **ENHANCE** | card **DONE** (`plays_panel.rs:663`); add compact/expanded/hover. |
| I2 live R:R while dragging | **DONE** | live R:R **DONE** + line drag **DONE** — already combined in the editor. |
| I3 empty states/onboarding | **NEW** | none for plays. |
| I4 keyboard/palette | **NEW** | none for plays. |
| I5 undo/redo | **NEW** | drawing undo/redo exists as a pattern to copy. |
| I6 toasts | **ENHANCE** | `alert_feed` badge system **DONE**; wire play events. |
| I7 a11y/theming | **ENHANCE** | cards use themed `ui_kit` **DONE**; add a11y labels. |

**Headline:** the **authoring + on-chart + play→order layers are substantially
built already** (editor with target-ladder allocations, fully-interactive price
lines with form sync, zone bands + R:R box, cards, activate→OCO). The genuinely
NEW work concentrates in four areas: **persistence**, **lifecycle/auto-grading**,
**chart-anchored annotations + multiple/time-anchored plays**, and the **entire
sharing/feed/social layer**. P0 therefore leans heavily on existing code
(persist what's already composed; auto-grade what already renders; keep the
existing overlay + order conversion) rather than rebuilding.

---

## 8. Design, usability & the advanced ceiling

The functionality above is the skeleton. This section is the *product* — how it
looks, how it feels, and how far it can go. apex has native superpowers
(gamma walls, auto-chart, the day-type classifier, signals feed, voice bridge,
homelab LLM, paper-order engine, GDI capture) that let the playbook do things a
generic "ideas" feed can't.

### 8.1 Design language

**The Play Card is the hero object** — one atom that renders coherently across
five surfaces (chart badge, panel list, feed, hover preview, share image). One
information hierarchy, four densities:
- **Ticker chip** (compact, feed row) — symbol · direction arrow · R:R · status dot.
- **Standard card** (list) — + level ladder, allocation ribbon, tags, author, mini track-record badge.
- **Expanded** (detail) — + thesis (markdown), per-level notes, sparkline/thumbnail, live metrics, comments.
- **Hero image** (share) — branded, screenshot-quality, QR + deep link baked in.

**Visual encoding that stays legible under stress:**
- Direction is **shape + color** (▲/▼ *and* bull/bear), never color alone → color-blind safe.
- **R:R as a proportioned bar** (reward green / risk red segments), not just a number.
- **Status is a state machine you can read**: Draft (dashed/ghost) → Published (solid) →
  Active (pulsing) → Won (green fill) / Lost (red) / Expired (grey).
- **Conviction** as filled pips (●●●○○); **track record** as a small "W/L, avg R" chip on the author.

**The on-chart overlay is a narrative, not just lines.** Today it's dashed
price lines + zone tints. Advanced version:
- **Entry zone** as a soft glow band; **reward box** (entry→targets) tinted green,
  **risk box** (entry→stop) tinted red, sized to actual R:R — the trade's shape is instantly visible.
- **Target ladder** with allocation ribbons (T1 50% / T2 30% / T3 20%) drawn as proportional ticks.
- **Anchored annotation callouts** with leader lines to the exact bar ("breakout retest here"),
  markdown on hover/expand.
- **Catalyst/expiry markers** on the time axis with a live countdown.
- **Live approach animation**: as price nears the entry zone the band "breathes";
  a live R-multiple readout tracks price; crossing a level triggers a subtle flash + toast.

**Motion & micro-interactions** (egui-friendly, cheap):
- Drag a level → **magnetic snap** to gamma walls / auto-chart levels / swing points / round numbers,
  with a **live R:R readout** floating at the cursor.
- Card hover → chart preview lights up that play (ghost overlay).
- Status transitions animate (Draft→Active pulse, Won confetti-lite).
- Everything reversible (undo/redo); nothing modal that can't be escaped.

### 8.2 Usability principles & signature flows

**Principles:** progressive disclosure (simple by default, advanced on demand) ·
smart defaults (auto-derive stop from target × R:R; snap to key levels; Σallocation
auto-normalizes) · zero-friction creation · always reversible · cross-surface
coherence · keyboard-first + accessible.

**Signature flows (the ones that make it feel magical):**
1. **Create-from-chart in 3 gestures** — click entry, click stop, click target on the
   chart → a Draft play materializes (levels, live R:R, snapped to key levels). Reuses
   the existing click-to-set + drag lines; just adds the capture wrapper.
2. **Signal → play** — a card on an ApexSignals pattern has "Make it a play"; prefills
   levels + a rationale referencing the signal.
3. **Talk it into existence** — voice bridge: "long SPY breakout above 450, stop 445,
   target 460, thesis gamma wall magnet" → Draft play. (apex already has the voice pipe.)
4. **Fork in one click** — someone's play → "Fork & edit" → your editable copy with
   attribution; a diff shows what you changed.
5. **Receive → preview → act** — an imported/DM'd play opens as a read-only ghost on
   YOUR chart (adapts to your timeframe) → Add to book / Fork / set alerts.
6. **Live grading feedback loop** — you don't babysit: toasts fire "SPY play hit T1
   (+1.8R)"; the card auto-updates; your track record ticks.

**Onboarding & empty states:** first run seeds 2–3 sample plays (a won, a lost, an
active) so the value is obvious; coach-marks on the create-from-chart gesture;
command-palette entries ("New play", "Publish", "Share to Discord").

### 8.3 The advanced ceiling (three tiers)

**Tier 1 — Premium polish (a great product):** persistence + auto-grading + the
card/overlay design above; export/link/image/QR/Discord share; a Plays feed with
filters; alerts from a play; play → paper bracket (paper-safe). All achievable with
apex's existing tech.

**Tier 2 — apex-native differentiators (nobody else can do these):**
- **Context fusion** — a play *references* apex intelligence: "target = gamma call
  wall", "stop below auto-chart trendline", "invalid if flip breaks". Levels snap to
  and stay linked to gamma walls / auto-chart output / seasonality / earnings dates.
- **Day-type-conditioned track records** — grade performance *by regime* using the
  day-type classifier: "your breakout plays win 71% on trend days, 22% on PIN days."
  This is genuinely differentiating analytics.
- **Auto-grading analytics** — expectancy, MFE/MAE, calibration curves (does your 3:1
  actually pay 3:1?), edge decay, Kelly-suggested sizing, per-setup win rates.
- **LLM thesis & critique** (homelab Qwen/Claude) — draft a thesis from chart context
  (pattern + levels + gamma + news); **critique a play** ("stop is inside the 1-ATR
  noise band; T2 sits exactly on the put wall — consider trimming before it");
  natural-language authoring; auto-summarize a noisy feed into the 3 best setups.
- **Backtest-a-setup** — "how has this exact template performed on this symbol
  historically?" using apex's bar history + replay engine.

**Tier 3 — moonshots (category-defining):**
- **Live war-room** — real-time co-authoring + presence + reactions on a play during
  its catalyst (earnings, FOMC); the play updates for all watchers live.
- **Paper copy-trading** — subscribe to an author; their published plays auto-instantiate
  as *paper* brackets in your account (paper-safe by construction); your track record
  vs theirs. **Never live without explicit per-order consent.**
- **Reputation economy** — verified, auto-graded track records; leaderboards by realized
  R and calibration; "provably not cherry-picked" because grading is automatic.
- **Public/embeddable plays** — a play has a public web view + an embeddable widget
  (Twitter/blog); the image card + deep link already make this cheap.
- **ML-personalized feed** — rank by "plays like ones you've won", your risk profile,
  and authors whose edge holds in the current regime.
- **Voice-annotated walkthroughs** — narrate a play; it becomes a step-through lesson
  (voice bridge + playback control).

### 8.4 Why this beats a generic "ideas" feed

TradingView-style ideas are static screenshots + text. apex's playbook is
**structured, live, auto-graded, and fused with real market microstructure**:
- levels that *snap to and reference* gamma walls and auto-detected structure,
- outcomes graded automatically (no self-reported wins),
- performance sliced by **day-type/regime**,
- one-click **paper** execution and copy-trading,
- LLM critique using the *actual* chart context,
- hands-free **voice** authoring.

Each of these plugs into an apex system that **already exists** — the playbook is
the surface that ties them together into a shareable, accountable trade-idea object.

### 8.6 Further improvements — next capabilities (prioritized)

Beyond the tiers above, the highest-leverage *new* directions:

**Capability — a living, context-aware play:**
- **Frozen context provenance** — snapshot the world at authoring (gamma walls,
  day-type, IV rank, pattern, news) so a reader/fork sees what the author saw.
- **Live thesis-health / auto-invalidation** — rules+LLM flag "thesis compromised"
  when the premise breaks (gamma flip, trend break, catalyst passed, IV collapse).
- **Conditional / scenario-tree plays** ("if breaks 450 → long; if rejects → short").
- **Portfolio-level playbook** — correlation-aware risk/heat across active plays.
- **Position-sizing engine** — size from account × risk% × stop distance; concurrent-risk caps.

**Usability — collapse friction:**
- <5 clicks to publish (a measured budget); contextual "make a play from this"
  on any level/drawing/gamma wall/signal; inline edit on every surface (no modals);
  smart triage inbox for received plays (ranked by author track record + watchlist
  relevance); draft autosave + resume; keyboard-first flow + palette; intent search.

**UX — glanceability & honest feedback:**
- The 1-second rule (one hero number = live R, rest on demand); surface author
  **calibration** (accuracy at this conviction); distance-to-decision urgency;
  constructive post-mortems ("what would have improved this"); notification hygiene;
  a11y + reduced-motion first-class.

**Design — one visual grammar & restraint:**
- A single reusable play geometry across card/chart/feed/image; overlay recedes
  until relevant; density modes (cockpit vs focus); motion encodes state only;
  the share image as a designed, brand-worthy artifact; theme via StyleSystem×ColorScheme.

**Cross-cutting — trust & ecosystem (the moat):**
- Anti-gaming verified track records (can't cherry-pick; reward *process* over luck);
  granular privacy/visibility; interop (TradingView import, broker export, webhooks);
  author analytics.

**Top three to build next:** (1) context provenance + live thesis-health (uniquely
enabled by apex's gamma/day-type/signals), (2) contextual one-gesture creation
(friction is the #1 killer of sharing), (3) anti-gaming verified track records
(the trust the whole social layer rests on).

### 8.5 Design/usability acceptance (testable via the harness)

- Color-blind-safe: direction encoded by shape+color (assert both present).
- Cross-surface coherence: the same play's key fields match across card/chart/feed captures.
- Progressive disclosure: advanced fields hidden until toggled (widget-tree conditional, like the auto-chart panel).
- Live metrics correctness: live-R / distance / calibration all pass derive-and-compare oracles.
- Snap correctness: snapped level == the referenced gamma wall / auto-chart level (exact).
- Accessibility: every interactive control carries a role+label in the widget-tree.
- Safety: copy-trading & play→order paths assert `no_live_orders`; never `PlaceAllDraftOrders`.
