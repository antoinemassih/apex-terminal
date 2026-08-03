# ShellProfile — Wave-1 Design Document (Stream S6)

**Status:** SIGNED OFF 2026-08-03, **with one structural amendment** — see
`docs/handoffs/frontend-ds-adoption/13-DS-6.0-DECISION.md` (decision D1).

**Amendment:** `ShellProfile` is **absorbed into `StyleSystem` as a `ShellSpec`
block**. It is not a separate store, axis, or selection mechanism — that would
have produced two competing layout-selection paths (risk R5 in the DS-adoption
package). Concretely:

- This document remains the design reference for what `NavStyle`, `DockStyle`
  and `RailSide` **mean**. That vocabulary is adopted as-is.
- The types live on `StyleSystem.shell`, resolved by the same one-line
  precedence rule as the colour and dimension axes.
- `Archetype` joins them there, defaulted by the theme and overridable
  per-**workspace** (not per-theme — themes are exportable, so a personal layout
  preference must not travel inside a shared design artifact).
- The `shell_profile` blob already reserved in `theme-authoring/README.md`
  becomes the serialized `ShellSpec`, so that forward-compat placeholder is
  honoured rather than orphaned.

Ownership of the overlap sits with the DS-adoption package: S6 contributes the
vocabulary, that package owns the resolver, storage and precedence.

**Scope:** Design and doc only for this file. Implementation proceeds under
DS-6.x against the amended shape above.

---

## 1. Current Shell Structure — Inventory

The app shell is assembled in a fixed top-to-bottom/left-to-right egui panel stack.
Every frame the sequence is unconditional: egui resolves the remaining `CentralPanel`
rect after all `TopBottomPanel` and `SidePanel` claims are registered.

### 1.1 Assembly site

`src-tauri/src/chart/renderer/render/pane/core.rs` — `draw_chart()` — is the
single function that orchestrates the per-frame shell. It calls (in order):

| Order | Call site (file:line) | What it registers |
|---|---|---|
| 1 | `core.rs:11602` — `render_toolbar(...)` delegates to `top_nav::render(...)` | `egui::TopBottomPanel::top("tb")` — the main top-nav pill bar |
| 2 | `top_nav.rs:1184` — `toolnav::render_toolnav(...)` | `egui::TopBottomPanel::top("toolnav")` — second chrome row (tools + alert feed) |
| 3 | `top_nav.rs:1187-1222` — inline `account_strip` block | `egui::TopBottomPanel::top("account_strip")` — conditional broker summary strip |
| 4 | `core.rs:11606` — `workspace_rail::render_workspace_rail(...)` | `egui::SidePanel::left("workspace_nav_rail")` — collapsible workspace switcher |
| 5 | `top_nav.rs:1649` — `bottom_dock::draw(...)` | `egui::TopBottomPanel::bottom("apex_bottom_dock")` — Orders/Positions/Account/Alerts dock |
| 6 | `top_nav.rs:1657` — `right_rail::render(...)` | `egui::SidePanel::right(...)` — the unified right-side panel stack |
| 7 | `core.rs:11621` — `egui::CentralPanel::default()...show(...)` | The workspace / chart pane grid |

Object Tree (`top_nav.rs:1671`) and floating modals (Order Health, Spread, settings) are
rendered inside `top_nav::render` as `egui::Window` / `Modal` overlays over the above —
they do not claim a layout strip.

### 1.2 Region descriptions

| Region | egui primitive | Width/Height driver | Visibility |
|---|---|---|---|
| **Top-nav (tb)** | `TopBottomPanel::top("tb")` | `Chrome.toolbar_height_scale` × 30–38 px + `region_gap` | Always (unless `toolbar_auto_hide` collapses to 2px accent hint) |
| **Toolnav** | `TopBottomPanel::top("toolnav")` | `Chrome.toolnav_height` > 0 (Aperture/Glass) | `style::toolnav_visible()` — style default + user override |
| **Account strip** | `TopBottomPanel::top("account_strip")` | `Chrome.account_strip_height` (26 px) | `watchlist.account_strip_open` (bool, user-toggled) |
| **Workspace rail** | `SidePanel::left("workspace_nav_rail")` | 52 px collapsed / 216 px expanded | Always registered; zero-width is not used — it is always shown at collapsed width |
| **Right rail** | `SidePanel::right(...)` | `watchlist.rail_col_width` (persisted, 240–820 px) | Rendered when any panel in `right_rail::PANELS` has `is_open = true` |
| **Bottom dock** | `TopBottomPanel::bottom("apex_bottom_dock")` | `watchlist.bottom_dock_height` (persisted) | `style::footer_visible()` — style default `Chrome.footer_default_open` + user override |
| **Central / workspace** | `CentralPanel::default()` | Remainder after all panels claim space | Always |

### 1.3 Sub-structure inside the top-nav

The `render` function in `top_nav.rs` is also the host for all floating/overlay panels
(settings, command palette, connection, spread, order health, etc.). Those panels do not
affect the shell geometry — they are `egui::Window` / `Modal` overlays.

Within the top-nav row itself, the layout is (left to right):
- Logo glyph
- Account button (IBKR / connection state)
- Workspace rail expand/collapse toggle
- Paper/Live $ badge
- Separator
- TPS boss-key button
- Separator
- Scrollable middle section: workspace picker, layout segmented control
- Fixed right section (right-to-left): Window controls → Settings/Search/ORDER/Toolbar → Separator → Right-nav panel toggles (Watchlist, Orders, Indicators, …)

### 1.4 State locations

Open/closed flags for every panel live in `Watchlist` (the frozen god-object, via
`watchlist.sidebar_state` / `update_sidebar_state`). Width and height of resizable
regions are also in `Watchlist` (`rail_col_width`, `bottom_dock_height`). These are
**app state** — they are not structural layout choices, and they will not move into
`ShellProfile`.

---

## 2. Proposed `ShellProfile` Struct

### 2.1 Precedent: `GroupEnclosure`

`GroupEnclosure` in `design_system/style_system.rs` is the model to follow:

```rust
/// How a toolbar button-group is enclosed. The concrete look (radius / fill /
/// border) lives as composed `Sx` at the render site, not as data threaded
/// through the style pipeline — so a new treatment is one new variant here plus
/// its `Sx` recipe, with no schema change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GroupEnclosure {
    #[default]
    None,
    Bordered,
    Frosted,
    Sharp,
}
```

`ShellProfile` follows the same philosophy: each enum variant is a named structural
choice. The pixel geometry and exact visual recipe (fill, border, radius, height) are
composed from existing `Sx` / `region_frame` / `region_gap` primitives at the render
site — not threaded through the struct. Adding a new variant = one new enum arm + its
render-site recipe; no schema migration.

### 2.2 Type definitions

```rust
//! `ShellProfile` — the structural shell variant axis (Stream S6 Wave 1).
//!
//! Carries only structural layout choices — which chrome regions exist and
//! how they are shaped. Open/closed panel flags (app state) and per-pane
//! display state (frozen Watchlist/Chart) live elsewhere.
//!
//! Loaded as an optional section of the ThemePack. When absent, the default
//! `ShellProfile::current_structure()` reproduces today's fixed shell exactly.

use serde::{Deserialize, Serialize};

/// How the primary navigation is presented.
///
/// This controls the egui panel type and layout of the topmost interactive row.
///
/// The concrete look (height, padding, button treatment, group enclosure) is
/// composed from existing `region_frame` / `region_gap` / `Chrome` tokens at
/// the render site — not duplicated here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum NavStyle {
    /// Single horizontal pill bar across the top.
    /// Today's `egui::TopBottomPanel::top("tb")` — the only variant that
    /// exists today.
    #[default]
    TopPills,

    /// Horizontal tab strip: section names appear as tabs (Files/View/…
    /// Bloomberg-style). Taller than TopPills; tab underline treatment driven
    /// by `Chrome.tab_underline_thickness`. Uses the same `TopBottomPanel`
    /// host but with a different internal layout recipe.
    TopTabs,

    /// Vertical icon strip on the left side, replacing the top bar.
    /// Would be registered as a `SidePanel::left` instead of `TopBottomPanel`.
    /// Logo + nav icons stacked vertically; the workspace rail moves into this
    /// strip or is suppressed.
    SideRail,

    /// Native OS menu-bar equivalent rendered inside the window frame.
    /// Uses `TopBottomPanel::top` at minimal height; horizontal menu items
    /// open into dropdown menus, no icon-pill treatment.
    MenuBar,
}

/// Where the bottom trading dock is placed and what shape it takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DockStyle {
    /// Full-width bottom panel: today's `TopBottomPanel::bottom("apex_bottom_dock")`.
    /// Floats as a rounded card under tiled styles (Aperture/Glass) via region_gap.
    #[default]
    BottomBar,

    /// Collapsed by default to a 30 px tab strip; click any tab to expand.
    /// Mechanically identical to `BottomBar` but `Chrome.footer_default_open = false`.
    /// Provided as an explicit variant so ThemePack authors can declare intent.
    BottomPill,

    /// No bottom dock. The four tabs (Orders/Positions/Account/Alerts) are
    /// surfaced exclusively in right-rail panels. The `TopBottomPanel::bottom`
    /// registration is skipped entirely — workspace gets the full height.
    Hidden,
}

/// Which side the panel stack occupies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RailSide {
    /// Right side — today's only variant.
    /// `egui::SidePanel::right(...)` registered after bottom dock.
    #[default]
    Right,

    /// Left side — `SidePanel::left(...)`.
    /// Workspace rail (workspaces) would need to move or merge.
    Left,

    /// No persistent rail. Panels open as floating `egui::Window`s instead.
    /// Implies the right-nav toggle buttons in the top-nav open floats.
    /// Existing `SidePanelShell` / `SplitSectionPanel` bodies remain valid;
    /// only the outer egui host changes.
    None,
}

/// Shell-level panel chrome defaults that flow from the profile rather than the
/// active `StyleSystem`. These are the choices that depend on *which shell
/// structure is active*, not just which style preset.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShellPanelDefaults {
    /// Whether the toolnav (second chrome row) is on by default for this profile.
    /// Overrides `Chrome.toolnav_height > 0` as the default-visible decision;
    /// the user's session override still wins per the existing hybrid logic.
    pub toolnav_default_visible: bool,

    /// Default width (px) for the right/left rail when first opened.
    /// Persisted per-session in `Watchlist::rail_col_width`; this is the factory reset.
    pub rail_default_width: f32,

    /// Whether the workspace rail (left strip) is shown.
    /// When `nav == NavStyle::SideRail`, the workspace rail is merged into the
    /// side-rail and this flag is ignored.
    pub workspace_rail_visible: bool,
}

impl Default for ShellPanelDefaults {
    fn default() -> Self {
        Self {
            toolnav_default_visible: false, // Today's Meridien default (toolnav off)
            rail_default_width: 400.0,
            workspace_rail_visible: true,
        }
    }
}

/// The structural shell variant. Serializable, palette-independent.
///
/// `Default::default()` reproduces today's fixed shell exactly — no visual
/// change for existing workspaces that have no `ShellProfile` section.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct ShellProfile {
    pub nav:      NavStyle,
    pub dock:     DockStyle,
    pub rail:     RailSide,
    pub defaults: ShellPanelDefaults,
}
```

### 2.3 ThemePack integration (proposed — no ThemePack struct yet in codebase)

When the ThemePack concept is formalised, `ShellProfile` loads as an optional section:

```rust
pub struct ThemePack {
    pub style:   StyleSystem,
    pub palette: ColorScheme,
    #[serde(default)]
    pub shell:   Option<ShellProfile>,   // absent = use current_structure()
}
```

Resolution: `shell.unwrap_or_else(ShellProfile::default)`.

---

## 3. Variant-driven vs. Permanently Fixed Regions

### 3.1 Variant-driven (controlled by `ShellProfile`)

| Region | `ShellProfile` field | Rationale |
|---|---|---|
| Top-nav bar | `nav: NavStyle` | The primary navigation shape is the most visible style differentiator. Meridien's Bloomberg-style tall bar is a pure aesthetic choice — no functional coupling. |
| Bottom dock | `dock: DockStyle` | Docking vs. hidden vs. collapsed-pill is a layout preference with no data coupling (dock reads the same `order_manager` / `account_data` regardless of shape). |
| Right (or left) rail | `rail: RailSide` | Side choice is structural but functionally equivalent — all panels use `SidePanelShell` regardless. Left-rail appeals to left-handed and tablet users. |
| Toolnav default-on | `defaults.toolnav_default_visible` | Tied to nav style: a `SideRail` nav typically doesn't need a second chrome row; `TopPills` may or may not. |
| Workspace rail visibility | `defaults.workspace_rail_visible` | When `NavStyle::SideRail`, workspace switching merges into the side strip; otherwise it is a separate collapsible panel. |

### 3.2 Permanently fixed (NOT in `ShellProfile`)

| Region | Why fixed |
|---|---|
| `CentralPanel` (the workspace / chart grid) | Always present; it is the entire point of the app. Zero structural variants are useful. |
| Account strip (`account_strip_open`) | This is **app state** (did the user click the IBKR button this session?), not a layout preference. |
| Object Tree side panel | Floating overlay, not a docked region. Opens as an `egui::Window`. |
| Floating modals (Settings, Spread, Order Health, Connection) | Modal chrome — not shell regions. |
| Per-pane layout (split tree, maximized pane) | Workspace / workspace-level state, not shell structure. |
| `Chrome.toolbar_height_scale`, `Chrome.toolnav_height` px | These are *dimension tokens* inside `StyleSystem.chrome` — they scale the variant, not select it. `ShellProfile` picks the variant; `Chrome` tokens tune its geometry. |

---

## 4. State-Interaction Boundary

### 4.1 ShellProfile carries: structural layout choices

- Which nav shape (`NavStyle`)
- Which dock style (`DockStyle`)
- Which rail side (`RailSide`)
- Factory-reset defaults for rail width and toolnav visibility

`ShellProfile` is **read-only at render time**. The render site branches on `profile.nav`
to select which `egui::TopBottomPanel` / `SidePanel` to register. No mutation inside
the render loop.

### 4.2 App state carries: everything that changes during a session

- `watchlist.sidebar_state.{watchlist_open, orders_open, settings_open, …}` — which panels are open
- `watchlist.rail_col_width` — user-dragged rail width
- `watchlist.bottom_dock_height` — user-dragged dock height
- `watchlist.bottom_dock_tab` — active tab inside the dock
- `watchlist.workspace_nav_expanded` — workspace rail expand/collapse
- `watchlist.toolbar_auto_hide` / `watchlist.toolbar_hover_time` — toolbar visibility state

**These stay in `Watchlist` (frozen). `ShellProfile` does not touch them.**

### 4.3 The frozen-state constraint

Per `docs/adr/0001-canonical-state-model.md` and `src-tauri/CLAUDE.md`: `Watchlist` and
`Chart` are frozen — no new fields may be added to them. New state goes on
`state/ChartState` or `state/aggregates.rs`.

`ShellProfile` itself is not state — it is a config value. It loads from the ThemePack
(or from a future `ShellProfile`-section in the workspace file) and is passed as an
immutable reference to the render functions that branch on it. No `&mut ShellProfile`
at render time; no Watchlist field for it.

---

## 5. Wave-3 Implementation Plan

### 5.1 Priorities (highest visible payoff first)

**Phase 1 — `NavStyle` variants at the render site**

*Highest payoff*: the top-nav bar is the most prominent differentiator between
Meridien and a Bloomberg/IDE look. `TopTabs` and `SideRail` are the target variants.

Branching site: `render_toolbar(...)` in `core.rs:11602` (the 6-line wrapper that calls
`top_nav::render`). The branch would be:

```rust
match profile.nav {
    NavStyle::TopPills => top_nav::render(ctx, ...),   // today's code, unchanged
    NavStyle::TopTabs  => top_nav_tabs::render(ctx, ...), // new module, S6 Wave-3
    NavStyle::SideRail => side_nav::render(ctx, ...),     // new module, S6 Wave-3
    NavStyle::MenuBar  => menu_nav::render(ctx, ...),     // low priority / future
}
```

Each variant module is composed from existing ui_kit primitives:
- `TopTabs` — same `TopBottomPanel` host, but replace the pill-row internals with
  `ui_kit::Tabs` (tab treatment driven by `Chrome.panel_tab_treatment`). The right-side
  panel toggle cluster stays identical.
- `SideRail` — register as `SidePanel::left` instead of `TopBottomPanel::top`. Interior
  uses `Button::icon` for nav items (already available) in a vertical stack. Logo +
  nav items top-anchored; workspace chips bottom-anchored (replaces separate workspace
  rail). Width: 52 px collapsed, 200 px expanded (reuses `workspace_rail.rs` constants
  `COLLAPSED_W` / `EXPANDED_W`).

**Phase 2 — `DockStyle` variants**

Branching site: `top_nav.rs:1649` where `bottom_dock::draw(...)` is called.

```rust
match profile.dock {
    DockStyle::BottomBar  => bottom_dock::draw(ctx, watchlist, account, t),  // unchanged
    DockStyle::BottomPill => bottom_dock::draw(ctx, watchlist, account, t),  // same fn, Chrome.footer_default_open=false
    DockStyle::Hidden     => { /* skip — no TopBottomPanel::bottom registered */ }
}
```

`BottomPill` requires no code change beyond how `footer_default_open` is seeded from
`Chrome` today — this variant is essentially a doc alias for `footer_default_open = false`.
`Hidden` is the valuable new case: it skips the `TopBottomPanel::bottom` registration
entirely, giving the workspace the full vertical height.

**Phase 3 — `RailSide` variants**

Branching site: `top_nav.rs:1657` where `right_rail::render(...)` is called.

```rust
match profile.rail {
    RailSide::Right => right_rail::render(ctx, ...),   // unchanged
    RailSide::Left  => right_rail::render_left(ctx, ...), // new fn, same internals
    RailSide::None  => right_rail::render_floating(ctx, ...), // panels as egui::Window
}
```

`render_left` is a thin wrapper: it registers a `SidePanel::left` before the workspace
rail instead of a `SidePanel::right`. The entire `RailCtx` / `PANELS` dispatch and all
`SidePanelShell` internals are reused unchanged — only the egui host panel type changes.
The workspace rail (`SidePanel::left("workspace_nav_rail")`) must be ordered before the
right-rail-in-left mode or they will conflict; the ordering is already correct in the
call sequence.

### 5.2 Primitive reuse

All three phases compose from existing primitives. No new widget primitives are needed
for Wave-3 Phase 1–3:
- Nav items: `ui_kit::Button::icon` + `Button::menu` (already used in `top_nav.rs`)
- Tabs: `ui_kit::Tabs` (already used in `SidePanelShell`)
- Separators: `ui_kit::Separator::vertical` (already used in `top_nav.rs`)
- Region card chrome: `style::region_frame` / `style::region_gap` (already used in `toolnav.rs` and `bottom_dock.rs`)
- Hover/active column tints: `paint_nav_col_tint` (already in `top_nav.rs`)

S4 recipes (TokenRecipes / `Sx` composites) are not yet widely used in the shell
render path, but `GroupEnclosure` already follows the same intent: the `Sx` recipe
for `GroupEnclosure::Bordered` composes fill + border at the render site. `NavStyle`
variants follow this: each variant's visual treatment is its render-site recipe, not
a set of threaded color/size fields.

### 5.3 ThemePack loading (default = today's shell)

`ShellProfile::default()` produces:
```
nav:   NavStyle::TopPills
dock:  DockStyle::BottomBar
rail:  RailSide::Right
defaults: {
    toolnav_default_visible: false,
    rail_default_width: 400.0,
    workspace_rail_visible: true,
}
```

This is identical to today's fixed shell. Every existing workspace, workspace file,
and style preset that has no `shell:` section loads this default and renders exactly
as today — zero visual regression.

ThemePacks that want a different shell add:
```toml
[shell]
nav  = "TopTabs"
dock = "Hidden"
rail = "Left"
```

---

## 6. Open Questions / Risks for User Review

### Q1 — `SideRail` and the workspace switcher merger

`SideRail` proposes to merge the workspace switcher into the rail (workspaces chips at
the bottom of the left strip). This removes the separate collapsible workspace rail panel.
**Decision needed:** should `SideRail` subsume workspace switching, or should the two
remain independent strips (which could conflict or create awkward double-left-panel egui
ordering)?

### Q2 — `RailSide::None` floating-panel UX

`RailSide::None` means every panel opens as a draggable `egui::Window`. This is a
substantially different interaction model — panels have no docked width or position.
**Decision needed:** is "all panels floating" a target interaction model the product
wants, or is `RailSide::None` merely theoretical? If it's real, the `SidePanelShell`
floating path (`SidePanelShell` already supports floating mode via `Header::dialog` +
`Modal`) needs a session-persistence layer for window positions.

### Q3 — `NavStyle::TopTabs` content scope

`TopTabs` could mean two things: (a) tabs for the nav clusters (Workspace / Layout /
panels) in the existing single-row format, or (b) a Bloomberg-style multi-level menu
bar with top-level section tabs (Charts / Trading / Research / Risk). The latter is a
significantly larger feature. **Decision needed:** which scope for Wave-3?

### Q4 — `ShellProfile` persistence layer

`ShellProfile` is a ThemePack section. But a user might want to override the ThemePack's
shell choice per-workspace ("I want SideRail on my multi-monitor layout but TopPills on
my laptop"). **Decision needed:** should workspaces be able to override `ShellProfile`
independently of the ThemePack? If yes, a `Optional<ShellProfile>` field in the workspace
file format is needed — but workspace file schema changes touch the frozen `Watchlist`
serialization path.

### Q5 — `NavStyle::MenuBar` priority

A native-style menu bar has a very different UX profile (hover-to-open menus, keyboard
navigation, no icon-pill treatment). It's listed as a variant for completeness but is
the most implementation-intensive. **Decision needed:** is this a Wave-3 target at all,
or deferred to a future stream?

### Q6 — Toolnav coupling to `NavStyle`

Today `toolnav_visible()` is a style-default (`Chrome.toolnav_height > 0`) plus session
override. With `ShellProfile`, `defaults.toolnav_default_visible` becomes the factory
default for the active shell. For `NavStyle::SideRail` the toolnav row may be redundant
(chart controls could move to the side-rail). **Risk:** if toolnav is suppressed for
`SideRail` by default, users switching between nav styles mid-session will see the
toolnav appear/disappear — the session override in `style::set_toolnav_override` should
reset to `None` when the nav style changes.

### Q7 — egui `SidePanel` ordering constraint

egui requires that all `SidePanel`s be registered before the `CentralPanel` and that
their order be stable frame-to-frame. `RailSide::Left` + existing `workspace_nav_rail`
(also `SidePanel::left`) means two left panels — egui will stack them. The current call
ordering (workspace rail registered at `core.rs:11606` BEFORE the central panel, right
rail at `top_nav.rs:1657` inside `top_nav::render` which runs BEFORE `CentralPanel`) is
already correct for right-rail. A left-mode right-rail would need to be registered at
the same slot but with a different id — testing is needed to confirm egui stacks two
left panels correctly without width-collision.

---

*This document is ready for user review. No code was written. Wave-3 implementation
begins after the user approves (or revises) the variant set and answers the open
questions above.*
