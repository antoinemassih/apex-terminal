# Style System Migration Brief — Parallel Work Streams

**Goal:** evolve the two-axis design system (ColorScheme × StyleSystem) into a fully data-driven shell-theming platform — VSCode/JetBrains-grade, where a "theme pack" (colors + scales + component recipes + shell profile) can restyle navigation, buttons, boxes, spacing, and chrome **without editing Rust**.

This brief is written for agents. Each stream is self-contained, has exclusive file ownership, step-by-step tasks, and acceptance criteria. Read `src-tauri/CLAUDE.md` before touching anything — the hard rules there (sacred `core.rs`, frozen `Watchlist`/`Chart` god-objects, token discipline, light-theme parity) apply to every stream.

---

## 0. Current-state summary (read this first)

### What exists and works
- **Axis 1 — `ColorScheme`** (`src/design_system/color_scheme.rs`): pure palette, 15 built-ins, DTCG JSON round-trip. Solid.
- **Axis 2 — `StyleSystem`** (`src/design_system/style_system.rs`): Typography/Spacing/Radii/Strokes/Alphas/Elevation/Density/Shadows/Treatments/Chrome. 9 built-in personalities (Meridien, Aperture, Octave, Cadence, Alto, Mariner, Lucid, Relay, Glass). Solid schema, **incomplete coverage**.
- **`ComponentTheme` trait** (`src/ui_kit/widgets/theme.rs`): the widget-facing color contract. `&THEMES[0]` purge complete. `PortableTheme` proves portability.
- **`Sx` layer** (`src/ui_kit/sx/style.rs`): Copy-only utility style values with per-state deltas (hover/active/disabled), token-tier builders, eased state blending. This is ~90% of a recipe spec already.
- **Per-frame snapshots:** `ui_kit::style::TokenSnapshot` (thread-local, live) and `design_system::snapshot::DesignSnapshot` (the intended superset).
- **User overrides:** CornerScale / BorderWeight / SpacingScale / MotionSpeed / Density global multipliers applied at token read sites.

### The problems (what each stream fixes)
| # | Problem | Stream |
|---|---------|--------|
| P1 | `StyleSettings` (`src/chart/renderer/ui/style.rs`, ~100 fields, 3,045-line file) is still the runtime carrier of deep-style knobs. The adapter `style_system_to_style_settings()` carries only ~20 fields from `StyleSystem`; the other ~80 fall back to `style_defaults(base_id)` — a hardcoded 9-arm Rust match. New shells cannot be loaded from data. | S1 + S2 |
| P2 | Three parallel token structs (`StyleSettings` / `TokenSnapshot` / `DesignSnapshot`) kept in sync manually — drift has already happened (see "P2.2 aligned…" comments). | S2 |
| P3 | `Treatments` grows one flat field per design idea; component-specific knobs (`wl_row_*`) leak into the global axis. O(fields) plumbing per new style idea. | S4 |
| P4 | Split-brain theme object: `ComponentTheme` answers colors from the passed object but dimensions from the global thread-local. Two differently-styled shells cannot render in one frame (no theme-preview gallery possible). | S5 |
| P5 | `ui_kit` is not portable: 48 `chart_renderer` reach-ins; `sx/style.rs` hard-aliases `type Theme = crate::chart_renderer::gpu::Theme`. | S3 |
| P6 | Lossy `u8` enum indices cross the snapshot boundary (`panel_tab_treatment: u8`, `button_treatment` in some paths, `pane_active_indicator: u8`, `panel_header_treatment: u8`). | S3 |
| P7 | Shell *structure* (nav layout, dock arrangement) is hardcoded; only skins vary. | S6 |
| P8 | `pane_gap_color: Option<Color32>` sits on the dimension axis — a color/dimension separation violation. | S1 |

### Target architecture (end state)
```
ThemePack (loadable from JSON/TOML, hot-reloadable)
├── ColorScheme        — palette (exists, done)
├── StyleSystem        — scales + global treatments (S1 completes it)
├── RecipeSet          — per-component Sx-style overrides, keyed
│                        "button.primary", "tab.active", "row.list", … (S4)
└── ShellProfile       — structural layout variants: nav style, dock
                         positions, rail order (S6)

Runtime:
└── StyleCtx<'a> { theme, snapshot, recipes }  — threaded into widgets (S5)
    └── thread-local snapshot kept as compat shim only
```
Resolution chain (VSCode model): **component recipe → semantic token → base token**. A missing recipe key falls through; nothing breaks.

> **Two parts.** Streams **S0–S6 build the themable *engine*** (Part 1, below). Streams **S7–S11 build the theme *platform*** — assets, packaging/install, validation, a standalone authoring app, and the consumer-facing settings surface (Part 2, after the per-agent template). The engine is necessary but not sufficient for "a proprietary theme installation system"; you need both. Part 2 has its own intro, wave graph, and integration checklist.

---

## Coordination rules (ALL streams — read carefully)

1. **One branch per stream**, branched from `main` (or the current integration branch — confirm with the user). Name: `style-mig/s1-schema`, `style-mig/s2-adapter`, etc.
2. **Exclusive file ownership.** Each stream may only WRITE the files listed in its "Owns" section. Reading anything is fine. If you believe you must edit a file owned by another stream, STOP and surface it — do not edit. (Concurrent sessions editing shared files have caused stash-loop wars in this repo before.)
3. **Shared files needing coordination:** `src/ui_kit/widgets/mod.rs`, `src/design_system/mod.rs` (re-export lists). Keep edits to these append-only and minimal; merge conflicts there are cheap to resolve.
4. **Never touch:** `src/chart/renderer/render/pane/core.rs` (sacred GPU path). Never add fields to `Watchlist` / `Chart` (frozen — use `ChartState` / `state/` aggregates per `docs/STATE_SYSTEM.md`).
5. **Every stream's definition of done includes:** `cargo check` clean, existing tests pass, `equivalence_tests.rs` pass (where applicable), and a walk-through in a light theme (Bauhaus) for any visual change.
6. **No visual regressions by default.** Wave 1–2 work is plumbing: pixel output must be identical before/after (the equivalence-test pattern in `src/design_system/equivalence_tests.rs` is the template — extend it, don't weaken it).

### Dependency graph / waves (full program, both parts)
```
WAVE 1 (fully parallel):  S0  S1  S3  S6-design  S7  S11-decision
WAVE 2 (after deps):      S2 (needs S1)  S4 (needs S3)  S5 (needs S3)
                          S3-crate-extraction (mandatory — unblocks S10)
WAVE 3 (engine integ.):   S2-final deletion · S4 widget adoption ·
                          S5 preview gallery · S6 implementation
WAVE 4 (platform):        S8 (needs S1+S4+S7)  S9 (needs S8)
                          S10 standalone app (needs S3-crate + S5 + S4)
WAVE 5 (product):         S8 in-app Themes settings section ·
                          S11 authoring tools/docs · end-to-end pack demo
```

---

## Stream S0 — Guardrails & CI lints

**Goal:** make regressions impossible while the other streams work.
**Owns:** new `xtask`/script files, CI config, `docs/` additions. No production source edits.
**Depends on:** nothing. **Blocks:** nothing (but land it early).

### Tasks
1. Write a lint script (xtask or simple grep-based CI step) that fails the build on:
   - new `pub` fields added to `StyleSettings` (snapshot the current field list; diff against it),
   - new `crate::chart_renderer` / `crate::chart::renderer` imports under `src/ui_kit/` (snapshot current 48 sites; only allow the count to go DOWN),
   - new `&THEMES[0]` references (already zero — keep it),
   - new `Color32::from_rgba_unmultiplied(0, 0, 0,` shadow patterns outside the allow-list.
2. Add a counted-baseline file (e.g. `docs/migration/baselines.toml`) the script reads, so streams can ratchet counts down as they land.
3. Document the ratchet workflow in `docs/migration/README.md`.

### Acceptance
- CI/lint runs locally via one command; intentionally adding a `StyleSettings` field makes it fail.
- Baselines reflect the true current counts.

---

## Stream S1 — Complete the `StyleSystem` schema (field migration)

**Goal:** every one of the ~100 `StyleSettings` fields has a typed home in `StyleSystem` (or an explicit "moves to recipe — S4" / "app setting — not style" disposition). After S1, `StyleSystem` is the **complete** dimension-axis schema.
**Owns:** `src/design_system/style_system.rs`, `src/design_system/baseline.rs`, `src/design_system/builtin.rs` (StyleSystem entries only — coordinate if S2 needs the same file), `src/design_system/loader.rs`, `src/design_system/export.rs`, `src/design_system/equivalence_tests.rs`, `src/design_system/snapshot.rs`.
**Depends on:** nothing. **Blocks:** S2.

### Field disposition table (authoritative starting point — verify against code, some already exist)

| StyleSettings field(s) | Disposition |
|---|---|
| `r_xs/sm/md/lg/pill/chip` | `Radii` — exists ✓ verify mapping |
| `stroke_hair/thin/std/bold/thick` | `Strokes` — exists ✓ |
| `font_section_label`, `font_body`, `font_caption`, `label_letter_spacing_px`, `nav_letter_spacing_px`, `section_header_tracking` | `Typography` — mostly exists ✓ verify |
| `font_hero` | ADD `Typography.size_hero` |
| `row_height_px`, `density` | `Density` — exists ✓ |
| `button_height_px`, `button_padding_x`, `tab_height`, `cta_height_px`, `cta_padding_x`, `card_padding_x/y` | `Spacing` — partially exists; add `card_padding_x/y` |
| `shadow_blur/offset_y/alpha`, `card_floating_shadow_alpha` | `Shadows` — map into `ShadowSpec`; add floating-card spec |
| `serif_headlines`, `button_treatment`, `hairline_borders`, `solid_active_fills`, `invert_active_fill`, `uppercase_section_labels`, `vertical_group_dividers`, `show_active_tab_underline`, `inactive_header_fill`, `nav_buttons_label_only`, `nav_buttons_uppercase_labels`, `tab_underline_under_text`, `card_floating_shadow`, `shadows_enabled`, `animations_enabled`, `surface_bevel`, `bevel_highlight_alpha`, `bevel_shadow_alpha`, `section_header_mono`, `wl_symbol_mono`, `panel_tab_treatment`, `pane_active_fill_accent` | `Treatments` — exists ✓ verify each |
| `toolbar_height_scale`, `header_height_scale`, `account_strip_height`, `pane_border_width`, `pane_gap`, `pane_gap_alpha`, `pane_active_indicator`, `active/inactive_header_fill_multiply`, `header_outer_border_alpha/width`, `header_divider_alpha`, `nav_active_col_alpha`, `dialog_backdrop_alpha`, `tab_inactive_alpha`, `tab_hover_bg_alpha` | `Chrome` — exists ✓ verify each |
| `hover_bg_alpha`, `active_bg_alpha`, `focus_ring_width`, `focus_ring_alpha`, `disabled_opacity`, `accent_emphasis` | ADD to `Chrome` (interaction sub-group) or a new `Interaction` struct |
| `tab_underline_thickness`, `section_label_padding_top/bottom`, `toolnav_height`, `region_gap`, `region_radius`, `region_border_alpha`, `nav_cluster_radius`, `nav_cluster_fill_alpha`, `nav_cluster_padding`, `drag_handle_alpha`, `drag_handle_dot_scale`, `toast_bg_alpha`, `card_stripe_alpha`, `panel_header_treatment`, `panel_section_fill_alpha`, `panel_footer_card`, `panel_footer_radius` | ADD to `Chrome` **with a `// RECIPE-CANDIDATE(S4)` marker comment** — these are per-component and will migrate to RecipeSet later; for now they need a data home so S2 can kill the hardcoded match |
| `wl_row_side_margin`, `wl_row_corner_radius`, `wl_row_divider_alpha` | Already in `Treatments` ✓ — mark `// RECIPE-CANDIDATE(S4)` |
| `button_group` (`GroupEnclosure`) | `Chrome` — exists ✓ |
| `pane_gap_color: Option<Color32>` | **Axis violation.** Move to `ColorScheme` as `Option<Rgba>` field `pane_gap` (or resolve: derive from `bg` when None). Document the decision. |
| `footer_default_open` | **Not style** — this is app/session state. Move to a `state/` aggregate (NOT `Watchlist`/`Chart` — they're frozen). Coordinate with the user before moving. |

### Tasks (in order)
1. Audit: diff the table above against the real structs; produce the corrected table in `docs/migration/field-disposition.md`. This doc is the contract S2 builds against.
2. Add all missing fields to `StyleSystem` sub-structs with doc comments, serde defaults (so existing JSON keeps loading), and `Default` impls matching today's `style_defaults(0)` (Meridien baseline).
3. Update the 9 built-in `StyleSystem` constructors so each carries the values currently produced by `style_defaults(0..=8)` for the newly added fields. **Source of truth: the existing `style_defaults` match in `chart/renderer/ui/style.rs` — transcribe exactly, do not redesign.**
4. Extend `DesignSnapshot` + `snapshot()` resolver to carry the new fields.
5. Extend `loader.rs` / `export.rs` so the new fields round-trip through JSON/TOML.
6. Extend `equivalence_tests.rs`: for each style id 0..=8, every newly-migrated field on the constructed `StyleSystem` must equal the `style_defaults(id)` value.

### Acceptance
- `docs/migration/field-disposition.md` exists with 100% field coverage and zero "TBD".
- Equivalence tests prove the 9 built-ins are value-identical to `style_defaults(0..=8)` for all migrated fields.
- JSON export → import round-trips losslessly.
- No call-site behavior change (S2 does that).

---

## Stream S2 — Adapter completion & `StyleSettings` retirement

**Goal:** `StyleSystem` becomes the single runtime source; `StyleSettings` shrinks to a derived, then deleted, struct.
**Owns:** `src/chart/renderer/ui/style.rs`, call sites of `get_style_settings` / `current()` / `style_defaults` across `src/chart/` (NOT `core.rs`).
**Depends on:** S1 (field-disposition doc + schema). **Blocks:** final theme-pack loading.

### Tasks (phased — each phase is a safe stopping point)
**Phase 2a — total adapter.** Extend `style_system_to_style_settings()` to populate **every** field from the `StyleSystem` (per S1's disposition doc). Delete the struct-update fallback to `style_defaults(base_id)`. The `base_id` parameter becomes unused — remove it.
**Phase 2b — defaults become data.** `style_defaults(id)` now simply does `style_system_to_style_settings(&builtin_style_systems()[id])`. Delete the 9-arm hardcoded match (the values live in `design_system::builtin` after S1 step 3). Keep `style_defaults_pub` signature stable for now.
**Phase 2c — call-site flip.** Migrate readers of `StyleSettings` fields to read from `DesignSnapshot` / `frame_tokens()` instead. Mechanical, wide — do it in small commits grouped by directory. The 2 remaining `current()` reach-ins from `ui_kit` get removed here (coordinate with S3 if they're in S3-owned files).
**Phase 2d — delete.** When the lint baseline (S0) shows zero readers, delete `StyleSettings`, `STYLE_STORE`, `get_style_settings`, the adapter, and the "Contour" alias hack. `ACTIVE_STYLE` index selection moves to the design-system registry.

### Acceptance
- After 2a/2b: pixel-identical rendering across all 9 styles × at least 3 themes (spot-check Meridien/Aperture/Glass × Midnight/Bauhaus/Dracula). Equivalence tests green.
- After 2d: `grep StyleSettings src/` returns only history/docs. `style.rs` line count drops by ≥1,000.
- A brand-new `StyleSystem` JSON dropped in the user-themes directory produces a 10th selectable style **with zero Rust edits** (the real exit criterion).

---

## Stream S3 — `ui_kit` portability & typed boundaries

**Goal:** `ui_kit` compiles with zero `chart_renderer` references; enums cross the snapshot boundary typed, not as `u8`.
**Owns:** `src/ui_kit/**` (except `widgets/` files being actively edited by S5 — coordinate via the wave plan; in Wave 1 S5 hasn't started, so S3 has the directory), specifically: `sx/style.rs`, `sx/color.rs`, `sx/recipes.rs`, `style.rs`, `tokens.rs`, `cursor.rs`, `mod.rs`, `widgets/tokens.rs`, `widgets/theme.rs` and the listed reach-in files.
**Depends on:** nothing. **Blocks:** S4, S5.

### Tasks
1. **Kill the concrete alias.** In `sx/style.rs`, remove `type Theme = crate::chart_renderer::gpu::Theme`. Convert `Sx::show()`, `Sx::decorate()`, `Sx::paint_into()`, `palette()` to take `&dyn ComponentTheme` (the `palette_ct` path already exists — unify on it). Update call sites in `src/chart/` accordingly (these specific call-site edits are granted to S3 as an exception to file ownership — keep them mechanical).
2. **Sweep the remaining reach-ins.** Work through the 48 `chart_renderer` references in `ui_kit` (S0's baseline lists them). Each is either: (a) a type that should live in `ui_kit` or `design_system` — move it; (b) a value that should arrive via `TokenSnapshot`/`ComponentTheme` — reroute it; (c) genuinely chart-specific — the widget is in the wrong layer, flag it in the migration doc rather than forcing it.
3. **Type the snapshot enums.** `TokenSnapshot.panel_tab_treatment: u8` → `TabTreatment`; verify `button_treatment` is the typed enum everywhere; `pane_active_indicator: u8` and `panel_header_treatment: u8` get real enums (define in `design_system::style_system`, mirror into snapshot). Update `Treatments` (coordinate with S1: S1 owns `style_system.rs` in Wave 1 — agree the enum definitions land in S1's branch, S3 consumes them after S1 merges, OR S3 defines them and S1 rebases; pick one and record it in the migration doc).
4. **Radius fidelity (small fix).** `Sx::paint` casts radius `f32 → u8`, losing sub-pixel and >255 pill values. Use `CornerRadius` per-corner f32-faithful construction (clamp at 255 only where egui requires u8).
5. **Crate extraction — now MANDATORY (was stretch).** Move `ui_kit` and `design_system` into their own workspace crates with **zero dependency on the chart app**. This is the keystone the whole program leans on: the standalone Theme Studio app (S10) physically cannot link `chart_renderer`, so the kit must stand alone. Do this as a final phase of S3 after steps 1–4 are clean. Deliverable: a new workspace layout where `apex-ui-kit` + `apex-design-system` build as library crates, the Tauri app depends on them, and `cargo build -p apex-ui-kit` succeeds with no chart-app in the graph. If a true blocker emerges, STOP and escalate — do not ship a half-extracted crate; S8/S10 are gated on this.

### Acceptance
- `grep -rn "chart_renderer\|chart::renderer" src/ui_kit` → 0 hits (S0 ratchet to zero).
- No `u8` style-enum fields remain on `TokenSnapshot`.
- All widgets render pixel-identical (spot-check button/tabs/panel rows across 3 styles).

---

## Stream S4 — RecipeSet: serializable per-component styling

**Goal:** the VSCode move — component looks become keyed data, not struct fields. New deep-style ideas stop requiring schema changes.
**Owns:** new files `src/ui_kit/sx/recipe_spec.rs`, `src/design_system/recipes.rs` (+ loader extension hooks coordinated with S1's loader — by Wave 2, S1 has merged), `figma/` recipe token docs.
**Depends on:** S3 (portable Sx). **Blocks:** Wave-3 widget adoption.

### Design (build exactly this; raise deviations before implementing)
1. **`RecipeSpec`** — a serializable mirror of `SxDelta` + per-state overrides: fill (tone/shade/alpha/solid), border (spec + width tier), radius tier-or-px, padding tiers, text tone/size tier, opacity; `hover`/`active`/`disabled`/`selected` deltas. Colors expressed as **semantic tone references** (`accent`, `bull`, `text@muted`, …) — never raw hex (palette-independence is the whole point). Raw hex allowed only behind an explicit `literal:` prefix for escape hatches.
2. **`RecipeSet`** — `HashMap<RecipeKey, RecipeSpec>` with namespaced string keys: `button.primary`, `button.ghost`, `tab.line.active`, `row.list`, `row.list.selected`, `section.header`, `nav.cluster`, `panel.footer`, `toast`, `card`, `kbd`, … Define the initial key registry in `docs/migration/recipe-keys.md`; keys are append-only.
3. **Resolution chain:** `RecipeSet.get(key)` → merge over the widget's built-in default Sx (which encodes today's look) → tokens. Missing key = today's appearance. `RecipeSpec → Sx` conversion happens once per frame per key (cache in the resolved theme-pack, not per-widget).
4. **First migrations (prove the model):** the `// RECIPE-CANDIDATE(S4)` fields from S1 — `wl_row_*` → `row.list`; `nav_cluster_*` → `nav.cluster`; `panel_footer_*` → `panel.footer`; `card_stripe_alpha` → `card`; `toast_bg_alpha` → `toast`; `tab_underline_thickness` → `tab.line.active`. For each: widget reads recipe; `Treatments`/`Chrome` field marked `#[deprecated]`; snapshot keeps carrying it until Wave 3 cleanup.
5. **Loading:** `ThemePack` gains an optional `recipes` section in its JSON/TOML; hot-reload path (`hot_reload.rs`) re-resolves the recipe cache.

### Acceptance
- A test theme-pack JSON that overrides `button.primary` (pill radius + accent fill) and `row.list` (flush, square, divider) visibly restyles the app with zero Rust changes.
- Absent recipes ⇒ pixel-identical to pre-S4.
- The six first-migration knobs render from recipes; deprecated fields have zero non-shim readers.

---

## Stream S5 — `StyleCtx`: threaded style context (kill ambient split-brain)

**Goal:** widgets resolve colors AND dimensions AND recipes from one passed-in context; two shells can render in a single frame (theme preview gallery becomes possible).
**Owns:** `src/ui_kit/widgets/**` (after S3 merges), `src/ui_kit/widgets/theme.rs` extension.
**Depends on:** S3 (and S4's types if available; design for them with a placeholder). **Blocks:** preview gallery.

### Tasks
1. Define `StyleCtx<'a> { theme: &'a dyn ComponentTheme, tokens: &'a TokenSnapshot, recipes: &'a RecipeSet }` (recipes behind `Option`/default-empty until S4 merges).
2. Add `show_ctx(self, ui, &StyleCtx)` entry points to widgets. Existing `show(ui, t)` becomes a shim: builds a `StyleCtx` from `t` + the thread-local snapshot + ambient recipes. **No call-site churn in this stream** — the shim keeps every existing caller compiling.
3. Fix the split-brain: the dimension default-methods on `ComponentTheme` (`font_md()`, `radius_sm()` etc. delegating to globals) get equivalents on `StyleCtx` that read `self.tokens`. Widgets migrate their internal reads from global helpers → ctx fields as they gain `show_ctx`. Order: Button → Tabs → PanelSection/PanelListRow → Input/Select → the rest (long tail can trail into Wave 3).
4. Proof artifact: a debug "theme gallery" screen rendering the same widget strip under two different `StyleCtx`s (different StyleSystem AND ColorScheme) side by side in one frame.

### Acceptance
- Gallery screen shows two visibly different shells in one frame with no cross-bleed.
- Zero behavior change for existing `show(ui, t)` callers.
- Migrated widgets contain no direct `frame_tokens()` reads in their paint paths (the ctx carries it).

---

## Stream S6 — `ShellProfile`: structural shell variants

**Goal:** shells differ structurally (navigation style, dock arrangement), not just in skin. This is the JetBrains-grade differentiator.
**Owns (Wave 1, design):** `docs/migration/shell-profile.md`. **Owns (Wave 3, impl):** new `src/design_system/shell_profile.rs` + the top-level layout composition code (identify exact files during design; they're in `chart/renderer` — the layout that places TopNav / right rail / bottom dock. NOT `core.rs`).
**Depends on:** design — nothing; implementation — S2 (so the profile loads with the theme pack).

### Tasks
1. **Wave 1 — design doc.** Inventory today's fixed structure (top pill-nav, main content, right rail stack, bottom dock). Propose `ShellProfile` as data:
   ```
   ShellProfile {
     nav: NavStyle,            // TopPills | TopTabs | SideRail | MenuBar
     dock: DockStyle,          // BottomPill | BottomBar | Hidden
     rail: RailSide,           // Right | Left | None
     panel_chrome: …,          // shell-level enclosure defaults
   }
   ```
   Follow the existing pattern: typed enum + the concrete look composed from recipes/Sx at the render site (the `GroupEnclosure` doc comment in `style_system.rs` states this philosophy — generalize it). Define which regions are variant-driven vs fixed; specify state interaction (open-panel flags live in app state, NOT the profile).
2. **Wave 3 — implement** `NavStyle` first (highest visible payoff): the top-nav render site branches on the profile enum; each variant is composed from existing ui_kit primitives + recipes. Then dock, then rail side.
3. Wire `ShellProfile` into the `ThemePack` loader (optional section, default = current structure).

### Acceptance
- Design doc reviewed by the user before Wave 3 implementation starts.
- Two shells differing in `NavStyle` + recipes + palette look like different products, selected purely from data.
- Profile-switch at runtime doesn't lose panel/app state.

---

## Wave-3 integration checklist (run as a single coordinating session)
1. Merge order: S1 → S3 → S2 → S4 → S5 → S6-impl (rebase each onto the previous).
2. Delete deprecated `Treatments`/`Chrome` recipe-candidate fields once recipe adoption (S4) covers them and snapshot readers are gone.
3. Ratchet all S0 baselines to zero; flip the lints from ratchet to hard-ban.
4. End-to-end demo: author a brand-new `ThemePack` JSON (new palette + new scales + recipe overrides + `SideRail` nav) → it loads, hot-reloads, and renders as a visually distinct shell with **zero Rust edits**.
5. Update `docs/DESIGN_SYSTEM.md` and `src-tauri/CLAUDE.md` (token sources table, "add a style" instructions now point at theme-pack data, not `style_defaults`).

## Per-agent prompt template (copy into each thread)
> You are executing **Stream S_N** of `docs/STYLE_MIGRATION_BRIEF.md` in apex-terminal. Read that brief section, `src-tauri/CLAUDE.md`, and `docs/migration/field-disposition.md` (if it exists) before any edit. Work ONLY on branch `style-mig/sN-<name>`. You may only write files listed under your stream's "Owns". Never touch `src/chart/renderer/render/pane/core.rs`; never add fields to `Watchlist`/`Chart`. Definition of done: your stream's Acceptance list, `cargo check` clean, tests green, Bauhaus light-theme walk-through for visual changes. If you need to edit a file owned by another stream, stop and report instead.

---
---

# PART 2 — Theme Platform Streams (S7–S11)

Part 1 (S0–S6) makes the app **fully themable from data**. Part 2 turns that capability into a **product**: themeable fonts/icons, a packaged installable theme format, runtime validation, a standalone authoring/storybook app, and the consumer-facing settings surface.

## Two surfaces — keep them separate (explicit user requirement)

The user wants theme **management** and theme **authoring** kept apart. They are two distinct products with two distinct audiences:

1. **Consumer surface — in-app `Settings → Themes` section (owned by S8).** A dedicated, self-contained settings section. Browse installed theme packs, install from file, enable/disable, select active, switch live. **Nothing about editing/creating themes lives here.** It must be its own settings section, not interleaved with other preferences.
2. **Creator surface — standalone "Theme Studio" application (owned by S10).** A separate desktop binary (eframe/egui, NOT Tauri, NOT the trading app) that links the extracted `apex-ui-kit` + `apex-design-system` crates. It is both the **component storybook** (every widget × variant × state) and the **live theme editor** (edit palette/scales/recipes/shell-profile, see it applied to the catalog in real time, export a `.apextheme` pack). It ships and runs independently of the terminal.

```
apex-design-system  (crate)  ─┐
apex-ui-kit         (crate)  ─┼─→ apex-terminal (Tauri app)  → Settings→Themes (S8, consume/install/switch)
                              └─→ theme-studio (eframe app)   → storybook + editor + export (S10, author)
                                       ▲
                              shared ThemePack format (S8) + validator (S9)
```

This is the VSCode split: the editor *uses* themes in its settings; theme *creation* is separate tooling. Both sides speak the same `ThemePack` format (S8) and run the same validator (S9), so a pack authored in Studio installs verbatim in the terminal.

## Palette depth — DECIDED ✅ (2026-06-13): widen, keep trading aliases
The semantic color layer is **widened** to independent `info / success / warning / danger` plus a richer neutral ramp. `bull` / `bear` are **retained as trading aliases** that *default* to `success` / `danger` respectively but may diverge per theme.

**Binding consequences for the streams that build on `ColorScheme`:**
- **S1** adds the new semantic fields to `ColorScheme` (`info`, `success`, `warning`, `danger`, neutral ramp). `bull`/`bear` stay as fields whose `Default`/loader behavior falls back to `success`/`danger` when unset. Migrate all 15 built-in schemes (set `success`/`danger` from the existing `bull`/`bear` so today's look is unchanged). Extend the equivalence/round-trip tests.
- **`ComponentTheme`** gains `info()/success()/warning()/danger()`; `success()`/`danger()` stop defaulting to `bull()`/`bear()` and read the real fields (keep `bull()`/`bear()` for trading widgets).
- **S4** recipes reference the widened semantics by name (`text@danger`, `fill@info`, …); `bull`/`bear` remain valid tone references for trading components.
- **S10/S11** authoring UI exposes the full semantic set; the authoring guide documents the alias relationship.
- Coordinate the `ColorScheme` field additions between S1 (lands the fields) and S11 (owns the spec/schema). Record the agreed owner in `docs/migration/field-disposition.md`.

### Platform dependency graph
```
S7  (themable assets)        — Wave 1, parallel; informs S8 bundling
S3-crate-extraction          — prerequisite for S10 (mandatory, see S3 step 5)
S8  (pack format+lifecycle)  — Wave 4; needs S1 schema, S4 recipes, S7 assets
S9  (validation + a11y)      — Wave 4; needs S8 format
S10 (Theme Studio app)       — Wave 4; needs S3-crate + S4 + S5
S11 (semantic palette+docs)  — Wave 1 decision, Wave 5 build; touches S1/S4/S10
```

---

## Stream S7 — Themable assets (fonts, icons, imagery)

**Goal:** a theme can ship and select its own **fonts** and **icon set**, not just colors and sizes. Without this every theme has identical lettering and iconography — the single biggest "looks the same" giveaway.
**Owns:** `src/ui_kit/fonts/` (new module wrapping the embedded `.ttf` loading), `src/ui_kit/icons.rs`, new `src/ui_kit/assets.rs`. Coordinate with S1 for the `Typography` family fields and S3 for crate boundaries.
**Depends on:** nothing for design; aligns with S1/S3. **Blocks:** S8 (asset bundling), S10 (authoring needs to preview fonts/icons).

### Tasks
1. **Font registry.** Replace the hardcoded Inter/JetBrains-Mono embedding with a registry: a set of built-in families plus a runtime path to register theme-supplied font bytes. Add `Typography.family_ui` / `family_mono` / `family_display` (family *names* resolved against the registry) to `StyleSystem` (coordinate the field add with S1).
2. **Font fallback chain.** Define behavior when a theme requests a family that isn't installed (fall back to built-in, surface a validation warning via S9). Never render tofu/blank.
3. **Pluggable icon sets.** Make `icons.rs` resolve glyphs through an `IconSet` indirection (built-in set + optional theme-supplied set keyed by the same icon names). A theme that omits icons uses the built-in set. Keep the `Icon::` name enum stable as the lookup key.
4. **Imagery/decoration (optional tier).** Allow a theme to supply background textures / a logo slot, behind a capability flag so it's opt-in and sandboxable (S9).
5. **Asset-handle contract** so S8 can bundle and S10 can preview the same bytes.

### Acceptance
- A theme pack referencing a bundled font family renders the whole UI in that font; removing it falls back cleanly with a warning, never tofu.
- Swapping icon sets visibly changes iconography app-wide with no code change.
- Built-in theme with no assets = pixel-identical to today.

---

## Stream S8 — `ThemePack` format, packaging & lifecycle (+ in-app Themes settings)

**Goal:** a single installable artifact (`.apextheme`) carrying palette + scales + recipes + shell-profile + assets + manifest; plus the install/enable/select lifecycle and the **dedicated in-app `Settings → Themes` section**.
**Owns:** new `src/design_system/theme_pack/` (manifest, (de)serialize, bundle reader/writer), new `src/.../settings/themes_section.rs` (the consumer UI), theme storage/registry on disk. Coordinate with `loader.rs` (S1) and `recipes.rs` (S4).
**Depends on:** S1 (StyleSystem schema), S4 (RecipeSet), S7 (assets), S6 (ShellProfile). **Blocks:** S9, S10 export target, Wave-5 demo.

### Tasks
1. **Manifest schema.** `id`, `name`, `author`, `version` (semver), `app_schema_version` (min/target), `is_dark`, capability flags (uses-fonts/icons/imagery/shell-profile), asset inventory, checksum. Versioned and forward-compatible.
2. **Bundle format.** Define `.apextheme` (a zip: `manifest.json` + `colorscheme.json` + `stylesystem.json` + `recipes.json` + optional `shellprofile.json` + `assets/`). Reader + writer. Streaming-safe, size-bounded.
3. **Schema versioning & migration.** A pack authored for an older app version still installs — define a migration path (defaulted new fields, deprecation handling). This is what keeps installed themes alive across app updates.
4. **On-disk registry & lifecycle.** Install (validate via S9 → copy to themes dir → register), uninstall, enable/disable, set-active. Active selection persists across sessions and survives app updates. Built-in themes are read-only entries in the same registry.
5. **In-app `Settings → Themes` section (the consumer surface).** A standalone settings section: list installed packs (built-in + user) with live preview swatches, Install-from-file button, enable/disable, click-to-activate with instant live switch (uses S5 `StyleCtx` hot-swap + S7 asset reload + `hot_reload.rs`). **Authoring/editing is explicitly out of scope here** — link out to Theme Studio (S10) instead. Keep this section isolated; do not interleave with other preferences.

### Acceptance
- Round-trip: export a `.apextheme` (from S10 or a fixture) → install via the Themes settings section → it appears, activates, and live-switches with no restart.
- An older-schema pack installs and renders via migration defaults.
- Uninstall removes it cleanly; active theme persists across an app restart.
- The Themes settings section contains zero authoring controls (separation requirement met).

---

## Stream S9 — Validation, accessibility & sandboxing

**Goal:** no installed pack can break or render the UI illegible; untrusted packs are safe to install.
**Owns:** new `src/design_system/theme_pack/validate.rs`, contrast/a11y utilities, the validation report type shared by S8 (install gate) and S10 (author-time linting).
**Depends on:** S8 (format). **Blocks:** trustworthy install; S10 author-time feedback.

### Tasks
1. **Structural validation.** Required keys present, types/ranges sane, enum values known, asset references resolve, manifest version compatible. Produce a structured report (errors vs. warnings), never panic.
2. **Accessibility gate.** WCAG contrast checks on the key foreground/background and semantic pairings; flag (warn) or block (error, configurable) illegible combinations. Catch "text == bg" disasters before they ship.
3. **Missing-token fallback policy.** Define and enforce the resolution fallback (recipe → semantic → base → built-in default) so a partial pack is always renderable; report what fell back.
4. **Sandboxing untrusted packs.** Bound asset sizes/counts, restrict asset types (no executables), strip/ignore unknown capability requests, namespace-isolate font/icon registration so one pack can't clobber another or a built-in. Treat every user-installed pack as untrusted by default.
5. Shared report surface so S8 shows it at install and S10 shows it live while authoring.

### Acceptance
- A deliberately broken pack (bad contrast, missing keys, oversized assets, wrong types) is rejected or downgraded-with-warnings — never crashes, never renders illegible.
- A valid pack passes clean.
- Same validator binary-identical results in both the terminal (S8) and Studio (S10).

---

## Stream S10 — "Theme Studio" standalone app (storybook + live editor + export)

**Goal:** a separate desktop application that is both the **component catalog/storybook** and the **theme authoring tool**, running on the extracted crates with no dependency on the trading app.
**Owns:** new workspace member `tools/theme-studio/` (eframe binary), its UI, the catalog registry, the editor panels, export-to-`.apextheme`.
**Depends on:** S3-crate-extraction (hard), S4 (recipes to edit), S5 (`StyleCtx` to apply a pack to the catalog in-frame), S7 (font/icon preview), S8 (export format), S9 (live validation). **Blocks:** Wave-5 demo, ongoing theme QA.

### Tasks
1. **Workspace member.** `tools/theme-studio` as an eframe app depending only on `apex-ui-kit` + `apex-design-system`. Confirms the extraction is real (if Studio can't build, the crates aren't clean — report back to S3).
2. **Storybook/catalog.** A scrollable gallery rendering every ui_kit component across every variant × size × state (Button, Tabs, Inputs, Panel rows/sections, Tags/Badges, Tooltips, Modals, Tables, …). This is also the canonical visual-QA surface for the whole program — wire it so adding a widget adds a catalog entry.
3. **Live editor.** Panels to edit `ColorScheme`, `StyleSystem` scales/treatments, `RecipeSet` keys, and `ShellProfile`. Edits apply to the catalog in real time via S5 `StyleCtx` (this is the payoff of fixing the ambient split-brain — two+ contexts in one frame).
4. **Author-time validation.** Run S9 continuously; show errors/warnings/contrast inline as the user edits.
5. **Import/export.** Open an existing `.apextheme` to edit; export a new one. Round-trips with S8.
6. **Side-by-side compare** (leverages S5): show two packs, or before/after, simultaneously.

### Acceptance
- `cargo run -p theme-studio` launches with no chart-app/Tauri in the dependency graph.
- The catalog shows every component; switching the edited pack restyles the whole catalog live.
- Author a pack end-to-end in Studio, export `.apextheme`, install it in the terminal (S8) — visually identical in both.

---

## Stream S11 — Semantic palette widening, public schema & authoring docs

**Goal:** make the system genuinely third-party-authorable: a richer/decided semantic layer, a published schema, and the docs/tooling an external author needs.
**Owns:** `docs/theme-authoring/` guide, generated token reference, published JSON-schema files for each pack component, the semantic-palette changes (coordinate the `ColorScheme` edits with S1 — agree ownership; likely S1 lands the field changes, S11 drives the spec/decision).
**Depends on:** the palette-depth decision (Wave 1), S1/S4/S8/S10. **Blocks:** external authoring.

### Tasks
1. **Resolve & implement palette depth** per the open decision above (recommended: independent `info/success/warning/danger` + neutral ramp, with `bull`/`bear` as defaulting aliases). Migrate built-ins; keep trading semantics intact.
2. **Publish JSON Schemas** for manifest / colorscheme / stylesystem / recipes / shellprofile so external tools (and Studio) validate against a single source of truth.
3. **Auto-generated token reference** from the schema (every token, type, default, what it affects) — no hand-maintained drift.
4. **Theme-authoring guide** in `docs/theme-authoring/`: concepts, the resolution chain, recipe keys (link S4's registry), the asset workflow (S7), the validation rules (S9), and an annotated example pack.
5. Update `docs/DESIGN_SYSTEM.md` + `src-tauri/CLAUDE.md` to point "add a theme" at the pack/Studio workflow.

### Acceptance
- A person who has never seen the code can author a valid, installable, accessible `.apextheme` from the docs + Studio alone.
- Published schemas validate every built-in pack.
- Token reference is generated, not hand-written.

---

## Wave-4/5 platform integration checklist (coordinating session)
1. Confirm S3 crate extraction is fully clean (`cargo build -p apex-ui-kit` / `-p apex-design-system` with no chart-app in graph) before starting S10.
2. Resolve the palette-depth decision (S11) before S11 build and before S10 finalizes its editor fields.
3. Lock the `ThemePack` format (S8) + validator report (S9) first; S10 export and the Themes settings section both consume them — freeze the contract before parallelizing.
4. End-to-end product demo: in **Theme Studio**, author a pack with a new palette, custom font + icon set, recipe overrides, and a `SideRail` shell profile → export `.apextheme` → in the **terminal**, install it via `Settings → Themes` → it validates, activates, live-switches, and persists across restart — **all with zero Rust edits and zero terminal rebuild.**
5. Negative demo: install a deliberately-broken pack → rejected with a clear report, UI unharmed.
