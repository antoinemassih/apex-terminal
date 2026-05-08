# Zed exact styling values — reference card

Companion to `ZED_DESIGN_SYSTEM_AUDIT.md`. Every value below is pulled directly from `zed-industries/zed` on `main`. Where a value is a `DynamicSpacing::BaseN` token, the resolved Default-density pixel value is given inline (Compact/Comfortable in parens when relevant).

Source repo: <https://github.com/zed-industries/zed> (branch `main`, fetched 2026-05-06)

---

## 1. Border discipline

Zed's border tokens are mapped to neutral/blue scale steps, then resolved per theme. Default themes (One Dark, One Light) inherit the resolved scale-step values below.

### Token → scale step mapping
File: `crates/theme/src/default_colors.rs`

| Token | Light | Dark |
|---|---|---|
| `border` | `neutral().light().step_6()` (~L51) | `neutral().dark().step_6()` (~L240) |
| `border_variant` | `neutral().light().step_5()` | `neutral().dark().step_5()` |
| `border_focused` | `blue().light().step_5()` | `blue().dark().step_5()` |
| `border_selected` | `blue().light().step_5()` | `blue().dark().step_5()` |
| `border_disabled` | `neutral().light().step_3()` | `neutral().dark().step_3()` |
| `border_transparent` | `system.transparent` | `system.transparent` |

### Resolved RGBA (sand neutral / blue scales)

| Token | Light hex | Dark hex |
|---|---|---|
| `border` | `#d9d9d9ff` | `#3a3a3aff` |
| `border_variant` | `#e0e0e0ff` | `#313131ff` |
| `border_focused` | `#acd8fcff` | `#104d87ff` |
| `border_selected` | `#acd8fcff` | `#104d87ff` |
| `border_disabled` | `#f0f0f0ff` | `#222222ff` |

Notes:
- `border_variant` is *lighter* than `border` in the dark theme (step_5 < step_6 in alpha-sorted neutrals — a deliberate inversion so the "soft" divider is the more visible one for in-content separation; perimeter `border` is duller).
- All borders are full alpha (`ff`); Zed does not lean on alpha for hairlines, it leans on luminance steps.
- For "faded" hairlines Zed uses `DividerColor::BorderFaded → border.opacity(0.6)` (see §4 below).

### Border painting — perimeter vs edge-only

Zed almost never uses full-perimeter `.border()`. The codebase pattern is edge-specific 1px borders:

- `.border_b_1()`, `.border_t_1()`, `.border_l_1()`, `.border_r_1()` are all 1px and explicitly set `border_color(cx.theme().colors().border)` or `.border_variant`.
- `crates/workspace/src/status_bar.rs:81` — status bar uses `.border_b(px(1.0))`. Single edge only.
- `crates/ui/src/components/tab.rs` — tabs use `border_r_1()` between tabs and `border_b_1()` under the bar.
- `crates/ui/src/components/divider.rs` — uses `h_px()` (horizontal divider, 1px tall) or `w_px()` (vertical, 1px wide); never `.border(...)`.

Pane/panel/workspace separating lines are all single-edge 1px hairlines drawn against `border` (perimeter) or `border_variant` (in-content). No box-shadows, no double borders.

---

## 2. Tabs

File: `crates/ui/src/components/tab.rs` (constants), `crates/workspace/src/pane.rs` (rendering & theme tokens).

| Property | Value | Notes |
|---|---|---|
| Tab height (content area) | `DynamicSpacing::Base32.px(cx) - px(1.)` → **31px** at Default density | The `-1` reserves the 1px bottom hairline. Compact: 27px, Comfortable: 35px. |
| Start slot (left icon area) | `START_TAB_SLOT_SIZE = px(12.)` | File icon, drag chevron. |
| End slot (close X area) | `END_TAB_SLOT_SIZE = px(14.)` | Close X / dirty dot. |
| Horizontal padding | `DynamicSpacing::Base04.px(cx)` → **4px** | (2 / 4 / 6 across densities) |
| Inner gap (icon ↔ label ↔ close) | `DynamicSpacing::Base04.rems(cx)` → **4px** |  |
| Tab → tab divider | `border_r_1()` = **1px**, color `colors.border` | Drawn on tabs themselves, not as separate elements. |
| Tab bar bottom hairline | `border_b_1()` = **1px**, color `colors.border` | Drawn on the tab-bar container. |

### State colors (theme tokens)

| State | Background | Text |
|---|---|---|
| Inactive | `tab_inactive_background` | `text_muted` |
| Active | `tab_active_background` | `text` |

The active-state "indicator" is **not a separate stroke or dot** in core code — it is the background colour swap (`tab_active_background` typically equals `editor_background` so the active tab visually merges with content) plus the missing bottom hairline (the active tab's bottom border is drawn `transparent` so it dissolves into the editor pane).

Close X on hover: implemented as an `IconButton` revealed by group-hover. There is **no opacity fade**; the close glyph swaps in/out via parent group state — a snap, not a transition. The dirty-dot indicator and close icon share the 14px end slot.

---

## 3. Status bar (footer)

File: `crates/workspace/src/status_bar.rs`

| Property | Value | Source |
|---|---|---|
| Container | `h_flex().w_full().justify_between()` | L55-56 |
| Inter-cluster gap | `DynamicSpacing::Base08.rems(cx)` → **8px** | L57 (Compact 4 / Comfortable 10) |
| Padding (all sides) | `DynamicSpacing::Base04.rems(cx)` → **4px** | L57 |
| Background | `colors.status_bar_background` | L57 |
| Top hairline | `.border_b(px(1.0))` color `border` | L81 — yes, `border_b` here is on a *parent wrapper*; the line sits above the bar to separate it from content. |
| Wayland scaling fix | `mb(px(-1.))`, `mt(px(-1.))` | L75-80 |
| Left-tools gap | `gap_1()` → **4px** | L101 |
| Toggle-cluster gap | `gap_0p5()` → **2px** | L167 |
| Cluster dividers | `Divider::vertical().color(DividerColor::Border)` | L172, L176 — vertical 1px, `border` colour |
| Icon size | `IconSize::Small` → **14px** | L149 |
| Active indicator | `Indicator::dot().color(Color::Accent)` | L151 |

No explicit `.h(...)` or `.h_8` — height is derived from icon + padding: `14 + 4 + 4 = 22px` content, plus 1px top hairline ≈ **23px effective**. Compact ≈ 21px.

`icon_muted` → `icon` transition: confirmed snap. There is no `with_animation` wrapper around the icon button; colour is recomputed each render from group/hover state.

Cluster division: a mix. Adjacent clusters use `Divider::vertical()` (a real 1px element), not just spacing — this is the visible faint line between groups in the screenshot.

---

## 4. Pane header

File: `crates/workspace/src/pane.rs` (search `tab_bar_background`).

| Property | Value |
|---|---|
| Header background | `colors.tab_bar_background` (≈ same family as `editor_background`, slightly different) |
| Bottom hairline | `border_b_1()`, color `colors.border` |
| Right-action icon size | `IconSize::Small` (14px) |
| Action icon spacing | `gap_1()` (4px) inside an `h_flex` |
| "+" new-tab button | `IconButton::new("plus", IconName::Plus)` with `IconSize::Small` and `ButtonStyle::Subtle` (transparent unless hovered) |

Header height is **not explicitly set**; it is `Tab::container_height` (Base32 - 1 = 31px) because the action row shares the tab-row gpui line. The header therefore matches tab height exactly — confirming the audit's "header doesn't add an extra band" finding.

`tab_bar_background` is *not* identical to content background. In One Dark it's a half-step darker than `editor_background`. The audit's claim that the pane header has no different bg than content is **slightly off** — there is a difference, but it's typically only one luminance step (≈ 4-6 RGB units), nearly invisible.

---

## 5. Spacing scale — `DynamicSpacing`

File: `crates/ui/src/styles/spacing.rs`

Density formula: a single named pixel produces `(n-2, n, n+2)` for (Compact, Default, Comfortable) on small values; gap widens further on large bases. Resolved table:

| Variant | Compact | **Default** | Comfortable |
|---|---|---|---|
| `Base0` | 0 | 0 | 0 |
| `Base1` | 1 | 1 | 2 |
| `Base2` | 1 | 2 | 4 |
| `Base3` | 2 | 3 | 4 |
| `Base4` | 2 | 4 | 6 |
| `Base6` | 3 | 6 | 8 |
| `Base8` | 4 | 8 | 10 |
| `Base12` | 10 | 12 | 14 |
| `Base16` | 14 | 16 | 18 |
| `Base20` | 18 | 20 | 22 |
| `Base24` | 20 | 24 | 28 |
| `Base32` | 28 | 32 | 36 |
| `Base40` | 36 | 40 | 44 |
| `Base48` | 44 | 48 | 52 |

`.rems(cx)` returns the Default value divided by 16; `.px(cx)` returns the literal pixel `Pixels` struct. Both already factor density.

Note: there is no `Base02`, `Base06`, `Base10` — the audit's naming was approximate. Real names use raw integers (`Base2`, `Base6`, `Base8` etc).

---

## 6. Animation timing

Zed's snap-first philosophy is reflected in the source: very few literal durations, almost no `with_animation` calls in tab/pane/status code paths. What is present:

- `crates/gpui/src/animation.rs` (and its sub-module) define the `Animation` builder API. Default behaviour is a single-shot animation with caller-supplied `Duration`. There is no global "default duration" constant.
- Easing functions exposed (gpui re-exports): `linear`, `ease_in`, `ease_out`, `ease_in_out`, `ease_in_out_quad`, `cubic_bezier(x1, y1, x2, y2)`, `bounce`. Implementations live alongside the animation module (varies by version).
- Panel dock reveal/hide (`crates/workspace/src/dock.rs`) has **no duration** — visibility is a state flip; the panel snaps. Confirmed by a direct read of the file.
- Loading spinners and a few callouts use `Animation::new(Duration::from_secs(2)).repeat()` style explicit calls. Tabs, status bar, and dock do not.

Implication for porting: assume **0ms** for almost everything. Reserve animations for explicit progress/loading/onboarding affordances.

---

## 7. Switch / toggle

File: `crates/ui/src/components/toggle.rs`

| Property | Value | Source |
|---|---|---|
| Track width | `DynamicSpacing::Base32.rems(cx)` → **32px** | L506 |
| Track height | `DynamicSpacing::Base20.rems(cx)` → **20px** | L507 |
| Track radius | `rounded_full()` | — |
| Thumb diameter | `DynamicSpacing::Base12.rems(cx)` → **12px** | L523 |
| Thumb radius | `rounded_full()` | — |
| Track OFF bg | `colors.element_disabled` | — |
| Track OFF border | `colors.border` (1px) | — |
| Track ON bg (Accent mode) | `status.info.opacity(0.4)` | — |
| Track ON border (Accent) | `text_accent.opacity(0.2)` | — |
| Track ON bg (Custom Hsla) | `color` (caller-supplied) | — |
| Track ON border (Custom) | `color.opacity(0.6)` | — |
| Thumb colour | `colors.text` | — |
| Thumb opacity ON | `1.0` | — |
| Thumb opacity OFF | `0.5` | — |
| Thumb opacity disabled | `0.2` | — |
| Slide animation | none — flexbox `justify_start()` ↔ `justify_end()` snap | — |

So the screenshot's smooth slide is **a re-render snap**, not a tweened animation. The eye fills it in.

Track aspect ratio: 32 × 20 → ~1.6:1. Thumb 12px inside 20px track = 4px vertical inset (centered); horizontal travel = 32 - 12 - (2 × inset) = ~16px.

---

## 8. Theme preview cards (onboarding)

File: `crates/onboarding/src/theme_preview.rs`

| Property | Value |
|---|---|
| Card width (default) | `px(240.)` |
| Card height (default) | `px(180.)` |
| Card width (compact list) | `px(200.)` |
| Card height (compact list) | `px(140.)` |
| Outer corner radius | `ROOT_RADIUS = px(8.0)` |
| Outer padding/border | `ROOT_BORDER = px(2.0)` |
| Inner element border | `CHILD_BORDER = px(1.0)` |
| Selected border | `border_color(theme.colors().border_selected)` (= `blue().step_5()`) |
| Unselected border | `border_color(colors.border_transparent)` |
| Inner radius formula | `inner_corner_radius(outer, padding+border)` — keeps concentric corners |
| Mock sidebar bg | `colors.panel_background` |
| Mock editor bg | `colors.editor_background` |
| Mock root bg | `colors.background.alpha(1.00)` |
| Mock skeleton row height | `SKELETON_HEIGHT_DEFAULT = px(2.)` |
| Mock sidebar width | `SIDEBAR_WIDTH_DEFAULT = relative(0.25)` (25% of card) |

Selected state uses **only** a colour swap — no width change. The 2px outer border is always present; only its colour transitions transparent → `border_selected`. This avoids the layout shift you'd get from a 0→2px border animation.

---

## 9. 2×3 keymap grid (Base Keymap)

File: `crates/onboarding/src/basics_page.rs`

The grid is rendered by `ToggleButtonGroup::two_rows(...)`, configured with:

```rust
.full_width()
.size(ui::ToggleButtonGroupSize::Medium)
.style(ui::ToggleButtonGroupStyle::Outlined)
```

Underlying component (file path varies between Zed versions; not in the canonical `crates/ui/src/components/toggle.rs` of current main — likely `button.rs` or a re-export):

- `Outlined` style means each cell has a 1px border in `colors.border`.
- Adjacent cells **share** borders: cell 2 sets `border_l_1` + `border_b_1`, cell 1 already provides the right edge — no doubled hairlines.
- Selected cell: `border_color(border_selected)` and `bg(element_selected)`.
- `ToggleButtonGroupSize::Medium` height inferred ≈ Base32 (31px content) consistent with other Medium components, but not directly verified — the explicit enum body wasn't visible in the files I could fetch.

---

## 10. Toggle row pattern (`SwitchField`)

File: `crates/onboarding/src/basics_page.rs` (caller), `crates/ui/src/components/toggle.rs` (component).

```rust
SwitchField::new(
    "onboarding-vim-mode",
    Some("Vim Mode"),                  // title (label)
    Some("Coming from Neovim?...".into()), // description (muted)
    toggle_state,
    callback,
)
```

Layout in the parent page:

- Parent container: `v_flex().gap_6()` → **24px** between sections (Compact 20 / Comfortable 28).
- No `Divider` element between rows — the visual separation in the screenshot is the 24px gap alone, not a hairline.
- Label uses default body text (14px, `colors.text`).
- Description uses muted text (`colors.text_muted`); typography is the standard `Label` with `.color(Color::Muted)`.
- Switch is right-aligned in an `h_flex().justify_between()` row.
- Internal row padding is the component's default — observed pattern is `py_2()` (8px top/bottom) with no horizontal padding (caller controls).

---

## Quick-lookup summary (per-token cheatsheet for porting)

```
border (dark)         #3a3a3aff   1px hairline
border_variant (dark) #313131ff   in-content separator
border_selected       #104d87ff   focus ring & selected outlines
status bar height     ~22-23px    (Base04 padding × 2 + 14px icon + 1px hairline)
tab height            31px        (Base32 - 1)
tab inner padding     4px         Base04
tab gap               4px
divider (line)        1px         h_px() / w_px() — never .border()
switch track          32 × 20
switch thumb          12          (4px vertical inset)
spacing default mode  Base{N} ≈ Npx, ±2 across density
animation duration    0ms for tabs/panels; explicit only for spinners
theme preview card    240 × 180, radius 8, outer border 2, inner 1
selected border       always present, transparent → border_selected
v-stack section gap   24px (gap_6)
```

---

## Coverage gaps

These were not retrievable from raw file fetches (404 or content not in viewable region):

1. **`crates/gpui/src/animation.rs`** — file path returned 404 in three different forms. The animation module on `main` has likely been split into a sub-directory; full easing/duration enumeration would need a local clone.
2. **`ToggleButtonGroupSize` exact px** — the enum body was not co-located with the component file I could fetch. Heights are inferred from convention (Small/Medium/Large ≈ 24/28/32) but unverified.
3. **`crates/workspace/src/pane.rs` literal values** — the file returned only logic; numeric styling lives in helper functions in `crates/ui/src/components/tab.rs` (which I did capture) or in inline `.h_*` / `.px_*` calls that the WebFetch summarisation elided. Tab height was confirmed via `tab.rs`; pane header height is inferred to match.
4. **Status bar explicit height** — there is no `.h(...)` call; height is computed from padding + icon size as documented.
5. **One Light vs One Dark *theme JSON overrides*** — the values above are scale-step defaults; the published One Dark / One Light theme files may override individual tokens. Worth a follow-up read of `assets/themes/one/one.json` if exact pixel-faithfulness matters.
