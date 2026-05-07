# Cosmic-text swap plan

## Why
egui's text rendering uses grayscale antialiasing. On dark backgrounds
(Apex's primary mode), grayscale AA makes thin strokes look fuzzy —
especially at our small body sizes (11/13/15 px). cosmic-text supports
subpixel AA which gives crisper, sharper edges. Other gains:

- Real OpenType ligatures (e.g. `==`, `=>`, `!=` in JetBrains Mono).
- Proper kerning pairs for proportional fonts (Inter, Plus Jakarta).
- Italic / weight axis selection from a font family fallback chain.
- Complex script shaping (RTL, combining marks). Not load-bearing for
  Apex today, but a free win.
- BiDi reordering — useful if we ever surface symbol names in Arabic
  or Hebrew locales.

## Status
egui does not have a public API to swap its text shaper. `epaint::Fonts`
owns the layout pipeline, the atlas, and the galley cache. Three
options:

### Option A: Patch egui's font system
Fork egui, replace `epaint::Fonts::layout` to delegate to cosmic-text.
Heaviest lift; we maintain a fork against upstream. Most invasive but
yields uniform quality across the entire app — every `ui.label`, every
button, every tooltip benefits with zero per-call-site work. Cost:
roughly 2–3 weeks for a working fork, plus ongoing rebase maintenance
each egui release. Galley caching, glyph-rect math, text editing
(`TextEdit` cursor placement) all assume the egui layouter — those are
the trickiest to keep behaviour-compatible.

### Option B: Custom widgets opt in to cosmic-text
A new `PolishedLabel` widget paints text via a separate path that uses
cosmic-text + a private glyph atlas. Old egui text continues with
grayscale AA. Migrate widgets one at a time: titles, headings,
tooltips, modal text first; keep numeric tape / chart axis labels on
egui's atlas. Less risky, more incremental. Drawback: visible
inconsistency between widgets that have been migrated and those that
haven't (the eye picks it up at zoomed-in pixel level on dark
backgrounds). Also: two atlases, two layout caches, two sets of font
loading code.

### Option C: Wait for egui to support it
Track upstream egui issues — there's been intermittent discussion of
cosmic-text but no merged PR as of 0.31. Cheapest but indefinite.

## Recommendation
**Option B for v1.** Ship a `PolishedLabel` + `PolishedRichText` that
opt-in to cosmic-text rendering. Widgets where text quality matters
most (modal titles, panel headers, tooltips, the OHLC readout) migrate
first. Keep egui's default rendering for high-frequency dynamic text
(price ticks, chart axis labels, scrolling tape — those repaint
constantly and don't benefit from subpixel as much because the eye
can't catch sharpness gains at 60 fps).

If Option B succeeds and the inconsistency bothers us enough, escalate
to Option A with the Option-B atlas/layout code as the starting point
for the patched layouter.

## Plan

### Phase 1 — Scaffold (this PR)
- Add `cosmic-text` + `swash` deps. Pin specific versions.
- Create `widgets/polished_label.rs` with API stub.
- Wire into `widgets::mod` exports.
- Smoke test fn that paints a few labels to a panel.
- Document v1 limitation: glyphs go through egui's atlas, so subpixel
  AA is degraded to grayscale at the atlas boundary. We still get
  cosmic-text's better shaping (ligatures, kerning).

**Estimate:** 1 day. **Risk:** dependency conflict between cosmic-text
0.12+ (which pulls fontdb / rustybuzz / yazi) and egui 0.31's
ahash/etc. transitive deps. If conflict found, stub the widget
delegating to `Label::new` and keep the API in place so call sites
don't change later.

### Phase 2 — Real subpixel pipeline
- Build an `egui_wgpu::CallbackTrait` callback that owns its own
  cosmic-text `FontSystem` + `SwashCache` + a wgpu texture atlas.
- Render glyphs at fractional positions; bypass egui's texture upload
  path so subpixel AA survives.
- Cache shaped buffers per (text, size, weight, family) tuple. Bound
  the cache (LRU, ~1000 entries) — `cosmic_text::Buffer` is heavyish.
- Hit-testing for click/hover: cosmic-text gives back glyph rects per
  cluster; map back into `Response`.

**Estimate:** 2–3 weeks. **Risk:** hit-testing edge cases
(BiDi, soft hyphens), wgpu pipeline state churn between egui's main
pass and our callback pass, atlas eviction policy.

### Phase 3 — Migration of high-value widgets
- modal titles → `PolishedLabel`
- panel headers → `PolishedLabel`
- tooltips → `PolishedRichText`
- alert / toast titles → `PolishedLabel`
- account summary header → `PolishedLabel`

**Estimate:** 3–5 days, mostly mechanical. Visual review of each
migration.

### Phase 4 — Decide on Option A escalation
After Phase 3 ships, gather screenshots A/B. If the inconsistency
between migrated and non-migrated widgets is glaring on real monitors
(not just at 4× zoom in screenshots), bite the bullet and fork egui.
Otherwise ship what we have.

## Risks
- **Atlas memory.** Two atlases ≈ 2× VRAM for glyph caching. At our
  body sizes that's roughly 4 MB → 8 MB. Acceptable.
- **Font loading duplication.** Both atlases load the same 6 TTFs.
  Mitigate by reading the bytes once and `Arc<[u8]>`-sharing.
- **Ligature surprises.** JetBrains Mono ligates `>=`, `->`, `=>` —
  these may visually alarm traders reading P/L formulas. Audit before
  migrating numeric widgets. Easy escape hatch:
  `cosmic_text::Attrs::cx_size_features` to disable `liga`.
- **Subpixel AA on rotated text.** Chart axis labels rotated 90° lose
  the subpixel benefit anyway — gives us cover for keeping them on
  egui's path.
- **Windows ClearType bias.** Subpixel AA assumes RGB stripe order;
  some users have BGR panels or vertical-stripe portrait monitors.
  cosmic-text does not autodetect; we'd need to expose a setting or
  accept the small minority case.

## v1 honesty
The Phase-1 widget routes glyph bitmaps through egui's grayscale atlas.
This means we get *shaping* improvements (ligatures, kerning) but
**not** subpixel AA in v1. The "polished" branding is aspirational
until Phase 2 lands. Marking it Phase-2 explicitly so we don't deceive
ourselves about what shipped.
