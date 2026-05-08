# Zed design system audit

## Source
- Repo: `zed-industries/zed` (branch: `main`, sampled 2026-05-06)
- Files reviewed:
  - `crates/theme/src/styles/colors.rs` (ThemeColors)
  - `crates/theme/src/styles/status.rs` (StatusColors)
  - `crates/theme/src/default_colors.rs` (concrete dark theme construction)
  - `crates/ui/src/styles/typography.rs` (TextSize, HeadlineSize)
  - `crates/ui/src/styles/spacing.rs` (DynamicSpacing)
  - `crates/ui/src/components/button/button_like.rs`
  - `crates/ui/src/components/list/list_item.rs`
  - `crates/ui/src/components/disclosure.rs`
  - `crates/ui/src/components/indicator.rs`
  - `crates/ui/src/components/keybinding.rs`
  - `crates/ui/src/components/tooltip.rs`

This audit ignores syntax-highlighting tokens, vim/helix mode tokens, terminal ANSI palette, and player/cursor tokens — none of those map to a trading terminal.

---

## Tokens

### Colors — surface & elevation
Zed has no "primary/secondary/destructive" semantic taxonomy. Surfaces are organized by **elevation** and **role**, with `element_*` tokens layered on top of any surface.

| Zed token | Purpose | Apex equivalent | Action |
|---|---|---|---|
| `background` | Window/empty pane base (step_1) | `theme.bg` | Rename to `surface_0` |
| `surface_background` | Grounded panels, tabs (step_2) | — | **Add `surface_1`** |
| `elevated_surface_background` | Context menus, dialogs, popovers (step_2 in dark; brightens in light) | — | **Add `surface_2`** |
| `panel_background` | Side panels (step_2) | sidebar bg | Alias of `surface_1` is fine |
| `editor_background` | Main content surface (step_1) | chart pane bg | Same as `surface_0` |
| `element_background` | Interactive element default | button idle bg | **Add — replaces ad-hoc bg fills** |
| `ghost_element_background` | Transparent-by-default interactive | toolbar buttons | **Add** — most of our chrome should be ghost |
| `element_hover` | Low-alpha tint on hover (~step_4 alpha) | hardcoded hover colors | **Add as alpha overlay token** |
| `element_active` | Pressed state | — | **Add** |
| `element_selected` | Selected list/tab | active tab tint | **Add** |
| `element_disabled` | Disabled fill | — | Add |
| `ghost_element_hover/active/selected/disabled` | Same states for ghost variant | — | Add (parallel to element_*) |

Key insight: Zed builds elevation from **a 12-step neutral ramp**, with most surfaces being *solid* steps (1, 2) and most interactive states being *alpha overlays* (step_4 alpha) on top. This is why hover never feels jumpy — the underlying surface doesn't change, an overlay just lights up. We currently use solid hover bgs, which is the cause of our slightly cartoony hover.

### Colors — borders
| Zed | Purpose | Action |
|---|---|---|
| `border` | High-contrast structural border | keep `theme.border` |
| `border_variant` | De-emphasized divider (step_5 solid, not alpha) | **Add `border_subtle`** |
| `border_focused` | Keyboard focus ring | **Add** |
| `border_selected` | Selected rows/checkbox | **Add** (use accent at low alpha) |
| `border_transparent` | Reserved slot to prevent layout shift | **Add** — fixes our 1px hover-jump bug pattern |
| `border_disabled` | Disabled outline | Add |

### Colors — text & icons
| Zed | Apex equivalent | Action |
|---|---|---|
| `text` | `theme.text` | keep |
| `text_muted` | `theme.dim` | rename `dim` → `text_muted` for clarity |
| `text_placeholder` | — | **Add** |
| `text_disabled` | — | **Add** |
| `text_accent` | accent text | **Add** |
| `icon` / `icon_muted` / `icon_disabled` / `icon_placeholder` / `icon_accent` | — | **Add the parallel icon ramp** |

We currently treat icon color = text color. Zed splits them so an icon can be muted independently of its label — this is *the* reason their toolbars feel calm.

### Colors — status (StatusColors struct)
Each status has fg + `_background` + `_border` (3 tokens × 11 statuses = 33 tokens).
- `created`, `modified`, `deleted`, `conflict`, `renamed`, `ignored` (VCS — but `created/modified/deleted` are reusable for trade lifecycle)
- `error`, `warning`, `info`, `hint`, `success`, `predictive`

**Apex mapping:**
- `bull` → reuse `created` (green family, "new value")
- `bear` → reuse `deleted` (red family, "value removed")
- `warn` → `warning`
- Add: `info` (used for badges/notices), `hint` (for ML predictions/auto-fills — perfect for our predictive engine outputs), `success` (filled order), `error` (rejected order/connection lost)

### Typography
Zed exposes **two** orthogonal scales:

`TextSize` (UI labels):
- `XSmall` = 10px
- `Small` = 12px
- `Default` = 14px
- `Large` = 16px
- `Ui` / `Editor` = user-configured

`HeadlineSize` (rems, with line-height locked at 1.6):
- `XSmall` 0.88 / `Small` 1.0 / `Medium` 1.125 / `Large` 1.27 / `XLarge` 1.43

Apex currently has 11/13/15/18.

**Action:** Reframe as `TextSize { XSmall=11, Small=12, Default=13, Large=15 }` plus a separate `HeadlineSize { Small=15, Medium=18, Large=22 }`. The split matters because trading dashboards need both dense labels (value tickers) and occasional hero numbers (P&L, account value), and the same enum can't serve both well.

### Spacing — DynamicSpacing
Zed has a density-aware scale (Compact / Default / Comfortable):

| Variant | Compact | Default | Comfortable |
|---|---|---|---|
| Base0/1/2/3 | 0/1/1/2 | 0/1/2/3 | 0/2/4/4 |
| Base4 | 2 | 4 | 6 |
| Base6 | 3 | 6 | 8 |
| Base8 | 4 | 8 | 10 |
| Base12 | 10 | 12 | 14 |
| Base16 | 14 | 16 | 18 |
| Base20–48 | scales by ±4 |

Formula: `(n−4, n, n+4)`.

**Action:** We don't need three densities (yet) but we should adopt the **named-base pattern** instead of magic numbers. A single `Spacing::Base{0,2,4,6,8,12,16,20,24,32}` resolved at theme level lets us shift density globally later without touching components. This is the highest-leverage portable concept in the entire audit.

---

## Component patterns

### Button (`button_like.rs`)
- API: `Button::new(id, label).style(ButtonStyle).size(ButtonSize)` + `.icon()`, `.tooltip()`, `.selected()`, `.disabled()`.
- **Styles:** `Filled` / `Tinted(Accent|Error|Warning|Success)` / `Outlined` / `OutlinedGhost` / `OutlinedCustom(Hsla)` / `Subtle` (default) / `Transparent`.
- **Sizes (height):** `Large 32` / `Medium 28` / `Default 22` / `Compact 18` / `None 16`. Notice these are **smaller** than shadcn (default 36) — Zed expects density.
- Padding from `DynamicSpacing::Base8` (large) or `Base4` (default). Gap = `Base4`.
- Hover for Filled = 50% fade; for Tinted = darken 5%; for Subtle/Outlined = `ghost_element_hover` overlay.
- **Apex gap:** Our buttons mostly mirror Filled. We have no Subtle/Ghost/Tinted. Most toolbar/chart-overlay buttons should be `Subtle`. Trade-action buttons (Buy/Sell) map naturally to `Tinted(Success/Error)`.
- **Action:** Add `ButtonStyle` enum with at least `Filled / Subtle / Ghost / Tinted`. Drop default height to 22px for chrome, 28px for primary actions.

### IconButton
- Just a Button in icon-only mode with mandatory `tooltip()` for a11y. Same size/style enums.
- **Action:** Enforce `tooltip()` at the type level for icon-only construction. We currently allow naked icon buttons.

### ListItem (`list_item.rs`)
- The universal row primitive: trees, sidebars, command palette, autocomplete, settings rows.
- API: `ListItem::new(id).start_slot(icon).child(label).end_slot(badge).indent_level(n).toggle(Some(open)).selectable(true).on_click(...)`.
- Padding: `DynamicSpacing::Base6` horizontal. `indent_step_size = 12px` per level.
- States: idle (no bg) → hover `ghost_element_hover` → selected `ghost_element_selected` → active `ghost_element_active`.
- **Apex gap:** We don't have a primitive row. Our sidebar, watchlist row, order-book row, alerts row, and order ticket row are all hand-rolled with similar-but-divergent paddings. **This is by far the highest-ROI port.**
- **Action:** Build `apex_ui::list_item` modeled exactly on this API. Migrate watchlist row, alerts row, sidebar nav, command palette in that order.

### Disclosure
- A standalone `IconButton` rendering `ChevronRight`/`ChevronDown` at `IconSize::Small` colored `Color::Muted`.
- Layout responsibility lives with the parent (typically a ListItem with `start_slot(disclosure)`).
- **Action:** Trivial port. Add `apex_ui::disclosure` once ListItem lands.

### Indicator
- Three variants: `dot` (1.5×1.5 rounded-full), `bar` (full-width × 1.5 rounded-top), `icon` (8px).
- Optional `border_color` for the dot/bar.
- **Apex gap:** We have no equivalent. Used for: connection status, alert active, unread, position open.
- **Action:** Direct port. Tiny component, immediate value.

### KeyBinding
- Renders chord with platform-aware modifier glyphs (Mac uses Cmd icon, Linux/Win use "Ctrl+" text).
- Per-key chip: `py_0p5`, `rounded_xs`, `text_muted`. Gap between keystrokes = `Base4`.
- **Apex gap:** We have a `Kbd` widget but it doesn't do platform glyphs and renders too prominently (text not muted).
- **Action:** Mute the text color, tighten padding, add platform-aware modifier rendering.

### Tooltip
- Container with `elevation_2` shadow (the *only* elevation Zed uses besides menus). Inner padding `py_1 px_2`.
- Supports trailing `key_binding` slot via `for_action()` factory — automatically pulls the keystroke from the action registry.
- **Action:** Two-slot tooltip (label + optional kbd) with the "fetch from action" pattern. We currently pass the keystroke string by hand everywhere.

### PopoverMenu
Not deeply read here, but the pattern: trigger element + menu builder closure, Zed wires positioning + dismiss-on-outside. We already have egui menus; the porting work is API ergonomics not visuals.

---

## Animation philosophy

Zed animates **almost nothing**. Concrete observations:
- Hover state changes are **instant** — `element_hover` is an alpha overlay that appears the same frame.
- Selection changes are **instant**.
- Focus rings are **instant**.
- Panels reveal/hide with a short slide (where the geometry change would otherwise be jarring).
- Toasts/notifications fade in.
- Long-running operation indicators pulse.

We currently apply our `motion` crate to most state changes (hover fades, selection eases, etc.). **This is the single biggest reason Apex doesn't yet feel like Zed.** Snappy state changes read as "responsive"; eased ones read as "decorative." For a trading tool where a hover delay of even 80ms over a price ladder is unacceptable, we should match Zed's discipline.

**Action:** Carve `motion` down to: panel show/hide, toast in/out, loading shimmer, drawer open/close. Everything else snaps.

---

## The "Zed feel" — what's actually doing the work

1. **Single accent at multiple alphas** — Zed picks one accent and uses it at full strength (focus ring, primary button), 25–40% (selection backgrounds), and 10–15% (hover overlays on accented elements). No second/tertiary semantic colors.
2. **Aggressive `text_muted`** — secondary labels, timestamps, file paths, chevrons, hint text are all muted. Default `text` is reserved for the *thing the user cares about right now*. We currently use full-strength text far too liberally.
3. **Alpha-blended borders and overlays** rather than additional solid color steps. `border_variant` is solid mid-tone, but `element_hover` is alpha — the choice matters.
4. **Zero gradients, zero box shadows except on overlays.** `elevation_2` shows up only on tooltips, popovers, dialogs.
5. **Tight rhythm:** `Base8` between clusters, `Base4` within a cluster, `Base2` for hairline adjustments.
6. **Crisp icons at fixed sizes.** No fade-on-load, no scale-on-hover.

---

## What's portable

Ranked by visible impact per day of work:

1. **`ListItem` primitive** — touches every row in the app. ~2 days, transforms 6+ surfaces immediately.
2. **Spacing scale (Base0…Base32 named tokens)** — half a day, unblocks consistent rhythm across all future components.
3. **Element state tokens (`element_hover/active/selected/disabled`, ghost variants) as alpha overlays** — half a day for the tokens, then ripple through components. Fixes the cartoony hover problem.
4. **Icon color ramp split from text** — half a day. Calms toolbars instantly.
5. **`Indicator` (dot/bar)** — 2 hours. Used in 5+ places already wanting it.
6. **Button style enum (Filled/Subtle/Ghost/Tinted)** — 1 day. Lets us stop hand-coloring buttons.
7. **Animation pruning** — 1 day audit + cuts. Snappier feel everywhere.
8. **Tooltip + KeyBinding integration** — 1 day. Free a11y win.
9. **Surface elevation tokens (surface_0/1/2)** — half a day naming + plumbing.
10. **Status colors (info/hint/success/error/warning) with bg+border triplets** — half a day; enables proper toasts and inline diagnostics.

---

## What's GPUI-only / not portable

- **`elevation_N` shadow primitive** — GPUI does real shadows. egui shadows are weaker; we'll fake with a 1px alpha-blended ring + slight bg lift. Acceptable.
- **`tailwind-style` builder methods (`.py_0p5().rounded_xs()`)** — that's GPUI's styling DSL. egui doesn't compose this way. We just translate the *values*.
- **Action/keystroke registry tying** — Zed's tooltip can ask the action system for the current keybind. We don't have a registered-action system at that level. Half-port: tooltip just takes an optional `KeyBinding`.
- **Player/collab cursor colors** — irrelevant.
- **Vim/helix mode indicators** — irrelevant.
- **Editor-specific tokens** (gutter, indent guides, diff hunks, wrap guides, document highlights) — irrelevant for charts/tables.
- **Density-aware DynamicSpacing across three modes** — overkill for now. Adopt the *naming*, hardcode the Default column, leave Compact/Comfortable as a future toggle.

---

## Recommended extraction order

1. **Phase 1 — Foundation (≈3 days):** spacing scale + surface tokens (surface_0/1/2) + element-state alpha overlays + icon ramp + text_muted everywhere it should be.
2. **Phase 2 — ListItem primitive (≈2 days):** build it, migrate watchlist + alerts + sidebar nav + command palette.
3. **Phase 3 — Button system (≈2 days):** ButtonStyle enum (Filled/Subtle/Ghost/Tinted) + size revision (default 22/28). Migrate toolbar + ticket buttons.
4. **Phase 4 — Indicator + Disclosure + KeyBinding polish (≈1 day total):** small, mostly direct ports.
5. **Phase 5 — Animation prune (≈1 day):** kill non-essential motion; keep panels/toasts/loading.
6. **Phase 6 — Tooltip with key-binding slot (≈1 day):** typed two-slot API, replace ad-hoc tooltip strings.
7. **Phase 7 — Status color triplets + inline diagnostics styling (≈1 day):** wire bull→created, bear→deleted, plus info/hint/success/error/warning fg+bg+border for toasts and order states.

Total: ~11 working days to lift the substance of Zed's system into `ui_kit`. The first three phases (≈7 days) deliver ~80% of the perceptible "Zed feel."

---

## Things Zed does that we should NOT copy

- **`indent_step_size = 12px`** is right for code trees, too wide for compact watchlists. Use 8px in dense contexts.
- **TextSize::Default = 14px** — slightly large for a trading dashboard with thousands of cells visible. Stay at 13px default.
- **Hover = 50% fade for Filled buttons** — fine for editor chrome, wrong for a Buy button where the user must feel certainty. Keep our trade-action hovers crisp (slight darken, no fade).
- **Status colors used for VCS semantics (`renamed`, `ignored`, `conflict_marker_ours/theirs`)** — irrelevant; don't pollute our status enum with them.
- **Editor token sprawl** — Zed has ~40 editor-only tokens. Don't mirror.
- **Alpha-overlay everything** — for the price ladder and DOM, *solid* row backgrounds with crisp transitions are better than alpha overlays which can stack visibly during rapid quote churn. Use the alpha pattern for chrome, not for streaming data rows.

---

## Honest assessment: how much of "the Zed feel" is system vs framework?

About **80% system, 20% framework.** GPUI gives them perfect text rendering, real shadows, and a tailwind-like DSL — those are nice but not load-bearing. The defining choices are all token discipline:

- The single-accent + heavy text_muted hierarchy is a *design choice*, equally achievable in egui.
- Alpha-overlay state changes are a *token choice*, trivial to port.
- The elevation-based surface taxonomy is a *naming choice*, pure rename work.
- The restraint on animation is a *cultural choice*, costs nothing to adopt.
- The density of the spacing scale is a *number choice*.

The 20% that's framework: precise sub-pixel font rendering, real elevation shadows, and the styling DSL's ergonomics. We won't perfectly match those in egui. But matching them isn't required for the app to *feel* like a Zed-class native tool — the token discipline gets us most of the way, and egui's pixel-snapping plus our existing custom font work close most of the rest.

**Bet:** if we land Phases 1–3 above, a side-by-side screenshot test will read as "in the same family as Zed" rather than "egui app trying to look like Zed."
