# Visual audit — 2026-08-05

Five review agents over 13 captured surfaces, plus direct verification of every
finding reported below. Ranked by severity within each section.

---

## 0. Read this first: the capture set was compromised

The audit ran on a screenshot set that does not show what its filenames claim.
This was discovered mid-audit, from the agents' own reports, and confirmed
directly.

| Claimed | Actual |
|---|---|
| 13 distinct surfaces | The **Settings modal is open in all 13**, covering the chart |
| `02/03/04-watchlist-{chain,heat,scan}` | **Pixel-identical.** Tab switches never landed; only CHAIN ever rendered |
| `07-orders-panel` | **Byte-identical** to `05-scanner` — the orders panel never opened |
| `05-scanner` | Shows the Indicators panel + CHAIN tab. No scanner content |
| `08-order-entry` | The order-entry form is not visible |
| Theme | `theme_idx=15 / style_idx=0` — an uncertified "Custom pairing" |

Three agents independently reported "these images are identical" without being
prompted to look for it. **The harness reported success for every one of those
steps.**

### What that means for the findings below

- **Geometry findings survive.** Overlap, clipping, misalignment and truncation
  are independent of palette, and the side panels were never covered by the
  modal. Everything in §1 and §2 was verified directly against the pixels.
- **Colour findings do not survive** and are excluded. On an uncertified
  pairing you cannot separate a palette bug from a combination nobody designed.
  The BUY/SELL pills render flat grey rather than bull/bear in this set; that is
  *not* reported as a defect until it is re-shot on a certified preset.
- **Four surfaces have no evidence at all** and were not audited: watchlist
  LIST / HEAT / SCAN, the scanner, the order-entry form, and the orders table.

### Root cause (fixed)

`/cmd` accepted `OpenSettings`, `OpenOrderEntry` and `OpenOrdersPanel` and
routed them to `QueuedDevCmd::HeadlessDialog` — which mutates the **headless
ticker's simulated** `open_dialogs` vector. In a windowed run nothing paints
from that vector. The commands returned `{"ok":true}`, changed a field no one
draws, and the UI never moved. `CloseAllDialogs` was the only dialog command
wired to real state, and it only closes.

Compounding it, `/state` could not report which watchlist tab was active — so
the tab sweep was *unfalsifiable*. A step cannot be asserted against state the
harness cannot see.

Fixes landed with this audit:

1. `AppCommand::SetDialogOpen { name, open }` — real dialog control; `/cmd`'s
   `Open*`/`Close*` names now drive the state the window actually paints.
2. `/state.watchlist.tab` now reports `list | chain | heat | scan`.
3. `scripts/ds-harness/capture_surfaces.py` — asserts the app is in the target
   state via `/state` **before** each shot, and refuses to write a file when it
   is not. A missing file is a bug report; a mislabelled file is a lie that
   survives into the audit.

> **Rule this establishes:** anything a scenario can *switch*, `/state` must be
> able to *report*. This is the same lesson `capture_app.py` already learned for
> theme convergence — generalised from "the palette is right" to "the screen is
> right".

---

### Re-capture

`docs/styling/audit/v2/` holds surfaces re-shot through the asserted script on
the certified **aperture / aperture** preset, pixel-verified as genuinely
distinct — including the first real captures of watchlist LIST, HEAT and SCAN
and of the orders panel. `v3/` holds re-shoots verifying the §1.2–§1.4, §2.0
and §2.7 fixes. The original set is kept in place for provenance.

The order-entry form is **not** among them: there is no such form to capture
(§2.8).

---

## 1. Order entry / DOM ladder — the highest-stakes surface

### 1.0 — CRITICAL — BUY and SELL rendered in the IDENTICAL colour

Found while verifying the fix for §1.1, on the certified preset. Sampled: BUY
and SELL both filled `rgb(153,55,34)`. Aperture's palette is not at fault —
its `bull` is green `(78,192,122)` and `bear` red `(216,80,62)`. The hue never
reached the paint.

Both buttons are `Variant::Primary`, differing *only* by tint. Every style
authors `button.primary` with `.fill(tone(Accent))`, and the recipe fill
replaced the variant-computed background outright — discarding the tint. So
**the two buttons that open opposite positions were visually
indistinguishable**, and every other semantically-tinted primary button in the
app collapsed to accent along with them.

**Fixed.** A recipe describes the *treatment*; `Accent` in a fill means "the
theme's primary hue", which is a default, not an assertion. A caller-supplied
semantic tint now re-hues an Accent recipe fill while keeping the treatment.
Narrow by design: only a fill that resolved to Accent is re-hued, alpha
preserved; a recipe naming any other tone keeps it. Two regression tests guard
it.

Note this was invisible on the original set for the exact reason §0 gives — the
uncertified pairing made every colour finding unusable, so the most severe
colour defect in the app sailed through five agents untouched.

### 1.1 — CRITICAL — BUY and SELL paint over FLATTEN and CANCEL

`dom_panel.rs:476-543`. Verified: BUY's rect covers FLATTEN's leading `F`; SELL
covers the `Ctrl+…` hint on both FLATTEN and CANCEL. Four overlapping controls,
each of which moves real money.

The row computes four slots that tile the panel width exactly, then passes each
one via `.min_size(...)` — which is a **floor only**. When keyboard hints were
later added ("UX-1 Fix 3"), `"BUY"` became `"BUY  Ctrl+B"`, the intrinsic width
exceeded the 30 %-of-width slot, and the buttons grew straight over their
neighbours.

Both values were individually reasonable. Only the *relationship* between
content width and slot width was wrong — so no token or vocabulary check could
see it. This is the **frozen-chrome defect class** again, in its overflow form.

**Fixed.** Added `Button::max_width` / `Button::fixed_size` — a real ceiling to
pair with the existing floor. When content does not fit, the keyboard hint is
dropped first (it is the affordance, not the action) and the label re-measured
before clamping, so a tight slot loses the hint rather than half the word. The
four DOM action buttons now use `fixed_size`.

The doc comment on `measure_content_w` already described this exact failure
("overran its 60px slot, the next button painted on top of it") and
`intrinsic_width` existed to prevent it — but nothing forced a caller to use
either.

### 1.2 — CRITICAL — the ladder drops the minus sign off 4-digit deltas

Not reported by any agent; found on direct inspection.

3-digit deltas render `-595`, `+186`. Four-digit ones render `·1437`, `·1698`,
`·1795` — the sign is clipped by the panel's left edge. **A depth ladder that
silently drops the sign off a delta shows the trader the wrong direction on
exactly the large prints that matter most.** This is a correctness defect
wearing a layout defect's clothes.

**Fixed.** Three changes, because one was not enough: cells now paint through a
painter clipped to their own column; `fit_cell` picks the widest of several
progressively shorter forms that fits; and Δ is left-aligned so clipping can
only ever eat the least significant digits. A test asserts that **every** form
of a delta keeps its sign — `fit_cell` may pick any of them, so the full form
being correct is not enough.

### 1.3 — HIGH — Δ and BID columns have no gutter; digits collide

Every row. `-595` + `703` renders as `-595703`; `+186`+`670` as `+186670`.
There is a divider between BID│PRICE and ASK│VOL, but **none** between Δ│BID or
PRICE│ASK, so both size columns run into their neighbour. On the densest
numeric surface in the app, size and delta cannot be read apart.

**Fixed** by the same per-cell clip and gutter as §1.2 — a collision between
adjacent columns is now structurally impossible rather than merely unlikely.

### 1.4 — HIGH — the "SIMULATED" badge is drawn over the column headers

The badge lands on top of the `PRICE` and `ASK` headers, smearing both
illegible. Two of five column headers are unusable, so the header row cannot be
used to disambiguate §1.3.

**Fixed.** The badge now has its own band above the header row, sized from the
tier it is painted in. Note it was only wide enough to collide in the
`SIMULATED` case — i.e. exactly when the numbers beneath it are fabricated.

### 1.5 — WITHDRAWN — the "size-flash highlight" is a depth bar

Reported as a highlight box failing to track its text. It is not: it is the
**depth bar**, whose width encodes size (`bw = fill * col_w * 0.85`), with the
number painted over it in a contrasting colour where the two overlap. It is
supposed to vary — that is the data.

Retained as a **legibility** note rather than a defect: the two-tone split makes
whichever digit happens to straddle the bar's edge read as accidentally bold.
Worth a design look, but nothing is broken.

### 1.6 — NOT REPRODUCED — the ladder overdrawing its bottom edge

Originally: the row below the last full row clipped mid-glyph and bled onto the
`MKT` dropdown. It no longer reproduces at the viewport that showed it.

Not claimed as fixed — the §1.4 badge band moved `body_top`, changing which row
straddles the bottom edge, so this is most likely incidental. Re-check if it
resurfaces.

---

## 2. Watchlist / option chain

### 2.1 — HIGH — the pane header painted every symbol twice

`painter_pane.rs:889`. `SPY` rendered as `SPYY` with a ghost trailing the last
letter — two `painter.text` calls 0.5px apart, a faux-bold double-strike from
before the mono family had a bold face. It does not read as bold; on a HiDPI
window it reads as a doubled glyph.

Confirmed identical in `01-default`, `05-scanner` and `09-dom-sidebar` — a
permanent double-draw, not a transition artifact. Two agents flagged it
independently; it had been looked straight at several times without being seen.

**Fixed** — single draw. Weight belongs to the font, not to the number of times
we draw.

### 2.0 — HIGH — the chain symbol box painted "SPY" twice

Distinct from §2.1, and found only once the tabs were captured correctly. The
`Input`'s **placeholder** and a **manual overlay** both drew the symbol —
different offsets, different fonts, grey under orange, half a character apart,
reading as a corrupted `SᴥPY` smear.

The overlay exists because the current symbol lives in `chain.symbol`, not in
the edit buffer. Both mechanisms were doing their job; nothing said which one
owns the unfocused-and-empty case. **Fixed**: placeholder only while focused,
which is exactly when the overlay stands down.

### 2.1b — HIGH — every dropdown caret was a tofu box

`select.rs`. The chevron painted `U+25BC` in the **proportional** family. Inter
does not carry Geometric Shapes, so every `Select` trigger in the app rendered
`□` where its caret belongs — including the option-chain expiry picker and the
DOM order-type picker, two of the most-used dropdowns here.

What kept it hidden: the carets immediately next to them look fine, because
those are icon-font glyphs rather than this codepoint. Confirmed on the
certified preset and **fixed** by painting in mono, which carries the shapes.

### 2.2 — HIGH — the chain's column headers ignore the column layout

`STK BID ASK OI` is painted as one crammed text run at the panel's left edge,
while the data columns (`767` / `3.30` / `3.33` / `0`) span the full panel
width. The drift grows across the row — the `OI` header sits roughly above the
BID data. The header row cannot be used to identify a single column.

It is also separated from its data by an intervening control row
(`Count / − 10 + / N M F`), so header and body are not even adjacent.

**Fixed (alignment).** The headers were a `ui.horizontal` of
`ui.allocate_ui(vec2(col_w, 14.0), ..)`. `allocate_ui` consumes only what its
CONTENT needs, not the size requested — so each header advanced by its own
label width (~26px for `STK`) instead of by its column width (44px+), and the
error compounded left-to-right. The data rows, meanwhile, advance
`x += col + gap` and paint directly.

**One grid described by two different layout mechanisms.** They agreed only by
coincidence, and did not. The header row now uses the same arithmetic the data
rows use, which makes the alignment structural rather than maintained.

The header/data ADJACENCY is still open: the header is emitted once above all
expiry groups, so the first group's control row sits between it and its data.
That is an ordering change, not an alignment one.

### 2.3 — MEDIUM — labels are clipped by the panel's left border

Three occurrences, all touching the border with zero padding: the top-level
`DTE` field label has lost its leading character, and the `0DTE` / `1DTE` group
headers touch the border directly. A green price badge in the same column is
mostly clipped outside the panel — only a ~20px sliver shows.

A systemic padding/clipping problem at that boundary, not three separate
mistakes.

**Fixed.** The chain body has no inner left margin, so a row starting at its
content edge sits directly under the panel's left border stroke. Applied a
shared `chain_row_inset()` at the three row starts. The proper fix is an inner
margin on the chain body container, but that arm spans ~700 lines and wrapping
it is a re-indent, not a change — the constant keeps the three sites from
drifting apart in the meantime.

### 2.4 — MEDIUM — a label that read as debug text (not a truncation)

`sel` floated unboxed in the DTE toolbar row. Reported as truncated; it was
not — the literal string was always `"sel"`. But sitting between a bordered
dropdown and the `Spread` chip, a lowercase three-letter fragment reads as
leftover debug text rather than a control, which is how it was reported and
how it looks.

**Fixed**: it is now `Select`. The word is short enough to spell.

### 2.5 — LOW — numeric columns are left-aligned

Consistent, so not an inconsistency — but for financial data, right-aligned
numerals are near-universal because they let the eye compare magnitude down a
column. A design call, not a bug.

### 2.6 — LOW — tab-bar divider appears once

A vertical rule sits between HEAT and SCAN but not between LIST/CHAIN or
CHAIN/HEAT.

---

## 3. Tool panels

### 3.1 — HIGH — RRG axis titles collide with tick labels, on both axes

The Y-axis title overlaps the `100` gridline label character-for-character
(rendering as `1M0m`); the X-axis `RS-Ratio` title does the same against its
`100` label. Reproducing identically on both axes points at a shared
axis-label layout routine, not an edge case.

**Fixed.** One uniform `margin = 20.0` was shared by tick labels AND axis
titles, and both drew into it. Because each title is centred on its axis and
the plot is forced symmetric around 100, the title landed precisely on the
`100` tick. Each gutter is now derived from the text that goes in it, with
ticks and titles in disjoint bands; the Y title moved to the head of its axis
(top-left), the one corner neither axis labels.

Took two passes: reserving one line height for the title band still left `Mom`
grazing `104`, because the topmost tick is drawn centred on the plot edge and
overhangs half a line upward.

### 3.2 — HIGH — auto-chart TUNING radio column breaks its left edge

Seven radio rows should share one left edge; five do. `sensitivity` sits ~35px
left of the column and `lookback` ~35px right of it, before `swing window`
snaps back. Two controls zig-zagging out of an otherwise ruler-straight list.

**Fixed, and they were not radio buttons.** They are `egui::Slider`s, whose
`.text(..)` label is positioned *after* a value box sized to the number in it.
`0.003`, `200` and `5` are different widths, so each label started at a
different x. Nothing was misaligned in the code — **the labels were being
positioned by their own values.**

Converted to the design-system `Slider`, which stacks the label above the track
and gives the value a fixed column, so every label shares one left edge
regardless of content. Eight raw `egui::Slider`s retired from this panel.

That conversion then introduced a *precision* defect: the DS slider's value
formatter hard-coded 2 decimals when no `step` was set, so `sensitivity`
(range `0.001..=0.02`) rendered as **`0.00`** across most of its travel. Fixed
by deriving decimals from the range, with integral sliders kept integral, and
guarded by four tests.

Worth naming as a pattern: swapping a component in is not free. "It looks
aligned now" is not the same as "it still says the right thing" — the same
shape as the DOM ladder's `L.8H` in §1.2.

### 3.3 — MEDIUM — object tree has two different empty states in one panel

`DRAWINGS` renders its empty state with a large icon, bold heading and subtext
at ~280px tall. `INDICATORS`, `OVERLAYS` and `WIDGETS` — same panel, same
zero-item state — render two bare lines of text at ~180px. Same condition, two
visual languages.

### 3.4 — MEDIUM — playbook leaves ~70 % of the panel as unstyled void

One card occupies the top ~330px; the remaining ~1500px is blank background
with no empty-state treatment and no "add another play" affordance. Legitimate
app state, bare presentation.

### 3.5 — LOW-MEDIUM — playbook label order contradicts its own slider

Labels read ENTRY / TARGET / STOP left-to-right; the bar beneath spans
STOP(45) → TARGET(60) with the thumb marking ENTRY(50). The arithmetic is
right, but the label directly above the bar's right end reads `STOP` while that
end *is* the target.

---

### 2.7 — HIGH — the scanner MOVERS row collided with "Configure filters"

`…RVOL Le[Configure filt]ers`, interleaved letter by letter. The chip row and
the trailing button were laid out in one wrapped row with the button nested
after the chips, so the chips consumed the width and the button's 110px minimum
painted over them.

**Fixed**, in three passes — each exposing the next layer, and worth recording
because only the third actually works:

1. Reserving the button's width first was necessary but **not sufficient** —
   egui does not clip an over-long row to its region, so the chips overflowed
   the remainder and collided anyway.
2. Adding wrap fixed the collision, but `horizontal_wrapped` **inherits the
   parent's direction**, and the parent is right-to-left (that is how the
   button pins to the right edge). The chips laid out backwards: `Active Losers
   Gainers MOVERS`. Collision gone, row still wrong.
3. Stating **both** explicitly — `left_to_right` + `with_main_wrap` — is what
   works.

The row was also hiding a chip: `Gappers` only became visible once it wrapped.

### 2.8 — HIGH — `order_entry_open` was a dead flag (partly fixed)

`order_entry_open` is declared on `Watchlist`, defaulted, mirrored into
`SidebarState` both ways, persisted, and reported by `/state.open_dialogs` —
and **nothing reads it**. Traced to GPU Milestone 2 (`e1ab6d15`, 2026-05-09),
which deleted the call to `show_order_entry_panel` and repointed the ORDER
button at multi-instance `Chart::floating_order_panes`, leaving the old flag
behind.

So for ~3 months `/state` advertised an `order_entry` dialog nothing could
render, while the order entry that *does* exist was not reported at all — the
rule from §0 broken in **both** directions at once.

**Fixed (observability).** `order_entry` is no longer reported;
`order_ticket.<pane>.<id>` is, emitted from the live `floating_order_panes`
list so it cannot outlive its renderer. `OpenOrderEntry`/`CloseOrderEntry` were
**removed rather than repointed** — keeping the names as aliases would preserve
the exact failure they caused. `AppCommand::SpawnOrderTicket` drives the same
construction the ORDER button uses. `CloseAllDialogs` now clears tickets too,
so a sweep cannot inherit one.

`show_order_entry_panel` and its module remain as superseded dead code —
deleting them is a separate call, not a styling fix.

**Still not capturable, and this is the interesting part.** The ticket spawns
and `/state` reports it, but it does not paint: `render_chart_pane`
early-returns at `core.rs:2053` when the pane has no bars (the "no data /
source unreachable" branch), and the floating-order render lives at
`core.rs:3405` — after it. This dev box is offline, so every pane takes that
branch.

That is worth knowing as **product behaviour**, not just a harness limit: an
open order ticket vanishes from the screen if the feed drops, while `/state`
still reports it open. Not fixed here — `core.rs` is sacred (ADR/perf), and
this is a trading-behaviour decision, not a styling one.

It also sharpens the limit of the capture script: its assertions verify
**state, not visibility**. Removing the dead flag closes the case where state
outlives its renderer entirely; it does not close the case where a live state
is conditionally unrendered.

### 3.6 — HIGH — a chart-layer line paints over the Settings modal

Verified by pixel sampling, not by eye: at `06-rrg.png (2101, 1200)` a green
line `rgb(29,67,44)` sits on top of the modal's near-black theme swatch. It
runs continuously the full height of the window and appears in all 13 original
captures. A modal is the topmost layer by definition; something was compositing
above it.

**Fixed.** It is the RIGHT-RAIL RESIZE GRIP (`right_rail.rs:207`) — not a chart
element, which is why it only appeared once a rail panel was open (`01-default`
has no line; `06-rrg` and `13-settings` do).

Two defects stacked:

1. **Wrong layer.** The grip is an `egui::Area` at `Order::Foreground` — the
   same order `ui_kit::Modal` uses. Within one order egui paints by creation
   sequence, so the grip won and drew through the modal. Background chrome does
   not belong on the modal layer; moved to `Order::Middle`, which still keeps
   it above the panel bodies so it receives its drag.
2. **Always-on at full strength.** The accent was painted unconditionally —
   `active` only chose between `alpha_solid` and `alpha_heavy`, both loud — so
   a full-height accent rule bisected the app whenever any rail panel was open,
   whether or not anyone was resizing. Idle now paints a border hairline;
   the accent is reserved for hover/drag. An affordance should announce itself
   on approach, not compete with the content it separates.

### 3.7 — MEDIUM — BUY/SELL are ellipses, FLATTEN/CANCEL are rounded rects

In one four-control row, two controls render as full ovals (a `Pill` radius
applied to a box tall enough that the radius consumes the whole height) and two
as rounded rectangles. Two shape languages side by side. Not fixed — this is a
design call about whether `RadiusTier::Pill` should clamp relative to height.

### 3.8 — MEDIUM — top nav overlaps at 1600px width

At a 1600×1000 viewport the top navigation items overrun each other
(`RRG`/`Journal`/`T&S`/`Indicator`/`Auto-Chart`/`Analysis` all collide).
Whether 1600px is a supported minimum is a product decision, but the failure
mode is overlap rather than wrap, scroll or overflow — so it degrades badly
rather than gracefully. Flagged, not fixed.

---

## 4. What is clean

Worth recording so effort is not re-spent here:

- Ladder PRICE column: decimal-aligned across ~50 rows, even row pitch.
- Option-chain data grid: consistent row heights, formatting, no clipping.
- Object tree section-header rhythm and shared left edges.
- Auto-chart LAYERS / METHODS checkbox lists and both segmented controls.
- RRG quadrant labels, scatter layout, two-column legend, TIME/TAIL sliders.
- Panel boundary between watchlist and indicators: clean, no bleed.
- Bottom tab bar (ORDERS / POSITIONS / ACCOUNT / ALERTS): even, clear active state.
- **Theme fidelity works.** Stacking the same toolbar region across all six
  design systems shows each following its own radius, accent and density
  tokens — the cascade is doing its job where it is wired.

---

## 5. The pattern across all of it

The design system's *vocabulary* is in good shape — colour, type and token
usage are consistent, and the six styles genuinely differentiate. Nearly every
defect above is a **relationship** failure instead:

- content wider than the slot it was given (§1.1)
- text wider than the clip rect (§1.2, §2.3)
- columns without gutters between them (§1.3)
- a badge and a header claiming the same pixels (§1.4)
- a highlight box not tracking the text it highlights (§1.5)
- a header row not tracking its own columns (§2.2)
- an axis title not tracking its tick labels (§3.1)

Every existing gate is a vocabulary check: it can confirm a value came from a
token, and cannot see two legal tokens producing an illegal *relationship*.
That is the same root cause behind the frozen-chrome defects fixed earlier
(`strip_fits_hero`, `toolbar_fits_controls`, `control_size_lint`), and it is
where the next round of gates belongs.

`Button::max_width` is the first structural fix of this class: it makes
"content must not exceed its slot" a property the widget enforces, rather than
a discipline every call site has to remember.

The two colour defects (§1.0, §2.1b) are a *second* relationship class —
**precedence**. Neither is an off-token value; both are two correct mechanisms
disagreeing about which wins:

- a recipe fill vs. a semantic tint → the recipe won, and buy/sell collapsed
- a glyph vs. the font family it is painted in → the family lacked the glyph

Both were invisible to every gate for the same reason: each individual value is
legal and on-token. This is the same shape as the `card` / `card.floating`
precedence bug fixed earlier in `PanelCard`, and it is now the third time
"which of these two mechanisms wins?" has produced a visible defect. It is
worth treating as a first-class design-system question rather than resolving it
case by case.

---

## 6. Fixed in this pass

| § | Defect | Where |
|---|---|---|
| 1.0 | BUY and SELL painted the same colour | `button.rs` — tint out-ranks an Accent recipe fill |
| 1.1 | BUY/SELL overdrew FLATTEN/CANCEL | `button.rs` `max_width`/`fixed_size` + `dom_panel.rs` |
| 2.1 | Pane header painted every symbol twice | `painter_pane.rs` |
| 1.2 | Ladder dropped the minus sign off deltas | `dom_row.rs` — clip + fit ladder + left-align Δ |
| 1.3 | Δ/BID and PRICE/ASK collided | `dom_row.rs` — per-cell clip + gutter |
| 1.4 | SIMULATED badge over the column headers | `dom_panel.rs` — own band |
| 2.0 | Chain symbol box painted "SPY" twice | `watchlist_panel.rs` — placeholder only when focused |
| 2.1b | Every dropdown caret was a tofu box | `select.rs` — mono, not proportional |
| 2.7 | MOVERS chips collided with "Configure filters" | `scanner_panel.rs` — explicit LTR + wrap |
| 2.2 | Chain headers drifted off their columns | `watchlist_panel.rs` — one layout mechanism |
| 2.3 | Chain labels clipped by the panel border | `watchlist_panel.rs` — `chain_row_inset()` |
| 2.4 | `sel` read as debug text | `watchlist_panel.rs` — `Select` |
| 3.6 | Rail grip painted through the Settings modal | `right_rail.rs` — Order::Middle + idle hairline |
| 3.1 | RRG axis titles collided with tick labels | `rrg_panel.rs` — derived gutters, disjoint bands |
| 3.2 | Auto-chart slider labels zig-zagged | `auto_chart_panel.rs` → DS `Slider`; range-aware precision |
| 2.8 | `/state` reported a dialog nothing renders | `dev_inspector/mod.rs` + `SpawnOrderTicket` |
| §0 | Dialog commands drove a phantom state | `AppCommand::SetDialogOpen`, `update_sidebar_state` |
| §0 | `/state` could not report the watchlist tab | `dev_inspector/mod.rs` |
| §0 | Captures written for states the app was not in | `scripts/ds-harness/capture_surfaces.py` |

All five design-system gates pass: design-system 603 (baseline 603),
style-migration, recipe adoption (18 consumers, floor 18), radius 91/91,
control-size 4/4.

## 7. Open — not fixed

order ticket unrendered on a bar-less pane (§2.8, **do this first**), §2.2 header/data adjacency, §2.5 numeric alignment, §2.6 tab divider, §3.3 object-tree empty states, §3.4
playbook void, §3.5 playbook label order, §3.7 mixed
button shapes, §3.8 top-nav overlap at 1600px.
