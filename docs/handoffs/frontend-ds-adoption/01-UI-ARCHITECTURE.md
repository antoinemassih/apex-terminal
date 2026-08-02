# 01 — The UI Layer: How It Actually Works

Orientation for engineers who will change how apex-terminal looks. Written to be read
once, end to end, before touching code.

Claims are marked **[verified]** (read directly out of the source during authoring) or
**[per doc]** (taken from an existing design document — re-grep before relying on exact
line numbers, which drift).

---

## 1. Shape of the codebase

197,621 lines of Rust across 443 files, in `src-tauri/src/`.

```
chart/          the terminal itself — renderer, panes, trading, io, state
  renderer/
    gpu.rs                    10,874  Theme struct, Watchlist, Chart, THEMES, setup_theme
    commands.rs                1,917  AppCommand bus
    render/pane/core.rs       13,672  🔒 SACRED — the paint pipeline + shell assembly
    ui/                              panels, components, toolbar, inputs, overlays
      style.rs                 3,807  ⚠️ LEGACY StyleSettings + free-fn helpers
      theme_studio.rs                 in-app .apextheme authoring
    trading/                          order manager, broker
    io/                               fetch
design_system/               11,094  ✅ CANONICAL two-axis token system
ui_kit/                              ~95 widgets + sx/ + layout/ + tokens
dev_inspector/                6,120  headless HTTP harness
foundation/
  design_inspector.rs         3,303  F12 live token editor
  design_tokens.rs                    dt_f32!/dt_u8!/dt_i8! runtime overrides
state/aggregates.rs           2,337  where NEW state goes
playground/                          designer + standalone playground binary
persistence/  data/  watchlist/
```

Binaries: `apex-native` (the app), plus a playground binary (`playground_main.rs`).

---

## 2. The frame

### 2.1 Shell assembly order **[per doc — `docs/migration/shell-profile.md`]**

`draw_chart()` in `chart/renderer/render/pane/core.rs` **[verified: `core.rs:10026`]** is
the single function that orchestrates the per-frame shell. egui resolves the remaining
`CentralPanel` rect after every `TopBottomPanel` / `SidePanel` has claimed its space, so
**registration order is layout order**:

| # | Registers | egui primitive |
|---|---|---|
| 1 | `render_toolbar` → `top_nav::render` | `TopBottomPanel::top("tb")` — main pill bar |
| 2 | `toolnav::render_toolnav` | `TopBottomPanel::top("toolnav")` — second chrome row |
| 3 | inline `account_strip` block | `TopBottomPanel::top("account_strip")` — conditional |
| 4 | `workspace_rail::render_workspace_rail` | `SidePanel::left("workspace_nav_rail")` |
| 5 | `bottom_dock::draw` | `TopBottomPanel::bottom("apex_bottom_dock")` |
| 6 | `right_rail::render` | `SidePanel::right(...)` |
| 7 | `CentralPanel::default()` | the workspace / chart pane grid |

Object Tree and floating modals render as `egui::Window` / `Modal` overlays inside
`top_nav::render` — they do **not** claim a layout strip.

**Consequence for you:** any change to which chrome regions exist is a change *inside the
sacred file*. It is a small, surgical branch in a thin wrapper — but it is in `core.rs`,
so it needs a single owner and cannot ride along with a sweep. Plan for that.

### 2.2 Region drivers **[per doc]**

| Region | Height/width driver | Visibility |
|---|---|---|
| Top-nav | `Chrome.toolbar_height_scale` × 30–38 px + `region_gap` | always (auto-hide → 2px accent hint) |
| Toolnav | `Chrome.toolnav_height > 0` | `style::toolnav_visible()` = style default + user override |
| Account strip | `Chrome.account_strip_height` (26 px) | `watchlist.account_strip_open` |
| Workspace rail | 52 px collapsed / 216 px expanded | always registered |
| Right rail | `watchlist.rail_col_width` (240–820 px, persisted) | any panel `is_open` |
| Bottom dock | `watchlist.bottom_dock_height` (persisted) | `style::footer_visible()` |
| Central | remainder | always |

Note the pattern: **`Chrome` supplies the dimension; `Watchlist` supplies the user's
session state.** Keep that boundary. Layout *structure* is config; open/closed and
dragged widths are app state.

---

## 3. The theme pipeline

This is the part you must understand completely. There are **five coexisting token
layers** because a migration is in flight. Knowing which is canonical is the difference
between a change that lands and a change that silently does nothing.

### 3.1 The canonical path (`design_system/`) — use this

```
builtin_color_schemes()  ──┐
                           ├──> ThemeRegistry ──> snapshot(style, colors) ──> DesignSnapshot
builtin_style_systems()  ──┘         │                [snapshot.rs:373]
                                     │
              .apextheme packs ──────┘
              (theme_pack/, loader.rs, hot_reload.rs)
```

- **`ColorScheme`** **[verified: `color_scheme.rs:119`]** — palette axis. `bg`, `surface`,
  `text`, `dim`, `text_muted`, `border`, `accent`, `bull`, `bear`, `warn`, optional
  `success`/`danger`/`warning`/`info`, `shadow`, `gold`, `hud_*`, `rrg_*`, `cmd_palette[11]`.
- **`StyleSystem`** **[verified: `style_system.rs:881`]** — dimension axis. Ten sub-structs:
  `Typography` `Spacing` `Radii` `Strokes` `Alphas` `Elevation` `Density` `Shadows`
  `Treatments` `Chrome`.
- **`RecipeSet`** (`recipes.rs`) — per-component overrides on top of both axes.

Resolution chains **[per `docs/theme-authoring/README.md`]**:

```
field:  recipe override → widget built-in Sx default → no paint / transparent
colour: success → bull  |  danger → bear  |  warning → warn  |  info → muted blue
```

The two axes **never mix until the Resolver joins them at render time**. That is why a
dark palette can wear a rounded Aperture geometry, and why contrast validation runs on
the `ColorScheme` alone.

### 3.2 Getting a theme at a call site

The full chart `Theme` is **ambient-stashed every frame**. Any function with a `ui` or
`ctx` can retrieve it — no threading required **[verified: `theme_impl.rs:84`]**:

```rust
let t = crate::chart_renderer::theme_impl::active_theme(ui.ctx());
```

For `ui_kit` widgets, take the trait instead **[verified: `ui_kit/widgets/theme.rs:17`]**:

```rust
pub fn my_widget(ui: &mut Ui, theme: &dyn ComponentTheme) { … }
```

`ComponentTheme` surface: `accent` `bull` `bear` `text` `dim` `border` `border_variant`
`warn` `bg` `surface` `surface_raised` `element_{hover,active,selected,disabled}`
`ghost_{hover,active}` `icon{,_muted,_disabled,_accent}` `shadow_color` `success` `danger` …

**Why this matters:** the ambient accessor exists precisely because threading `&Theme`
through three layers is real work while typing `Color32::from_rgb(56, 200, 120)` is not —
which is how the RRG panels ended up theme-blind despite the palette carrying tuned
`rrg_*` colours for every theme. The shortcut must never be cheaper than the correct path.

### 3.3 ⚠️ Derivation heuristics — the thing this programme is about

Several accessors **synthesise** values the design systems **author**:

| Derivation | Where | Problem |
|---|---|---|
| `elevate(bg, amount)` | `ui_kit/style.rs:462` **[verified]** | Additive luminance shift, dark→lighter / light→darker at 3/5 strength. **Achromatic** — cannot produce Aperture's *warm* `#141311` from `#000000`. |
| `ELEVATE_*` constants | `ui_kit/style.rs:478-483` **[verified]** | `PANEL_HEADER 30`, `PANEL_SECTION 26`, `PANEL_BODY 20`, `CARD 22`, `RAISED 30`, `MODAL 38`. Global constants tuned against one theme's ramp. |
| `ComponentTheme::surface_raised()` | `ui_kit/widgets/theme.rs` **[verified]** | Default impl is a `color_layer_up(t, 1)` 7 %-step heuristic. |
| `ELEVATION_{1,2,3}_FACTOR` | `ui_kit/style.rs:445-447` **[verified]** | Legacy gamma multipliers `0.95/0.88/0.85`. **Only ever darken** — collapse to black on a `#000` canvas. Superseded by `elevate()` on 2026-07-30; verify nothing still calls them on a near-black theme. |

The design systems hand-author a **four-step** background ramp and a **four-step** ink
ramp per theme. We store two-to-three points and synthesise the rest. Lucid's ramp is not
even monotonic (panel *lighter* than canvas, then surface *darker*) and no single-direction
function can produce it. Full analysis in `docs/DESIGN_BRIEF_DS_ADOPTION.md` §2 Law 3;
the fix is in [`02-TOKEN-CONTRACT.md`](02-TOKEN-CONTRACT.md).

### 3.4 The other four token layers — know what they are

| Layer | Location | Status |
|---|---|---|
| **Canonical** | `design_system/` | ✅ Target. New work goes here. |
| **Legacy `StyleSettings`** | `chart/renderer/ui/style.rs` (3,807 LOC) | ⚠️ Being pruned. `style-mig-lint.sh` check #1 ratchets its `pub` field count **DOWN — no new fields**. |
| **ui_kit locals** | `ui_kit/style.rs`, `ui_kit/tokens.rs` | Helpers + elevation math. `elevate()` lives here. |
| **Widget enums** | `ui_kit/widgets/tokens.rs` | `Size` / `Variant` / `Density` — use these in new components. |
| **Design-mode overrides** | `foundation/design_tokens.rs` | `dt_f32!` / `dt_u8!` / `dt_i8!` — under the `design-mode` feature every token reads from a runtime struct, which is what makes F12 live editing work. **Any new token must be wired through these macros or it will not appear in the editor.** |

### 3.5 Text: let the cascade set size

`egui::Style` is inherited by child `Ui`s and its `text_styles` table is a
semantic-name → `FontId` map. All 14 `TextStyle` tiers are registered into it every
frame (`TextStyle::install`, called from `setup_theme`):

```rust
// preferred — size comes from the inherited table
ui.label(TextStyle::Body.as_rich_cascading("Hello", t.text));

// a subtree can override ONE tier for all its children
ui.style_mut().text_styles.insert(TextStyle::Body.egui(), smaller_font);
```

Hand-passed `FontId`s can never do the second thing, which is why ~70 % of the app had
drifted onto 9–11 px: each of ~626 text sites independently chose a size. `as_rich`
(explicit) still works; migrate to `as_rich_cascading` opportunistically.

**Relevance to this programme:** per-theme type scales (Meridien's is a full step larger
than Lucid's) are only expressible through the cascade. Hand-passed sizes will not move.

### 3.6 Sx recipes

`ui_kit/sx/` composes styles at the render site rather than threading fields through the
pipeline. `GroupEnclosure` in `style_system.rs` is the canonical model **[per doc]**:

> "The concrete look (radius / fill / border) lives as composed `Sx` at the render site,
> not as data threaded through the style pipeline — so a new treatment is one new variant
> here plus its `Sx` recipe, with no schema change."

**Follow this pattern.** When you need a new visual treatment, prefer *enum variant +
render-site recipe* over *new threaded field*. Ratcheted by `scripts/sx_ratchet.sh`.

---

## 4. Ownership boundaries

### 4.1 🔒 Sacred — `chart/renderer/render/pane/core.rs`

13,672 lines. The GPU-optimised chart paint pipeline; the hottest path in the app.

- **No mechanical sweeps.** No token migration, no button consolidation, no cleanup.
- **No helper extraction "for organisation."** Function-call overhead, lost inlining and
  parameter passing can show up as measurable frame drops. If you think you need one,
  write a doc and benchmark it first.
- **One owner at a time.** Multi-agent fanout does not cover this file.
- Visual tweaks the user explicitly asks for are fine — single owner, verified in the
  running app.
- Ratchet scripts deliberately do not grep inside it.

See `docs/PANE_RS_SPLIT_PLAN.md` — the split is deferred indefinitely; perf risk outweighs
the organisational win.

### 4.2 🧊 Frozen — `Watchlist` and `Chart` (`chart/renderer/gpu.rs`)

Per ADR-0001, a state migration is in progress.

- **No new fields on either struct.**
- New per-chart state → `chart/state/ChartState`. New app/UI state → `state/aggregates.rs`.
- Anything new **must** be mirrored in `push_to_*` / `sync_from_*` or it will not persist.
- New mutation goes through `AppCommand` (`chart/renderer/commands.rs`), not direct `&mut`.
- Keyboard/global input must not be read inside `render_chart_pane` — it fans out to every
  pane. Active-pane-gate it.

**This constrains the layout work directly.** A new dashboard archetype needs layout
state; it goes on a `state/` aggregate, never on `Watchlist`.

### 4.3 Hard rules (from `src-tauri/CLAUDE.md` — binding)

| Rule | Enforcement |
|---|---|
| Never `&THEMES[0]` — accept `&Theme` / `&dyn ComponentTheme` | `style-mig-lint.sh` check 3, **hard ban at 0** |
| Never `Color32::from_rgba_unmultiplied(0,0,0,…)` for shadows — use `t.shadow_color` | `style-mig-lint.sh` check 4 |
| No new `pub` fields on `StyleSettings` | `style-mig-lint.sh` check 1 |
| `ui_kit/` must not import `chart_renderer` | `style-mig-lint.sh` check 2 |
| Tokens not literals (`mono_sm()` not `FontId::monospace(11.0)`) | `check-design-system.sh` |
| Prefer `ui_kit::Button` over `egui::Button` | review |
| Prefer `ui_kit::Tag` / `Badge` over legacy chips/pills modules | review (legacy are `#[deprecated]`) |
| Prefer `ui_kit::Header` over `panel_header()` / `dialog_header()` free fns | review |
| Use `shadow_*_themed(t)` variants, not the bare ones | review |
| Walk a light theme (Bauhaus) before claiming done | review |

`check-design-system.sh` baseline at introduction: **903 violations across 127 files**.
It only tightens — a file may never exceed its recorded count; improvements lock in with
`--update`. Token-definition files (`style.rs`, `theme*.rs`, `builtin.rs`,
`design_inspector.rs`) are exempt because they must use raw primitives to *build* the
tokens everything else consumes.

---

## 5. Where to add things

| You want to… | Go to |
|---|---|
| Add/edit a palette | `design_system/builtin.rs` → `builtin_color_schemes()` (`:110`) |
| Add/edit a dimension style | `design_system/builtin.rs` → `builtin_style_systems()` (`:842`) |
| Add a palette **field** | `design_system/color_scheme.rs` + snapshot + import/export + validate |
| Add a dimension **field** | `design_system/style_system.rs` + the same four |
| Add a per-component override | `design_system/recipes.rs` |
| Add a widget | `ui_kit/widgets/<name>.rs`, re-export from `widgets/mod.rs` |
| Add an icon | `ui_kit/icons.rs` |
| Add a chrome geometry knob | `StyleSystem.chrome` (`style_system.rs:683`) |
| Add a visual *treatment* | New enum variant in `Treatments` + its render-site `Sx` recipe |
| Add UI state | `state/aggregates.rs` — **never** `Watchlist`/`Chart` |
| Add a token to the F12 editor | wire through `foundation/design_tokens.rs` `dt_*!` macros |

### New-widget checklist (from `CLAUDE.md`)

1. Builder in `ui_kit/widgets/<name>.rs`
2. `pub struct <Name><'a>` with chaining builder methods returning `Self`
3. `pub fn show(self, ui: &mut Ui, theme: &dyn ComponentTheme) -> Response`
4. Sizing/state from `ui_kit::widgets::tokens::{Size, Variant, Density}`
5. Colours from the `theme` argument — never `&THEMES[0]`
6. 5–10 line module doc: what it does, variants, when to use it, when to use something else
7. Re-export from `widgets/mod.rs`

---

## 6. Panel primitives you will reuse

Do not hand-roll panel chrome. The library is deep:

- **`SidePanelShell::new(id, title)`** — outer shell (replaces hand-rolled
  SidePanel + header + frame). `::tabs(id, &mut state, tabs)` for tab-driven panels.
  Widths: `Width::Narrow` 240 / `Medium` 300 / `Wide` 400, all resizable.
- **`SplitSectionPanel::new(id, &mut splits)`** — feed/signals/analysis multi-pane.
- **`PanelSectionGroup::new(&mut fracs)`** — N stacked sections with user-draggable
  dividers; caller owns `&mut [f32; N]`.
- **`PanelSubSection::new(id, title)`** — collapsible category with caret + count chip,
  optional `.header_trailing(…)` slot.
- **`PanelListRow`** — `.columns(&[Column…])` for streaming free-form rows (slice-based,
  no per-cell allocation — tape emits ~200 rows/frame); `.trailing_buttons(&[TrailingBtn…])`
  + `.show_full()` for typed icon strips.
- **`PanelSection`** — `.collapsible(&mut expanded)`, `.delete_when_empty()`,
  `.action(label, tone)` (click surfaces via `SectionResponse.action_clicked`).
- **`PanelFooter`**, `PanelEmpty`, `PanelLoading`, `PanelCard`, `PanelKeyValueRow`.

**Shell API contract:** `show` does **not** take `&mut open`. The caller early-returns and
writes its own flag from `SidePanelShellResponse.close_clicked` — this removes the borrow
conflict when the body closure also needs `&mut watchlist`.

Floating panels (settings, news, connection) use `Header::dialog` + `Modal`, not
`SidePanelShell`.

---

## 7. Mental model to carry away

1. **Two axes plus recipes, joined by a resolver at render time.** Palette and dimensions
   are independent by design.
2. **Five token layers coexist.** `design_system/` is canonical; `StyleSettings` is being
   pruned; know which one you are editing.
3. **The shell is a fixed ordered stack of egui panels assembled in a sacred file.**
   Structural changes there are surgical and single-owner.
4. **Derivation is the default and authoring is the exception** — and that is precisely
   the mismatch this programme fixes.
5. **Two files are off-limits and one file is the hottest path in the app.** Everything
   in this handoff routes around them.
6. **The ratchets are the memory of past cleanups.** They only tighten. Do not raise a
   baseline.
