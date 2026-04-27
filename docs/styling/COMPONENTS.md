# Chart Renderer Component Catalog

A reference of every reusable visual primitive in `src-tauri/src/chart_renderer/`.
Use this when you need to answer **"I want to change how X looks — where do I edit?"**.

Most reusable helpers live in `chart_renderer/ui/style.rs`. Painter-based widgets
(spinners, mini badges, drawing primitives) live in `chart_renderer/ui/chart_widgets.rs`.
Chart-specific badges that are not factored into helpers (last-price tag, option C/P
badge, link-group dot, connection pill) are inlined inside `gpu.rs` — they are
documented here with their exact line ranges.

All sizes/colors flow through the design-tokens system (`design_tokens.rs` macros
`dt_f32!`, `dt_u8!`, `dt_i8!`); the constants below are the **fallback defaults** when
no token override exists. The token-fed scalar accessors are at the top of `style.rs`:

- Font sizes: `font_xs..font_2xl` — `style.rs:26-31` (8 / 10 / 11 / 14 / 15 / 15 px).
- Spacing: `gap_xs..gap_3xl` — `style.rs:43-49` (2 / 4 / 6 / 8 / 10 / 12 / 20 px).
- Radii: `radius_sm/md/lg` — `style.rs:60-62` (3 / 4 / 8 px).
- Strokes: `stroke_hair..stroke_thick` — `style.rs:69-73` (0.3 / 0.5 / 1.0 / 1.5 / 2 px).
- Alpha ramp: `alpha_faint..alpha_heavy` — `style.rs:82-92` (10 / 15 / 20 / 25 / 30 / 40 / 50 / 60 / 80 / 100 / 120).
- Shadow: `shadow_offset/alpha/spread` — `style.rs:108-110` (2 / 60 / 4).
- `color_alpha(c, a)` shorthand — `style.rs:540`.
- `mono` / `mono_bold` — `style.rs:124-131` (one-line `RichText` builders, monospace).

---

## H2: Buttons

There are **six** distinct button helpers. Pick by visual weight:

### `tb_btn` — Toolbar button (top-strip / chart-header style)
- **File:line:** `ui/style.rs:157`
- **Anatomy:** monospace 12 px label, 24 px tall, 4 px corner radius, 0.8 px stroke.
  - **Active:** fill = `color_alpha(accent, alpha_tint)` (≈30α), border = `color_alpha(accent, alpha_active)` (100α), bottom 1.5 px accent underline at 60α.
  - **Inactive:** fill = `color_alpha(toolbar_border, alpha_ghost)` (15α), text = `dim`.
  - **Hover:** PointingHand cursor + 1 px white bevel highlight on top edge (alpha 10).
- **Caller-controlled:** label, active flag, accent, dim, toolbar_bg, toolbar_border.
- **Hardcoded:** font size 12 (NOT `font_lg()` despite the comment), height 24, radius 4, stroke 0.8, hover bevel alpha 10.
- **Top call sites:**
  - `gpu.rs:3578` — internal closure that powers all of the main toolbar.
  - `gpu.rs:3583` — `tb_btn` outer closure (used everywhere via `tb_btn_tip`).
  - `gpu.rs:4594` — layout dropdown caret.
  - `gpu.rs:4787` — "+ Window" button.
  - via `tb_btn_tip` — every panel toggle (Orders, DOM, Order Entry, Settings, Feed, Playbook, Watchlist, Analysis…), e.g. `gpu.rs:3696,3701,3705,3744,4744,4751,4756,4762,4767`.

### `tb_btn_tip` — Toolbar button + hover tooltip (variant)
- **File:line:** `gpu.rs:3582` (closure, not in `style.rs`).
- **Difference vs `tb_btn`:** thin wrapper that adds `.on_hover_text(tip)` and threads click state through `TB_BTN_CLICKED`. Same visuals.
- **Top call sites:** `gpu.rs:3672,3696,3701,3705,3835,4744,4751`.

### `icon_btn` — Square icon button
- **File:line:** `ui/style.rs:427`
- **Anatomy:** Square hit-box of `(size + 8).max(22)` px on each side, **frameless** (`.frame(false)`), zero internal padding (sets `button_padding = 0`). On hover paints a `radius_sm` (3 px) fill at `alpha_ghost` (15α) plus a `stroke_thin` border at `alpha_muted` (40α). Cursor → PointingHand.
- **Caller-controlled:** icon glyph, color, size.
- **Hardcoded:** padding 0, min-side `(size+8).max(22)`, hover alpha (15/40), hover radius `radius_sm`.
- **Top call sites:**
  - `ui/style.rs:259` — close-X inside `dialog_header_colored`.
  - `ui/alerts_panel.rs:153` — cancel pending alert.
  - `ui/alerts_panel.rs:229` — clear active alert.
  - `ui/object_tree.rs:220` — lock/unlock toggle.
  - `ui/object_tree.rs:246` — delete-trash icon (red).
  - `ui/indicator_editor.rs:58`, `ui/option_quick_picker.rs:92,107`, `ui/orders_panel.rs:255,320`.

### `close_button` — Pre-themed `icon_btn(X, dim, font_lg)` (variant of icon_btn)
- **File:line:** `ui/style.rs:449`
- **Difference:** convenience wrapper. `Icon::X` glyph, dim color, `font_lg()` (14 px).

### `action_btn` — Tinted action button (Place / Cancel / Clear All)
- **File:line:** `ui/style.rs:601`
- **Anatomy:** monospace 9 px **strong**, 20 px tall, 3 px radius, 0.5 px stroke.
  - **Enabled:** fill = `color_alpha(color, alpha_muted)` (40α), text = `color`, border = `color_alpha(color, alpha_active)` (100α).
  - **Disabled:** fill `alpha_faint` (10α), text `alpha_active` (still 100α but on a near-transparent fill it reads as muted), border `alpha_line` (50α).
- **Hardcoded:** font 9, height 20, radius 3, stroke 0.5.
- **Top call sites:** `gpu.rs:5076` (Create), `ui/orders_panel.rs:204,209,216,236,249`, `ui/plays_panel.rs:462`.

### `trade_btn` — BUY/SELL deep-saturated button
- **File:line:** `ui/style.rs:615`
- **Anatomy:** 24 px tall, 3 px radius, label = white **bold** monospace 11 px, fill = input color × `button.trade_brightness` (default 0.55). On hover, repaints fill at `button.trade_hover_brightness` (0.7) + over-paints the white label at `font_lg` (14 px) — a small "punch in" effect.
- **Caller-controlled:** label, base color, width.
- **Top call sites:** `gpu.rs:1151` (BUY), `gpu.rs:1186` (SELL).

### `small_action_btn` — Inline header micro-button ("Clear All", "+ New Play")
- **File:line:** `ui/style.rs:639`
- **Anatomy:** 16 px tall, `radius_sm` (3 px), `stroke_thin` (0.5 px) border. Fill = `alpha_soft` (20α), border = `alpha_dim` (60α), text = `font_sm` strong.
- **Top call sites:** `ui/alerts_panel.rs:133,156,198,278`, `ui/plays_panel.rs:28,286,290,498`, `ui/orders_panel.rs:61,223`.

### `simple_btn` — Form action button (Create / Cancel)
- **File:line:** `ui/style.rs:651`
- **Anatomy:** 18 px tall, `radius_sm`, `stroke_thin`. Fill `alpha_faint` (10α), border `alpha_muted` (40α), text non-strong `font_sm`. Caller sets `min_width`.
- **Top call sites:** `ui/scanner_panel.rs:127`.

---

## H2: Pills, Badges, Status Indicators

### `status_badge` — Generic status pill ("DRAFT", "PLACED", "TRIGGERED")
- **File:line:** `ui/style.rs:562`
- **Anatomy:** rendered as an `egui::Button` (so it becomes a hit target — note: it is technically clickable). Font `dt_f32!(badge.font_size, 8.0)` strong, height `dt_f32!(badge.height, 16.0)`, radius `radius_sm` (3 px), `stroke_thin`. Fill = `color_alpha(color, alpha_subtle)` (25α), border = `color_alpha(color, alpha_dim)` (60α).
- **Caller-controlled:** text + tint color.
- **Top call sites:** `ui/alerts_panel.rs:151,227,249,300,320`, `ui/orders_panel.rs:324,380,420`, `ui/connection_panel.rs:78`.

### Option **C/P** badge (chart-pane header)
- **File:line:** inlined in `gpu.rs:5855-5870`
- **Anatomy:** small filled rectangle next to the symbol when `chart.is_option`. Height = `(header_height - 6).min(16)`, padded width = glyph + 8 px, radius 3, fill = `t.bull` (call) or `t.bear` (put) at alpha 200, foreground = dark `Color32::from_rgb(24,24,28)`. Single character "C" or "P", monospace 9.5 px, centered.
- **Caller-controlled:** none — chart-pane intrinsic. Edit colors via `t.bull/t.bear`, or replace the constants in `gpu.rs:5859,5866`.

### Option **DTE** badge (`0D`, `5D`, …)
- **File:line:** `gpu.rs:5871-5883`
- **Anatomy:** same geometry as C/P badge but fill = `color_alpha(t.accent, 200)`. Width = glyph + 6 px. Label format: `"0D"` if `dte<=0` else `"{n}D"`.
- **Caller-controlled:** colors via `t.accent`.

### Last-price Y-axis tag (with arrow pointer)
- **File:line:** `gpu.rs:6256-6315`
- **Anatomy:** rectangle hugging the right edge of the chart (`rect.left+cw+1`), 2 px corner radius, fill = directional color (`t.bull` if up-on-day else `t.bear`), foreground = same color × 0.15 (very dark, tinted). Padding 4 × 2. Includes a 3-vertex left-pointing triangle arrow into the chart at `(badge.left-4, price_y)`. Label is double-drawn at +0.5 px x for poor-man's bold.
- **Companion:** faint dashed level line spanning the chart at `alpha=28` of the same color (`gpu.rs:6266-6274`).
- **Caller-controlled:** `t.bull/t.bear/t.dim` from theme. Font hardcoded 13 px monospace.

### Y-axis price tick labels
- **File:line:** `gpu.rs:6244-6253`
- **Anatomy:** plain right-aligned label, 11.5 px monospace, double-drawn at +0.5 px for bold effect, color `t.text`. Grid line at `stroke_thin` 0.5 px, color `t.dim × 0.3`.

### Crosshair price label
- **File:line:** crosshairs are in `gpu.rs:11784-11940` (dashed accent/blue/gold variants). The price label component is computed from `cursor_price = min_p + (max_p - min_p) * (1 - rel_y)` (`gpu.rs:11904`). Each crosshair variant inlines its own tag rendering — there is no extracted helper.

### Connection status pill (toolbar)
- **File:line:** `gpu.rs:4725-4741`
- **Anatomy:** 20 × 20 hit area, painted as a `circle_filled` of radius 3 at the center.
  - Connected: `Color32::from_rgb(46, 204, 113)` (green).
  - Disconnected: `Color32::from_rgb(230, 160, 40)` (amber).
- Hover: PointingHand + tooltip "Connection: OK" / "Connection: Issue".
- Click: opens `connection_panel`.

### Breaker badge (REST circuit-breaker readout)
- **File:line:** `ui/apex_diagnostics.rs:92` (`section_connection`); state from `apex_data::rest::breaker_snapshot()` at line 95; "reset breaker" button at `apex_diagnostics.rs:32`.
- Rendered as plain dim labels — there is no badge frame; it is text only.

### Link-group colored dot (chart-pane top-left)
- **File:line:** `gpu.rs:5385-5405`
- **Anatomy:** 10 px circle. When `link_group ∈ 1..=4`, painted filled with one of:
  - 1 = `rgb(70,130,255)` blue
  - 2 = `rgb(80,200,120)` green
  - 3 = `rgb(255,160,60)` orange
  - 4 = `rgb(180,100,255)` purple
- When unlinked (0): painted as a 1 px ring at `t.dim × 0.4`. Click cycles 0→1→2→3→4→0.

### Mini-widget badge (collapsed chart_widget card)
- **File:line:** `ui/chart_widgets.rs:560` (`draw_mini_badge`)
- **Anatomy:** dark pill, fill `Color32::from_rgba_unmultiplied(0,0,0,40)`, radius 4. Left-aligned 7 px monospace label at `t.dim × 0.5`; right-aligned `font_sm` value in the per-widget color from `mini_summary` (`chart_widgets.rs:576`).

---

## H2: Order/List Cards

### `order_card` — Side-stripe row card (orders, alerts, plays)
- **File:line:** `ui/style.rs:572`
- **Anatomy:** Frame with caller-supplied bg, inner margins `card.margin_left=9`, `right=6`, `y=5`; corner radius `card.radius` (4 px). Renders a 3 px-wide vertical accent **stripe** (`card.stripe_width`) along the entire left edge with rounded NW/SW corners. Caller-controlled: `accent` (stripe color), `bg`, content closure. Returns `bool` for click.
- **Top call sites:** `ui/alerts_panel.rs:146,221,243,294,314`; `ui/orders_panel.rs:81,306,374,415`.

---

## H2: Frames, Popups, Dialogs, Tooltips

### `panel_frame` / `panel_frame_compact` — Sidebar frames
- **File:line:** `ui/style.rs:137` (standard) / `:145` (compact).
- **Anatomy:** `Frame::NONE`, fill `toolbar_bg`, stroke `stroke_std` (1 px) at `alpha_heavy` (120α) of `toolbar_border`. Standard inner margin = `(gap_xl, gap_xl, gap_xl, gap_lg)` = (10,10,10,8); compact = `(gap_lg, gap_lg, gap_lg, gap_md)` = (8,8,8,6).
- **Top call sites:** `ui/alerts_panel.rs:22`, `ui/analysis_panel.rs:34`, `ui/feed_panel.rs:31`, `ui/journal_panel.rs:42`, `ui/orders_panel.rs:24`, `ui/signals_panel.rs:28`, `ui/playbook_panel.rs:21`.

### `popup_frame` — Floating popup window (no titlebar)
- **File:line:** `ui/style.rs:196`
- **Anatomy:** `Frame::popup()` + caller fill, inner margin `gap_lg` (8 px). Optional border. No corner radius set — inherits style default.

### `dialog_window` / `dialog_window_themed` — App-quality dialogs (variants)
- **File:line:** `ui/style.rs:207` / `:218`
- **Anatomy:** zero inner padding, `radius_lg` (8 px) corners. `dialog_window` hardcodes fill `rgb(26,26,32)` and `rgba(60,60,70,80)` border. `dialog_window_themed` uses `toolbar_bg`, 12 px corners, and adds a rich shadow `offset=[0,8] blur=28 spread=2 alpha=80`.

### `dialog_header` / `dialog_header_colored` — Title bar with X close
- **File:line:** `ui/style.rs:237` / `:242`
- **Anatomy:** Frame with auto-darkened bg (`bg − dialog.header_darken=8`), inner margin `(12,10,10,10)`, top-only `radius_lg` (12 px) corners. Title = `font_lg` strong; X uses `icon_btn` at `font_xl` with `dim × 0.7`.

### `tooltip_frame`
- **File:line:** `ui/style.rs:503`
- **Anatomy:** fill `toolbar_bg`, `stroke_thin` border at `alpha_strong` (80α), `tooltip.padding=8`, `tooltip.corner_radius=8`. Pair with `paint_tooltip_shadow` (`style.rs:522`) for the drop shadow under painter-based tooltips.

### `stat_row` — Single labeled value line inside a tooltip
- **File:line:** `ui/style.rs:512` — label left at `tooltip.stat_label_size=8`, value right strong at `tooltip.stat_value_size=10`.

---

## H2: Headers, Tabs, Sections, Labels

- **`panel_header`** / **`panel_header_sub`** — `ui/style.rs:454` / `:459`. Title `font 11` strong accent; optional subtitle `font 9` dim; right-aligned `close_button`.
- **`tab_bar`** — `ui/style.rs:474`. Frameless `font_lg` strong button per tab; active gets a bottom-edge fill bar of thickness `tab.underline_thickness=2` in `accent`. Top call sites: `ui/orders_panel.rs:29`, `ui/settings_panel.rs:32`, `ui/watchlist_panel.rs:38`.
- **`section_label`** — `ui/style.rs:325`. Tiny 7 px **strong** monospace.
- **`dim_label`** — `ui/style.rs:331`. `font_sm` regular.
- **`dialog_section`** — `ui/style.rs:313`. Indented `font_sm` strong with left margin.
- **`col_header`** — `ui/style.rs:337`. Fixed-width `font_xs` dim cell, right-align switch for numeric vs text columns.
- **`form_row`** — `ui/style.rs:547`. Right-aligned fixed-width label (`font_sm` dim) + content closure on the right.

---

## H2: Separators

- **`separator`** — `ui/style.rs:272`. Full-width line, `stroke_thin`. Bottom space `separator.after_space=1`.
- **`dialog_separator`** — `ui/style.rs:281`. Same line, but inset by `margin` on both sides.
- **`dialog_separator_shadow`** — `ui/style.rs:291`. Inset line + 3 fading dark lines underneath at alphas `[20, 12, 4]` (overridable via `shadow.gradient` token); bottom space `separator.shadow_space=4`. Top sites: `gpu.rs:11113,11180,11448`, `ui/indicator_editor.rs:344,483`, `ui/overlay_manager.rs:73`.
- **`split_divider`** — `ui/style.rs:713`. 6 px tall draggable handle. Inactive: 1 px line at `alpha_faint` of dim. Active (hover/drag): 2 px line at `dim × 0.6` + three center dots (radius 1.5, spacing 8) at `dim × 0.4` + `ResizeVertical` cursor. Returns drag-delta y.

---

## H2: Segmented Control

### `segmented_control`
- **File:line:** `ui/style.rs:356`
- **Anatomy:** pill group with a sunken trough behind it.
  - Trough: caller's `toolbar_bg` darkened by `segmented.trough_darken=12`, radius `radius_md+1`, `stroke_thin` border at `alpha_strong` of `toolbar_border`. Expanded horizontally by `segmented.trough_expand_x=4`.
  - Buttons: 20 px tall (`seg_btn_h`), padding-x 5, monospace 12 strong, no stroke. Active fill = `color_alpha(accent, alpha_tint+5)` ≈ 35α. Per-button corner radii are computed so only the **first** and **last** segments round on the outer corners (`radius_sm`).
- **Caller-controlled:** active index, label list, theme colors.
- **Top call sites:** `gpu.rs:3724` (timeframe), `gpu.rs:4588` (layout).

---

## H2: Spinner / Loading

- **`draw_refined_spinner`** — `ui/chart_widgets.rs:3073`. Two rounded-rect "tiles" chasing each other around a square's perimeter; period ~1.6 s. Caller passes `painter`, `center`, `radius`, `color`.
- **`refined_spinner`** — `ui/chart_widgets.rs:3124`. egui-flow wrapper that allocates space and calls `draw_refined_spinner` at 0.42 × size. Requests repaint each frame.
- **`draw_loading_skeleton`** — `ui/chart_widgets.rs:3062`. Spinner + small "Loading…" caption at `font_xs` dim × 0.4.
- **Top call sites:** `gpu.rs:6143`, `ui/discord_panel.rs:153,319,451`, `ui/trendline_filter.rs:206`.

---

## H2: Sparklines / Mini Visualizations

There is **no shared sparkline helper**. Each occurrence is inlined:

- Watchlist row sparkline: `ui/watchlist_panel.rs:910-` (32 px wide).
- Command-palette result preview: `ui/command_palette.rs:820`.
- Per-widget chart bodies inside `chart_widgets.rs` — see the long list of `draw_*` painter helpers (`chart_widgets.rs:1303-3500`): `draw_trend_gauge`, `draw_momentum_gauge`, `draw_volume_profile`, `draw_volume_shelf`, `draw_signal_radar`, `draw_payoff_chart`, `draw_correlation`, `draw_dark_pool`, `draw_position_pnl`, `draw_news_ticker`, etc. Each is fully self-contained and theme-aware via `&Theme`.
- Donut/arc primitives for those widgets: `donut_ring` (`chart_widgets.rs:1274`), `draw_arc` (`:1207`), `draw_arc_ring` (`:2296`).

---

## H2: Drawing Primitives

- **`dashed_line`** — `ui/style.rs:665`. Dashed (6 on / 3 off) or dotted (2 / 2) segments along an arbitrary line. Skips out-of-bounds (`len < 1` or `> 20000`).
- **`draw_line_rgba`** — `ui/style.rs:687`. Thick line into an RGBA buffer, used for procedural icon generation.
- **`hex_to_color(hex, opacity)`** — `ui/style.rs:530`. Hex-string → `Color32` with opacity (0..1).
- **`color_alpha(c, a)`** — `ui/style.rs:540`. Re-emit a color with replaced alpha byte.
- **`paint_tooltip_shadow`** — `ui/style.rs:522`. Black-alpha 60 fill offset by `(shadow_offset, shadow_offset)` behind the tooltip rect.

---

## H2: Resize / Drag Handles

- **`resize_handle`** (chart_widget grip) — `ui/chart_widgets.rs:3500`. 12 × 12 bottom-right corner, three diagonal pen strokes spaced 3 px apart at `stroke_thin` × `alpha_muted` of `t.dim`. Cursor → `ResizeNwSe`. Returns the drag delta or `None`.

---

## H2: How to change a look — quick map

| Want to change… | Edit here |
|---|---|
| All toolbar buttons | `ui/style.rs:157` (`tb_btn`) |
| Status pills (DRAFT / PLACED / TRIGGERED) | `ui/style.rs:562` (`status_badge`) |
| Order row look (stripe, padding, radius) | `ui/style.rs:572` (`order_card`) + tokens `card.*` |
| Last-price Y-axis tag | `gpu.rs:6256-6315` |
| Crosshair price labels | `gpu.rs:11784-11940` (per-mode inline) |
| Option **C / P** badge | `gpu.rs:5855-5870` |
| **0D / nD** expiry badge | `gpu.rs:5871-5883` |
| Link-group dot colors | `gpu.rs:5392-5396` |
| Connection green/amber dot | `gpu.rs:4730-4733` |
| Mini-widget collapsed badge | `ui/chart_widgets.rs:560` (`draw_mini_badge`) |
| Tooltip frame | `ui/style.rs:503` (`tooltip_frame`) + tokens `tooltip.*` |
| Sidebar panel frame | `ui/style.rs:137` (`panel_frame`) |
| Spinner motion | `ui/chart_widgets.rs:3073` (`draw_refined_spinner`) |
| Segmented-control trough | `ui/style.rs:356` + tokens `segmented.*` |
| Default font / radius / spacing scale | `ui/style.rs:26-92` (and `design_tokens.rs` overrides) |

---

## H2: Token Cheat Sheet

When a value reads `crate::dt_f32!(badge.font_size, 8.0)` it means:
look up `badge.font_size` in the runtime design-tokens table; if absent, use `8.0`.
Override paths used in the catalog above include:
`badge.font_size`, `badge.height`, `card.margin_left/right/y`, `card.radius`,
`card.stripe_width`, `tooltip.padding`, `tooltip.corner_radius`,
`tooltip.stat_label_size`, `tooltip.stat_value_size`, `segmented.trough_darken`,
`segmented.trough_expand_x`, `dialog.header_darken`, `tab.underline_thickness`,
`split_divider.height/inset/dot_radius/dot_spacing/active_stroke/inactive_stroke`,
`separator.after_space`, `separator.shadow_space`, `shadow.gradient`,
`button.trade_brightness`, `button.trade_hover_brightness`,
`form.row_height`, `table.header_height`.

To preview overrides at runtime, enable the `design-mode` feature and edit
tokens through the inspector — see `design_tokens.rs`.
