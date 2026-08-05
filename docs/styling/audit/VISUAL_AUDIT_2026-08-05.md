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

## 1. Order entry / DOM ladder — the highest-stakes surface

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

### 1.3 — HIGH — Δ and BID columns have no gutter; digits collide

Every row. `-595` + `703` renders as `-595703`; `+186`+`670` as `+186670`.
There is a divider between BID│PRICE and ASK│VOL, but **none** between Δ│BID or
PRICE│ASK, so both size columns run into their neighbour. On the densest
numeric surface in the app, size and delta cannot be read apart.

### 1.4 — HIGH — the "SIMULATED" badge is drawn over the column headers

The badge lands on top of the `PRICE` and `ASK` headers, smearing both
illegible. Two of five column headers are unusable, so the header row cannot be
used to disambiguate §1.3.

### 1.5 — MEDIUM — the size-flash highlight is one character wide

The rose highlight behind ask-size values is sized to roughly one glyph
regardless of the value: `1250` highlights only the `0`, `11K` only the leading
`1`. The bounding box is not tracking text width, so a single digit reads as
artificially bold against its neighbours.

### 1.6 — MEDIUM — the ladder overdraws its own bottom edge

The row below the last full row is clipped mid-glyph and its leading digit
bleeds down onto the `MKT` dropdown in the order-entry footer. The ladder is not
clipping to its rect.

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

### 2.2 — HIGH — the chain's column headers ignore the column layout

`STK BID ASK OI` is painted as one crammed text run at the panel's left edge,
while the data columns (`767` / `3.30` / `3.33` / `0`) span the full panel
width. The drift grows across the row — the `OI` header sits roughly above the
BID data. The header row cannot be used to identify a single column.

It is also separated from its data by an intervening control row
(`Count / − 10 + / N M F`), so header and body are not even adjacent.

### 2.3 — MEDIUM — labels are clipped by the panel's left border

Three occurrences, all touching the border with zero padding: the top-level
`DTE` field label has lost its leading character, and the `0DTE` / `1DTE` group
headers touch the border directly. A green price badge in the same column is
mostly clipped outside the panel — only a ~20px sliver shows.

A systemic padding/clipping problem at that boundary, not three separate
mistakes.

### 2.4 — MEDIUM — truncated label reads as debug text

`sel` floats unboxed in the DTE toolbar row with ~800px of dead space before
the `Spread` chip. Not a word in this context; reads as truncated or
leftover text.

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

### 3.2 — HIGH — auto-chart TUNING radio column breaks its left edge

Seven radio rows should share one left edge; five do. `sensitivity` sits ~35px
left of the column and `lookback` ~35px right of it, before `swing window`
snaps back. Two controls zig-zagging out of an otherwise ruler-straight list.

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
