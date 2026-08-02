# 08 — Design-System Architecture Audit

**Date:** 2026-08-02
**Method:** six parallel read-only audit agents over `src-tauri/src` (197k lines), each
grounding every claim in file:line evidence and hard grep counts. Full agent reports are
summarised here; where a number matters it was measured, not estimated.
**Reference model:** the ApexTerminalThemes React port — which achieved ~90 % theme
fidelity and whose `tokens.ts` declares itself a *"1:1 mirror of Rust `style.rs`"*. The
audit question was: **where does the mirror break?**

---

## 0. Scorecard

| Dimension | Score | One-line verdict |
|---|---|---|
| **Cascade / inheritance** | 3 / 10 | 16-tier text cascade built and tested; 26 % adoption; `ui_kit` architecturally excluded from it |
| **Utility layer (Sx / recipes)** | 3 / 10 | Colour sub-layer thoroughly adopted (386 sites); `Sx` paints ~10 boxes vs 713 hand-painted; recipes resolve to no-op in every shipped config |
| **Layout (flex / grid)** | 3 / 10 | Real Taffy binding shipped with grid compiled in; 10 call sites vs 1,365 hand-geometry; intrinsic sizing broken |
| **Component model** | 5.5 / 10 | Real contract, Storybook binary, controlled state — but 4 theme-delivery mechanisms and 20+ variant vocabularies |
| **Resolution (sources of truth)** | 10 sources (target: 1) | The canonical `DesignSnapshot` is **not in the runtime path at all** |
| **Bypass (literals)** | Colour ~solved · geometry weak | 1.9 % theme-blind colour, 97 % token typography; the gap is strokes/radii/frames, 46 % of it in the sacred file |

### The unified diagnosis

Every dimension shows the **same failure shape**:

> **The right architecture was built — often explicitly to realise the React vision — and
> then adoption stalled between 1 % and 30 %, while the previous generation was never
> deleted.** The result is not a missing design system but four to six coexisting ones per
> concern, which can and do disagree.

The React port worked because it had **one mechanism per concern with no alternatives**:
one token store (`var(--ds-*)`), one theme switch (`data-ds`), one override layer
(259 `[data-ds]` rules), one derived-state idiom (`color-mix`), one variant style
(pure token functions). The Rust app has, respectively: **6 dimension stores, 4 colour
paths, a dormant recipe layer, 4 interaction-state systems, and 20+ variant vocabularies.**
Fidelity is capped not by what cannot be expressed but by which of several disagreeing
resolvers a given pixel happens to route through.

---

## 1. The resolution graph — 10 sources of truth

*(Agent 5; the keystone finding.)*

### 1.1 The headline

**`design_system/snapshot.rs::DesignSnapshot` — the documented join of
`ColorScheme × StyleSystem`, 165 fields, with a 1,072-line equivalence-test suite — is
dormant.** Its only non-test caller is `ThemeRegistry::snapshot()`, and `live_registry()`
has zero consumers outside `design_system/`. The design system the docs describe **is not
in the render path.** The real per-frame token struct is `ui_kit::style::TokenSnapshot`
(52 fields), built by `chart/renderer/ui/style.rs::begin_frame()` from the *legacy*
`StyleSettings` — of whose 99 fields only **22** flow through; the other **77 are read
live via `current()` at ~187 call sites**, and **30 of TokenSnapshot's fields are
independent literals**.

### 1.2 The live sources

**Dimension axis (6):**
1. `STYLE_STORE[ACTIVE_STYLE]` → `StyleSettings` — the de-facto master (99 fields, 187 direct reads)
2. `FRAME_TOKENS_LOCAL` → `TokenSnapshot` (22 derived + 30 literal fields)
3. `egui::Style` — **written twice per frame** from #1 and #2; second write wins
4. hot-reload `THEME_OVERRIDE` — carries **radii + strokes only**; invisible to #1 and #3
5. `DESIGN_TOKENS` (`dt_*!`) — compiled out of release builds entirely
6. User scalar overrides (`corner_scale`, `border_weight`, `effective_density`) — applied **only** in #2's accessors

**Colour axis (4):**
7. `LIVE_THEMES` (per-pane `theme_idx`)
8. ambient `PortableTheme` — per-style methods fall back to **trait defaults**
9. ambient `Theme` — same egui Id, different TypeId; per-style methods read `current()` live
10. `egui::Style.visuals` — reflects the **active pane only**

**Dormant (4):** `DesignSnapshot` · `ThemeRegistry`/`ActiveTheme` · `RecipeSet` (1 call site) · `gpu::THEMES` const (16 entries, `cfg(test)`).

### 1.3 Traced disagreements (each one is a visible defect class)

**Radius.** Three resolvers per frame: `radius_sm()` (hot-reload ▸ dt ▸ StyleSettings,
× CornerScale) · `foundation::shell::Radius::corner()` (raw `current()`, **skips both**
hot-reload and CornerScale) · `apply_ui_style` (raw, feeds every egui-native widget).
With CornerScale = Sharp, a `ui_kit::Button` renders square while the `RowShell` and every
combo box beside it stay rounded.
*Correction to `05-TOKEN-SURFACE-REFERENCE.md` §6:* the `shell.rs:18-21` "fixed 999.0"
comment is now **stale** — the base pill value is synced at runtime — but the split-brain
is real in a different form: divergence under CornerScale and hot-reload, plus **four
different pill defaults** across layers (999 / 99 / 9999 / 999).

**Spacing — the biggest inert axis.** `StyleSystem::Spacing.xs..xxl` is authored per
style and **never reaches `gap_*()`** — the adapter diverts `spacing.md/lg` to card
padding and drops the rest, so `gap_md()` is a hard 12.0 on every style. The
density/whitespace half of the style axis is inert. Meridien's airier spacing cannot
currently be expressed by authoring tokens.

**Typography — the inverted ladder.** Two font stores: `TokenSnapshot.font_*` (literals
9/10/12/14/16/22) and `StyleSettings.font_body/caption/section_label/hero` (per-style).
On Aperture, `font_body = 11` while `font_sm() = 12`, so **`TextStyle::Body` renders
smaller than `TextStyle::BodySm`** — and no single edit can fix it because the rungs live
in different stores. Separately, `ui_kit::Header` hardcodes monospace while
`section_header_mono()` exists to decide exactly that — and `PanelSection` honours it, so
two headers in one panel disagree about font family.

**A live light-theme bug.** `apply_ui_style` (`style.rs:2942`) writes popup/window
shadows with **hard-coded `Color32::BLACK`**, clobbering the `t.shadow_color`-aware
shadows set 100 lines earlier in `setup_theme`. Direct violation of CLAUDE.md rule 2;
breaks all five light palettes today; one-line fix.

**The adapter is lossy — why theme packs "don't do anything".**
`style_system_to_style_settings` silently drops: **all 20 `Alphas`**, `spacing.xs..xxl`,
`strokes.medium/md`, `typography.mono_*`, **all of `Elevation`**, 3 of 4 `Shadows` roles,
`radii.none/full`. `color_scheme_to_theme` drops `success/danger/warning/info/
pane_gap_color`, and the reverse map hardcodes them to `None` — **pack round-trips destroy
authored data.**

**Per-pane theming is half-honoured.** Pane *bodies* follow their own `theme_idx`, but
every ambient read, every egui-native widget, and every popup follows the **active pane**.
An inactive Bauhaus pane next to an active Vesper pane renders a light chart body with
dark buttons. And `theme_pack_bridge` activates packs against **pane 0 only**.

**Two ambient themes.** `setup_theme` stashes both a `PortableTheme` and a `Theme` under
the same egui Id (different TypeId). Their per-style methods resolve differently
(`PortableTheme` falls to trait defaults — e.g. `cards_float() = true`), so `PanelCard`
floats or doesn't depending on **which object type reached it**.

---

## 2. Cascade — built, tested, 26 % adopted, structurally fenced off

*(Agent 1.)*

- **374 of ≈1,431 text-emitting sites are cascade-aware** (26 %): 122 `as_rich_cascading`
  + 236 via six tier-wrapper widgets + 16 painter `font_id_in`. 527 `RichText` chains
  still bake `.size()`.
- **The structural blocker:** `TextStyle` lives in `chart/renderer/ui/foundation/`, and
  the dependency direction (`chart` → `ui_kit`) forbids `ui_kit` from importing it.
  **All 95 `ui_kit` files: zero cascade participation.** A subtree override of
  `apex.Body` is invisible to every design-system widget. The cascade cannot reach the
  components until the tier enum moves down into `ui_kit`.
- **Painter text is 3.3 % covered** — 485 `painter.text()` sites, 16 cascade-aware. This
  is where the densest surfaces live (lists 49:1 against).
- **Colour is never inherited.** Even the cascade-aware call *requires* a colour argument
  at 100 % of sites; 618 signatures thread a theme param; the ambient accessor (97 reads)
  is a Context-global singleton, not a stack — Theme Studio must manually set/restore it
  around its preview.
- **Exactly one production subtree override exists** (`chart_controls.rs:88-97`), and it
  works — proof of mechanism, absence of ergonomics. `ui.scope()` appears 3× app-wide;
  spacing has no push/pop primitive (21 hand-rolled save/restores); and the **cascade
  root itself is off-token** (`gpu.rs:5416-19` hardcodes `item_spacing`, `button_padding`,
  `menu_margin`, `interact_size`).
- Bright spot: only **6 literal `FontId` sizes remain app-wide** — the token migration
  succeeded; delivery is what's broken.

---

## 3. Utility layer — a prototype with one production-grade sub-layer

*(Agent 2.)*

- **`Sx` expresses 4 of ~30 Tailwind property families** (fill, border, radius, padding —
  and padding/gap/text are honoured only by `show_ct`, used once). No shadow, no focus
  state, no per-corner radius, no font family/weight, no transition control. `opacity` is
  a dead field.
- **Adoption: ~10 Sx-painted boxes vs 713 `rect_filled`/`rect_stroke`** (≈1.4 %).
  8 of 75 widget files touch Sx at all. Where they do, most use it as a *lookup* —
  resolve, extract one field, paint by hand — with the `match fill` unwrap boilerplate
  copy-pasted 3× for want of a `fill_color()` helper.
- **The colour sub-layer is genuinely adopted**: `Tone × Shade` ramps + `palette_ct` at
  **386 call sites** (359 in ui_kit). The Tailwind-shade vocabulary works. The project's
  own reporting conflates this success with Sx-the-painter's failure.
- **`RecipeSet` — the `[data-ds]`-rules equivalent — is dormant**: 4 widgets consult it
  (6 of 28 registered keys), **zero recipe data ships**, every constructed set is empty,
  `resolve_cached` has no callers, and `Button` (532 `Variant::` refs) receives the
  RecipeSet in its `StyleCtx` and never reads it. `gpu.rs` carefully protects a pack's
  RecipeSet from being clobbered — protecting a value nothing reads.
- **Four interaction-state systems**: Sx states (0 users) · `apply_interaction` (2 call
  sites serving 7 row types, all setting only `.selected`; file is marked
  `#![allow(dead_code)]`) · `button_style.rs` tables (buttons only; its own header says
  "the cva/shadcn idea adapted to immediate-mode") · **196 hand-rolled `.hovered()`
  sites**. Hover feel — a large part of theme personality — is unthemeable by
  construction.
- **The named guardrail measures the wrong thing**: `sx_ratchet.sh` greps a legacy
  colour-call pattern, currently fails at 18 vs baseline 4, and is wired into no CI.
  **No metric anywhere measures actual Sx or recipe adoption.**

---

## 4. Layout — the engine shipped; the migration didn't

*(Agent 3.)*

- **`flex.rs` is a real Taffy 0.12 binding** — justify/align/grow/shrink/wrap/gap,
  f32-accurate padding, pure `solve()`, 18 headless unit tests. **The `grid` feature is
  compiled into the binary and never used.** `docs/UI_WORKFLOW.md` still describes Taffy
  in the future tense.
- **Adoption: 10 production flex containers** across 7 files, vs **1,365 hand-geometry
  sites** (excl. the sacred file) and 672 `ui.horizontal`-family calls. Ratio ≈ 137 : 67 : 1.
- **Why it stalled — one specific defect:** `Size::Auto` resolves to **0** (no intrinsic
  sizing; pinned by its own test). Every content-sized child must be hand-measured into
  `Item::fixed`, so migration costs more code than the arithmetic it replaces. Fix =
  Taffy `MeasureFunc` bridged to egui galley measurement.
- **`ui_kit/widgets/` is the worst offender** — 385 manual-geometry sites vs 49 layout
  calls, including `pane_grid.rs` computing `rect.left() + gap_xs()` inside the layout
  primitive itself.
- **No grid model.** The three grid-ish things (binary split tree, depth-capped 8 · a
  Full/Half rail packer · a uniform tiler with the app's only breakpoint) cannot express
  spans. **Aperture's 12-col × 92 px mosaic is inexpressible today** while the engine to
  express it sits compiled in the binary.
- **Structure isn't themeable**: `gap_*()` respects the spacing scale; `row_height_*`
  (18/20/22/24/30), `HEADER_H=28`, `TILE_GAP=6.0`, splitter width, `Width::{240,300,400}`
  do not. Themes can breathe gutters, not proportions. The editorial `300px/1fr/360px`
  needs a **root shell solve** that has no home — the shell is independent `SidePanel`
  reservations.
- Found bug: `surface.rs:172-174` infers padding from the first child's left edge —
  wrong under `Justify::Center` and for columns.
- Positioning is ~17 % tokenized vs spacing ~60 % — tokens reach the flow API
  (`add_space`) but not the geometry API, because there is no container to hand a token to.

---

## 5. Component model — the healthiest layer, uneven by construction

*(Agent 4. Scale correction: `ui_kit/widgets/` = **75 files / ~78 primitives**, not ~95.)*

- **Four theme-delivery mechanisms:** `show(ui, &dyn ComponentTheme)` (58 files) ·
  generic `&T` (11) · **`impl Widget` + ambient (28 widgets — a theme-free second entry
  point per widget)** · `StyleCtx` (2: Button, Tabs). 60 of 62 widgets internally build
  `StyleCtx::from_theme` with a **static empty RecipeSet** — theme-pack recipes can only
  ever reach Button and Tabs.
- **`Variant` is consumed by 3 of 62 components** (and `Progress` handles 2 of its 13
  arms). 20+ private tone/variant vocabularies fill the gap; `shell_variants.rs`'s five
  enums are consumed by zero ui_kit widgets. `Button` retains 9 colour escape hatches.
  There is no single lever for "restyle Danger everywhere."
- **The React mirror is dangling:** `CardSlots.tsx` still declares `Mirror target: Rust
  ui_kit/widgets/card_slots` — deleted as orphaned. Seven card types coexist; the only
  slotted card (`CardShell`) lives outside ui_kit, stores a concrete `&Theme`, has 1
  caller.
- **Slot signatures split 50/50:** half the body closures are `FnOnce(&mut Ui)` (theme
  dropped — including `Modal` and `ToolOverlay`), half are `FnOnce(&mut Ui, &T)`.
  Everything nested in a dialog falls back to ambient — this is how mechanism-drift
  propagates into every modal.
- **The documented contract cites a `Density` enum that does not exist** — density is a
  process-wide global; two densities cannot coexist in one frame (blocks Mariner's
  "10 % tighter" as a scoped property).
- Genuine strengths: `#[must_use]` builders, controlled state as the norm (3
  memory-stashing widgets), dead legacy shims verified at 0 callers, **a real Storybook
  binary** (`apex-playground`, 13 story modules, 4 themes) plus an in-app gallery — though
  14 primitives have no story, including all four panel-state widgets.
- Contract details drifting: 6 widgets return `()` (dead-end composition), 4 don't take
  `ui`, `Sparkline::show` ignores its theme argument, `DialogHeader`'s *default* entry
  point is the ambient one.

---

## 6. Bypass — the literal-hunting era is nearly over

*(Agent 6. This agent **overturned** the pessimistic priors.)*

- **The ratchet works.** Baseline 903 → **516 (-43 %)**, tracked through git history.
  *(Corrects `00-START-HERE.md` / `01-UI-ARCHITECTURE.md`, which cite 903 as current.)*
- **Colour is ~solved:** ~4,700 token-driven colour expressions vs ~92 real theme-blind
  production literals (**1.9 %**). `Color32::RED`-style constants: 2 hits app-wide.
- **Typography is 97 % token-adopted** — 29 literal sizes left, ≈1 hour of work.
- **The remaining bypass is geometry:** 158 literal stroke widths + 163 literal
  `rect_filled` radii (the gate's admitted blind spot — needs an AST lint) + 63 raw
  frames. **Frames are the worst-adopted primitive: 25 helper calls vs 63 raw (28 %).**
- **Concentration, not diffusion:** the sacred `core.rs` holds **46 %** of all bypass —
  its *colour* is healthy (2,773 theme-token refs), its *geometry* (124 strokes, 57 radii,
  42 gammas) is frozen. That is the known fidelity ceiling for the chart canvas. The
  other 54 % sits in ~15 identifiable files with an effort-ranked top-10 (pane header →
  top_nav → watchlist pair → shared shells → DOM pair → screener buttons → stroke sweep).
- **Only 9 non-core literal `gamma_multiply` sites** stand between the light palettes and
  correct hover/pressed states outside the canvas — one afternoon.
- Housekeeping: `tps_overlay.rs` (fake-Excel boss key) and `bug_anchor.rs` are
  *intentionally* theme-blind → exempt them (516 → 479). CLAUDE.md's "~2 intentional raw
  buttons" is stale — it's ~11, five in `screener_heatmap.rs`.
- **Theme-responsiveness estimate:** off-canvas chrome **~92–95 %** colour-responsive;
  on-canvas ~85 % colour / ~5 % geometry; weighted by what a user notices on theme
  switch: **~90 %.**

---

## 7. Corrections this audit makes to earlier package documents

| Doc | Stale claim | Corrected |
|---|---|---|
| `00`/`01` | "903 violations across 127 files" | 516 / 108 and falling; ratchet is working |
| `05` §6 | `radius_pill()` "fixed 999.0" split-brain as described | Base value now synced; the split-brain survives as CornerScale/hot-reload divergence in `shell::Radius::corner()` + `apply_ui_style`, plus 4 conflicting defaults |
| `01` §3.1 | Registry/snapshot presented as the canonical runtime path | **Dormant.** Runtime path is `StyleSettings → begin_frame → TokenSnapshot` + 187 direct `current()` reads |
| `02` (rev 2) | Token gaps framed as mostly *missing fields* | Compounded by a **lossy adapter**: many "existing" fields (all Alphas, Spacing.xs..xxl, mono_*, Elevation, 3 shadow roles) are authored and then **dropped before render** |
| `src-tauri/CLAUDE.md` | "~2 intentional `egui::Button` sites"; `Density` enum in tokens.rs | ~11 sites (5 in screener_heatmap); no `Density` enum exists |
| `docs/UI_WORKFLOW.md` | Taffy "is the realistic route" (future) | Shipped at `ui_kit/layout/flex.rs`, grid feature compiled in, 10 call sites |
| `ui_kit/widgets` "~95 files" (my earlier docs) | | 75 files / ~78 primitives |

---

## 8. What the React mirror proves, precisely

The React port hit ~90 % with a token vocabulary explicitly mirrored from `style.rs`.
Combined with Agent 6's finding that colour and typography are already ~solved on the
Rust side, the fidelity gap decomposes cleanly:

| React mechanism | Rust state | Gap type |
|---|---|---|
| 1. `var(--ds-*)` everywhere | Colour ✅ 98 % · type ✅ 97 % · **geometry ❌ (strokes/radii/frames/spacing)** | adoption |
| 2. One `data-ds` swap | `activate_theme_pack` is the only all-axes path — and it's lossy + pane-0-only | **plumbing bugs** |
| 3. 259 `[data-ds]` rules | `RecipeSet` dormant: no data, 4 consumers, Button opted out | **dead layer** |
| 4. `color-mix` derived states | `Tone×Shade` ✅ adopted — but 196 hand-rolled hover sites choose *when/what* to derive | half-adopted |
| 5. Variants = pure token fns | 3 of 62 widgets on `Variant`; 20 private vocabularies | fragmentation |
| (enabler) flexbox | Engine shipped; 10 call sites; intrinsic sizing broken; no grid wrapper | **one defect + migration** |
| (enabler) cascade | Built + tested; ui_kit fenced out by dependency direction | **one structural move** |

Every gap is a *delivery* problem. None is a vocabulary problem. That is exactly what the
mirror predicted, and it is why the fix plan in
[`09-DESIGN-VISION.md`](09-DESIGN-VISION.md) is convergence and deletion, not construction.
