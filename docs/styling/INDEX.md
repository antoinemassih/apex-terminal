# Apex Terminal — Styling Index ("where-is-X")

The Rust/egui native trading terminal renders almost everything as immediate-mode painter calls. Most "magic numbers" are literals at the call site, with a small set of shared tokens in `src-tauri/src/chart_renderer/ui/style.rs` and theme colors in `src-tauri/src/ui_kit/theme.rs`.

This file is the entry point for restyling work. For each visible element, find:

- **What** — one-line description
- **Where** — file:line(s) where it is painted
- **Inputs** — current font sizes, colors, padding, stroke widths, corner radii
- **Levers** — what to change to alter the look

All paths absolute. All paths in `src-tauri/src/`.

---

## 0. Global tokens & theme — change these first

| Concern | File | Notes |
|---|---|---|
| Font sizes (`FONT_XS`..`FONT_2XL`), spacing (`GAP_*`), radii (`RADIUS_*`), strokes (`STROKE_*`), alpha tiers (`ALPHA_*`), shadows (`SHADOW_*`), `TEXT_PRIMARY/SECONDARY` | `chart_renderer/ui/style.rs:23-119` | Single source of truth. Both `const`s and `font_xs()` accessor fns. Const values: `FONT_XS=8, FONT_SM=10, FONT_MD=11, FONT_LG=12, FONT_XL=13, FONT_2XL=14`. `RADIUS_SM=4, MD=6, LG=12`. `STROKE_HAIR=0.3, THIN=0.5, STD=1.0, BOLD=1.5, THICK=2.0`. |
| Helpers — `tb_btn`, `popup_frame`, `dialog_window`, `dialog_window_themed`, `dialog_header`, `panel_frame`, `panel_frame_compact`, `tab_bar`, `segmented_control`, `icon_btn`, `close_button`, `panel_header`, `tooltip_frame`, `stat_row`, `paint_tooltip_shadow`, `action_btn`, `trade_btn`, `small_action_btn`, `simple_btn`, `order_card`, `status_badge`, `split_divider`, `dashed_line` | `chart_renderer/ui/style.rs:135-744` | All site-wide widgets. Restyle here to repaint everywhere. |
| 8 chart themes (`Midnight`, `Nord`, `Monokai`, `Solarized`, `Dracula`, `Gruvbox`, `Catppuccin`, `Tokyo Night`) with `bg`, `toolbar_bg`, `toolbar_border`, `bull`, `bear`, `bull_volume`, `bear_volume`, `grid`, `axis_text`, `ohlc_label`, `accent`, `crosshair` | `ui_kit/theme.rs:7-83` | The canonical theme palette. Used as `t.bg`, `t.text` (does **not** exist on `ChartTheme`; many sites use `t.text` from a wider `Theme` struct — check call sites), `t.bull`, `t.bear`, `t.accent`, `t.dim`, `t.toolbar_bg`, `t.toolbar_border`. |
| Drawing colors (8 presets) | `ui_kit/theme.rs:86-95` | Used by drawing tools. |
| Spacing constants `TOOLBAR_HEIGHT=28`, `PADDING_RIGHT=80`, `PADDING_TOP=4`, `PADDING_BOTTOM=30`, `STYLE_BAR_WIDTH=480` | `ui_kit/theme.rs:98-102` | High-level layout. |
| Custom fonts loaded — Inter, DM Sans, Geist, Plus Jakarta, Space Grotesk, JetBrains Mono | `ui_kit/*.ttf` | Font selection happens in egui style setup (search `setup_theme` in `chart_renderer/gpu.rs:3163`). |
| Design-mode token overrides (`dt_f32!`, `dt_u8!`, `dt_i8!` macros) | `src-tauri/src/design_tokens.rs` | When `design-mode` feature is on, every token reads from a global runtime struct. Default values match the consts. |

Most "magic numbers" in chart paint code are literals — see categories below.

---

## 1. Top toolbar

The whole top toolbar (logo, theme/layout/draw menus, panel toggles, window controls).

- **Toolbar bar** — `chart_renderer/gpu.rs:3614-3650`
  - `exact_height = 30.0` (compact) or `38.0` normal — line `3617`
  - Fill: `t.toolbar_bg`; left margin 8px; bottom border `Stroke(1.0, t.toolbar_border)` line `3637-3640`
  - Paper-mode green underline `Stroke(4.0, rgb(46,204,113))` line `3645-3648`
  - Auto-hide threshold `8.0` trigger zone, `500ms` linger — line `3590-3601`
- **Toolbar buttons** — `tb_btn` helper at `chart_renderer/ui/style.rs:157-191`
  - Font: `monospace 12.0`, height 24, radius 4, stroke 0.8 (line `170-172`)
  - Active: accent fill at `ALPHA_TINT(30)`, accent border, 1.5-px underline at `ALPHA_DIM(60)` (line `175-180`)
  - Hover: 1-px white-10 bevel (line `183-188`)
- **Logo** — `chart_renderer/gpu.rs:3654-3660` — 14×14 painter slot.
- **Theme + Style two-column dropdown** — `chart_renderer/gpu.rs:4604-4653`
  - Trigger label: monospace 12 strong, color `t.dim` (line `4609`)
  - "THEME" / "STYLE" mini-headers: monospace `8.0`, color `t.dim*0.5` (line `4617`, `4638`)
  - Theme swatch: `16×14` rect, radius 2; bull/bear circles r=`2.5` (line `4621-4624`)
  - Theme/Style row text: monospace `11.0`, accent if selected else `t.dim` (line `4627-4628`, `4644-4645`)
  - Min widths: theme col 160, style col 120; vertical separator spacing 8 (line `4616`, `4637`, `4634`)
- **Layout dropdown, draw dropdown, panel toggles** — same `tb_btn` helper, sequence in `render_toolbar` (`chart_renderer/gpu.rs:3558` onward through ~`5040`).

**To restyle the whole toolbar**: change `tb_btn` body in `style.rs:157-191`, the toolbar `Frame` in `gpu.rs:3615-3617`, and theme palette `toolbar_bg`/`toolbar_border` in `ui_kit/theme.rs`.

---

## 2. Chart pane

Painted from `render_chart_pane` at `chart_renderer/gpu.rs:5258` onward.

### 2.1 Pane header (background + active-pane underline)

- **What** — the strip at the top of every chart pane; holds the link-dot, nav arrows, tabs/symbol, and add-tab/template buttons.
- **Where** — `chart_renderer/gpu.rs:5345-5470`
- **Inputs**
  - Header height: `pane_top_offset` from `pane_header_h()` / `pane_tabs_header_h()` at `gpu.rs:101-122` (driven by `wl.pane_header_size` enum: `Compact`/`Normal`/`Large`).
  - Header bg: `t.bg.gamma_multiply(0.6)` if active else `*1.2` (line `5363-5367`).
  - Active-pane accent underline: `3.0px` solid `t.accent` (line `5371-5382`); style 1 (Aperture) uses `2.0px` at `ALPHA_HEAVY+ ` (140) — line `5374-5378`.
- **Levers** — `pane_header_h()` (`gpu.rs:101`), `header_bg` formula (`gpu.rs:5363`), the underline thickness/alpha block (`gpu.rs:5371-5382`).

### 2.2 Link-group dot, back/forward nav

- **Link dot** — `gpu.rs:5386-5405`. Diameter `10px`. Group palette: blue/green/orange/purple at `gpu.rs:5392-5397`.
- **Back/Forward arrows** — `gpu.rs:5409-5462`. Square `18×18`, radius `3.0`, `Icon::CARET_LEFT/RIGHT` at proportional `12.0px`, hover bg `color_alpha(toolbar_border,60)`.

### 2.3 Tab bar (within pane)

- **Tabs (active/inactive/hover/close)** — `chart_renderer/gpu.rs:5465-5783`
- **Inputs**
  - Title font: `pane_header_size.title_font()` (Compact ~11, Normal ~12, Large ~13) — see `chart_renderer/mod.rs` `PaneHeaderSize` enum.
  - Price font: `title_font_size - 1.0`, min `9.0` (line `5468-5469`).
  - Tab height: `pane_top_offset - 2.0` (line `5470`).
  - Tab padding: `tab_pad = 10.0`, `gap = 6.0`, `close_w = 14.0`, inter-tab gap `1.0` (line `5505-5507`, `5665`).
  - Tab bg: active `t.bg*0.5`, hover `*0.75`, idle `*0.9` (line `5540-5546`).
  - Active accent bottom border: `Stroke(2.0, t.accent)` (line `5552-5557`).
  - Inactive separator: 0.5-px `t.dim*0.3` vertical hairline (line `5558-5564`).
  - Symbol bold-draw: 0.5-px x-offset double-text (line `5581-5589`); inactive color `t.dim*0.7`.
  - Price color: `t.bull`/`t.bear`; inactive alpha 180, active 255 (line `5632-5635`).
  - Close button: `×` `FontId::proportional(13.0)`, hover bg `color_alpha(t.bear, 30)` radius 2 (line `5645-5660`).
  - Corner radius (top corners): `4` (line `5549`).

### 2.4 Option-pane badges in tab/header (C/P + 0D/{N}D)

- **Tab variant** — `chart_renderer/gpu.rs:5597-5629`
- **Header variant** — `chart_renderer/gpu.rs:5855-5884`
- **Inputs** — badge font `monospace(9.5)`, dark fg `rgb(24,24,28)`, fill `color_alpha(t.bull|t.bear|t.accent, 200)`, radius 3, height `min(row_h-6, 16)`. Width: galley + 8 (call/put) or +6 (DTE).
- **Levers** — change `badge_font` size, `dark_fg`, the colored fill alpha (200), and corner radius (3.0) at the lines above.

### 2.5 Simple symbol header (no tabs)

- **What** — when only one tab, the original single-symbol header.
- **Where** — `chart_renderer/gpu.rs:5818-5965`
- **Inputs** — title font `title_font_size`, label color `t.bull` if active else `t.text`. Light bold via 0.5-px x-offset double-draw; Octave style adds extra (line `5847-5852`).
- Price displayed separately at `cursor_x` after badges (line `5894-5906`).

### 2.6 "+ Tab" button and "T" template button

- **+ Tab** — `gpu.rs:5670-5718` (tab mode) and `gpu.rs:5942-5965` (no-tabs mode). Width `44`, radius `4`, hover bg/border tied to `ALPHA_SUBTLE`/`ALPHA_LINE`/`ALPHA_MUTED`. Label "+ Tab" font `monospace(title-2, min 9)`.
- **T (template)** — `gpu.rs:5787-5816` and `5910-5940`. Width `22`, uses `Icon::STAR`, active state lights up at `ALPHA_ACTIVE`.

### 2.7 Chart-area top-left badge strip (TF pill, OV button)

- **Where** — `chart_renderer/gpu.rs:12823-12869`
- **Inputs**
  - Padding `pad=6`, badge height `bar_h=18`.
  - TF pill: monospace `10.0`, bg `t.bg*0.4`, radius `3.0`, padding `+10` (line `12832-12839`).
  - OV button: width `32`, monospace `10.0`, radius `3`, stroke `0.5 t.toolbar_border`. Active bg `color_alpha(t.accent, 60)`.

### 2.8 Chart bars (candles, wicks, Heikin-Ashi)

- **Main candle batch** — `chart_renderer/gpu.rs:6650-6900` (wick + body meshes). Wick half-width `0.5`. Bodies built via `egui::Mesh`.
- **Heikin-Ashi / alt batch** — `chart_renderer/gpu.rs:6604-6647`.
- **Single-candle drawing for overlays** — `gpu.rs:6909-6925`.
- **Selected-candle highlight** — `gpu.rs:11695-11703`.
- Colors come from `t.bull` / `t.bear` and `t.bull_volume` / `t.bear_volume`. To change candle look, edit theme colors and the half-width / bar-spacing math near `bs` and `bw` in `gpu.rs:6650+`.

### 2.9 Y-axis ticks + last-price badge

- **Y ticks (price labels right of chart)** — `chart_renderer/gpu.rs:6240-6254`
  - Grid line: `Stroke(0.5, t.dim.gamma_multiply(0.3))` (line `6245`).
  - Label font: `monospace(11.5)`, color `t.text`, bolded by 0.5-px double-draw (line `6248-6251`).
  - Position: `rect.left()+cw + 3.0..3.5`.
- **Last-price horizontal level line** — `gpu.rs:6266-6274`. Dashed, alpha 28, dash pitch `3px on / 7px off` (`dx += 10`).
- **Last-price Y-axis badge** — `gpu.rs:6276-6313`
  - Font `monospace(13.0)`, fg = price color × 0.15, bg = full price color. Padding 4×2. Radius `2.0`. Triangle pointer width 4. Bold by 0.5-px double-draw.
- **Levers** — font size at `gpu.rs:6248,6279`; padding `pad_x=4, pad_y=2` (line `6286-6287`); radius `2.0` (line `6294`); colors via theme `bull`/`bear`/`dim`. Right-edge offset of axis: `pr` derived from `chart.padding_right` token at `gpu.rs:6129` (default `80.0 * 0.525`).
- Mini "price-axis right edge" alt site at `gpu.rs:9934-9950` (52×14 badge, monospace via `painter.text`).

### 2.10 X-axis time labels

- **Where** — `chart_renderer/gpu.rs:6319-6358`
- **Inputs** — labels at `y = rect.top()+pt+ch-10`, font `monospace(8.0)`, color `t.dim.gamma_multiply(0.6)` (line `6353-6354`). Min label spacing `min_label_px=70` (line `6322`). Format `MM/DD` for daily intervals or `HH:MM` otherwise.
- **Levers** — font size 8 and dim multiplier `0.6` at line `6354`; spacing at `6322`.

### 2.11 Crosshair price label (cursor pill)

- **Where** — `chart_renderer/gpu.rs:12263-12299`
- **Inputs**
  - Crosshair lines: `Stroke(0.5, color_alpha(t.text, 50))` horizontal + vertical (line `12267-12268`).
  - Price pill: font `monospace(13.0)` white, bg `Color32::from_rgba_unmultiplied(20,20,26,240)`, border `Stroke(1.0, color_alpha(t.text,80))`, radius `3.0`, padding 5×2, bolded by 0.5-px double-draw.
  - Crosshair time tag (bottom): font `monospace(8.0)`, alpha-160, bg `t.toolbar_bg`, padding 3×1 radius 2.
- **Levers** — pill color tuple at `12279`, font size `12271`, padding/radius at `12273-12279`.

### 2.12 Active-pane border indicator

- Pane outline rect when multi-pane: `gpu.rs:5318-5322`. Active stroke `1.5px t.bull*0.8`; inactive `0.5px t.dim*0.3`.
- Plus the 3-px accent **header underline** (see 2.1).

---

## 3. Drawing-tool middle-click picker

- **Main picker frame** — `chart_renderer/gpu.rs:15820-15924`
  - Width `140`. `Frame::popup` fill `t.toolbar_bg`, stroke `Stroke(1.0, t.toolbar_border)`, inner_margin `8`, radius `6`.
  - "FAVORITES" label: monospace `9.0`, color `t.dim` (line `15838-15839`).
  - Favorite cells: 3-col grid, `cell_w=cell_h=floor((avail-2*3)/3)`, gap 3, radius 5. Active bg = `t.accent*0.30`, hover `t.toolbar_border*0.55`. Stroke 1.5/0.7. Icon font `proportional(cell_w*0.55, min 11)`.
  - "ALL TOOLS" rows: 20-px tall, monospace `10.0`, hover bg `t.accent*0.18`, caret right at `proportional(11)`.
- **Flyout submenu** — `gpu.rs:15925-16045`. Width `180`, frame matches main picker (radius 6, stroke 1.0). Lives just to the right of the main picker.
- **Levers** — picker frame at `15831-15835`; cell metrics at `15843-15846`; flyout dimensions at `15940-15946`.

---

## 4. Command palette

- **Where** — `chart_renderer/ui/command_palette.rs`
- **Top-level frame** — corner radius `10.0` at `:299`.
- **Modes** — `draw_normal_mode` (`:373`), `draw_ai_mode` (`:326`), `draw_help_mode` (`:682`).
- **Inputs**
  - Search "⌕" glyph size 14 (`:387`); AI title 14 strong; hint text size 11 monospace; result rows mostly `9.0` and `8.0` monospace.
  - "✦ Gemma 4" badge: size 9, radius 9 (`:401-404`).
  - Hotkey labels: monospace `9.5`, color `t.dim` (`:611`).
- **Levers** — palette width passed in as `pal_w` (search call site in `gpu.rs` for `command_palette::draw`); font sizes are literal across `command_palette.rs:380-720`.

---

## 5. Connection panel

- **Where** — `chart_renderer/ui/connection_panel.rs:11-90`
- **Frame** — `Stroke(1.0, color_alpha(t.toolbar_border, ALPHA_ACTIVE))`, radius `RADIUS_LG (12)`, inner_margin 0 (`:22-24`).
- **Section title** — monospace 7, color `t.dim*0.5` (`:32`).
- **Service rows** — name monospace 9 strong (`:75`), detail monospace 8 dim*0.45 (`:83`).

---

## 6. Diagnostics panel

- **Where** — `chart_renderer/ui/apex_diagnostics.rs:11-209`
- Frame radius `8.0` (`:26`); icon font proportional 9 (`:73`); row labels monospace 8.5 (`:202`).

---

## 7. Side panels — common scaffolding

All side panels use `style::panel_frame` or `panel_frame_compact` (`style.rs:137-150`) for their outer frame: fill `t.toolbar_bg`, inner margins `8/8/8/6` (or `6/6/6/5` compact), border `Stroke(1.0, color_alpha(t.toolbar_border, ALPHA_HEAVY=120))`. Most panel headers use `panel_header(title, accent, dim)` (`style.rs:454-471`) — monospace `11.0` strong title.

### 7.1 Watchlist (`chart_renderer/ui/watchlist_panel.rs`, 2171 lines)

Largest panel; heavy paint. Most numbers are literals at the call site.

- **Header / session pill / list selector** — `:50-150`
  - Session label monospace `8.5` strong (`:50`); list combobox active text monospace `9.0` accent (`:95`).
  - Add `+` button proportional `12.0` (`:147`).
- **Search box** — desired_width `search_w`, font `monospace(11.0)` (`:205`).
- **Filter pills** — monospace 8.0, fill via `color_alpha(side_col, ALPHA_GHOST)` (`:328-369`).
- **Pinned section** — row height `30.0`, `section_h = pinned*30+6` (`:457`).
- **Row body** — `:825-960`
  - **Active indicator stripe** — 2.5-px-wide strip on left, color `t.accent` (`:832-836`).
  - **RVOL left border strip** — variable width `2/3/4` px and color (orange/green/blue) keyed to RVOL bucket (`:844-857`).
  - Grip dots: proportional 9, color `t.dim*0.2` (`:859-860`).
  - Pin star: `Icon::SPARKLE` proportional 9, color `rgb(255,193,37)` if pinned (`:862-869`).
  - Symbol text: monospace `sym_font_sz`, position based on hover/pinned offset (`:874-875`).
  - Earnings pill "E:N": monospace 7, fill `rgb(255,193,37)`, radius 6 (`:880-888`).
  - Alert dot: r=5.5, fill `rgb(231,76,60)` (`:891-895`).
  - **RVOL badge text** — monospace 7, color tier orange/bull/dim (`:931-937`).
  - **Day-range bar** — width 24, line stroke 2.0 muted, dot r=2.5 (`:941-950`).
  - Price right-aligned, proportional font (`:952-955`).
- **Levers** — row height (`:457`), RVOL stripe widths (`:846-852`), badge font sizes (literal `7.0`/`8.5`), the per-bucket color thresholds at `:845-852` and `:933-934`.

### 7.2 Orders panel

- **Where** — `chart_renderer/ui/orders_panel.rs:10` (`fn draw`)
- Cards use `style::order_card` (`style.rs:572-596`) — left 3-px accent stripe, fill bg, margin 9/6/5/5, radius 4. Restyle these in `style.rs`.
- Row chips: `color_alpha(close_color, 12)` radius `RADIUS_SM` (`:92-114`).

### 7.3 DOM sidebar

- **Where** — `chart_renderer/ui/dom_panel.rs`
- Header font `monospace(8.5)` (`:84`); +/- buttons `monospace(8.0)` (`:104,110`); price column main font `monospace(12.5)`, secondary `9.0` (`:243-244`); inline price text `monospace(11.0)` (`:307`).
- Order tag side font `monospace(9.0)`, qty `monospace(12.0)` — `:431-434`.

### 7.4 Time & Sales tape

- **Where** — `chart_renderer/ui/tape_panel.rs:10-100`
- Row font `monospace(8.5)` (`:58`).

### 7.5 Spread builder

- **Where** — `chart_renderer/ui/spread_panel.rs:217+`
- Outer frame radius `RADIUS_LG (12)` (`:236`); leg cards radius `RADIUS_MD (6)` (`:280`); leg-X close `monospace(12)` (`:350`); side/op-type chips `color_alpha(_, ALPHA_GHOST)` radius `RADIUS_SM` (`:362, 379`).

### 7.6 Option chain panels (0DTE / Far)

- The 0DTE quick picker: `chart_renderer/ui/option_quick_picker.rs:28-330`. Frame inner_margin `GAP_LG`, radius `RADIUS_LG` (`:63-64`).
- Full chain rows are rendered as part of `watchlist_panel.rs` and `chart_widgets.rs` (search "chain_0dte" / "OptionRow"). Strike rows use the same row-height & font pattern as watchlist rows.

### 7.7 Object tree

- **Where** — `chart_renderer/ui/object_tree.rs:102+`
- Row buttons min size `30×18`, radius `RADIUS_SM` (`:389`).

### 7.8 Settings panel

- **Where** — `chart_renderer/ui/settings_panel.rs:11+`
- Outer stroke `STROKE_STD (1.0)`, radius `RADIUS_LG (12)` (`:23`).
- Toggle buttons min size `34×20` radius `RADIUS_SM` (`:159, 388, 403`).
- Pill sub-buttons `38×18` (`:304`).
- Type-label monospace 7 (`:209`).

---

## 8. Trading widgets

### 8.1 Order entry / order panel body

- **Where** — `chart_renderer/gpu.rs:912-1210` (`render_order_entry_body`)
- **Width** — `panel_w = 270` advanced else `210` (`:11223`); floating variant `300/230` (`:12924`); footprint variant `270/210` (`:7735`).
- **Order-type segmented row** — `:932-952`. Buttons `monospace(8.0)`, height 18, end-corner radius 3, mid-corner 0; selected fill `color_alpha(t.accent,60)`, idle `color_alpha(t.toolbar_border,25)`. Stroke 0.5.
- **TIF row** — frameless monospace 8 selectable buttons (`:955-963`).
- **Last-price reference** — monospace 12 dim (`:1044`).
- **Buy/Sell action buttons** — call `style::trade_btn` (`style.rs:615-636`) — bg = color × 0.55, hover bg color × 0.7, height 24, radius 3, white monospace `11.0` strong.

### 8.2 Order ticket DOM ladder slide-out

- **Where** — `chart_renderer/gpu.rs:11352-11600` (inside `if chart.dom_open`)
- Tick size `0.01` (or `1.0` for indices) at `:11369`.
- Live NBBO inside row sized by `live_bid_sz` / `live_ask_sz` (`:11371-11378`).
- Row colors derived from `t.bull` / `t.bear`; quantity glyphs `egui::FontId::monospace(9..12)` (search `painter.text` in this block).

### 8.3 Floating strike-order panes / footprint mini panels

- Same `render_order_entry_body` at narrower `panel_w=210` (`gpu.rs:7735`).
- Drag handle at `gpu.rs:11252-11349` — clamps `order_panel_pos` to chart rect.

### 8.4 Bid/ask pills (in tabs and DOM)

- No dedicated "bid/ask pill" widget; bid/ask rendering happens inline in DOM (`dom_panel.rs:243-307`) and live-quote fetches at `gpu.rs:11356-11367`. To add pill chrome, mirror the option C/P badge code at `gpu.rs:5605-5614`.

---

## 9. Popups / modals

### 9.1 Symbol picker (live search)

- **Where** — `chart_renderer/ui/trendline_filter.rs:87-307` (note: file misnamed; the picker lives here).
- Frame: `corner_radius RADIUS_LG (12)`, inner_margin 6 (`:193-194`).
- Search input font `monospace(11.0)` (`:200`).
- Result rows: monospace 9 with name in `rgb(200,200,210)` (`:40`).
- **Levers** — frame `:193-194`; fonts `:40, 200`.

### 9.2 Option quick picker

- **Where** — `chart_renderer/ui/option_quick_picker.rs:28-330`
- Frame inner_margin `GAP_LG (8)`, radius `RADIUS_LG (12)` (`:63-64`).

### 9.3 Template popup

- **Where** — `chart_renderer/ui/template_popup.rs:9-230`
- "TEMPLATES" header monospace `FONT_LG (12)` strong accent (`:42`).

### 9.4 Generic picker — `chart_renderer/ui/picker.rs`

Currently a stub (1 line). All real picker logic lives in `trendline_filter.rs` (symbol) and `option_quick_picker.rs` (options).

### 9.5 Indicator editor

- **Where** — `chart_renderer/ui/indicator_editor.rs:10-520`
- Outer corner_radius 6 (`:38`).
- Indicator rows: button height 22, internal sub-buttons 22×18 / 26×18 / 28×18 / 22×14 / 24×22 (`:88-499`); radius 2 or 3.
- Trash button: fill `color_alpha(del_color, ALPHA_GHOST)`, radius 3 (`:499`).

### 9.6 Hotkey editor

- **Where** — `chart_renderer/ui/hotkey_editor.rs:9-110`
- Key chip: fill `key_bg`, radius 3, min `80×18` (`:97`).

### 9.7 Drawing-tool middle-click picker

See section 3.

---

## 10. Dialogs / shadows / cards (cross-cutting)

- `dialog_window` / `dialog_window_themed` — `style.rs:207-234`. Themed variant: fill `t.toolbar_bg`, border 1.0 `color_alpha(t.toolbar_border, 80)`, radius 12, drop shadow `offset=[0,8] blur=28 spread=2 alpha=80`.
- `dialog_header` — `style.rs:236-266`. Auto-darkens window fill by 8 units; title monospace `font_lg()` strong; X close at `font_xl()`. Top corners radius 12.
- `tooltip_frame` — `style.rs:503-509`. Inner padding 8, radius 8.
- `order_card` — `style.rs:572-596`. Stripe width 3, radius 4, side margins 9/6, vertical 5.
- `status_badge` — `style.rs:562-569`. Font 8, height 16, radius `RADIUS_SM`.

---

## 11. Quick reference — common style edits

| To change… | Edit |
|---|---|
| Global font sizes | `chart_renderer/ui/style.rs:35-40` (`FONT_*`) |
| Global spacing | `style.rs:51-57` (`GAP_*`) |
| Global corner radii | `style.rs:64-66` (`RADIUS_*`) |
| Theme palette (any color) | `ui_kit/theme.rs:26-83` |
| Toolbar height | `chart_renderer/gpu.rs:3617` (`38.0` / `30.0`) |
| Pane header height | `chart_renderer/gpu.rs:101-122` (`pane_header_h`) |
| Tab geometry | `chart_renderer/gpu.rs:5505-5507`, `5540-5557` |
| Active-pane underline thickness | `chart_renderer/gpu.rs:5371-5382` |
| Y-axis tick font | `chart_renderer/gpu.rs:6248` |
| Last-price badge | `chart_renderer/gpu.rs:6276-6313` |
| Crosshair pill | `chart_renderer/gpu.rs:12271-12283` |
| TF / OV badges | `chart_renderer/gpu.rs:12823-12869` |
| Toolbar button look | `style.rs:157-191` (`tb_btn`) |
| Trade buttons (BUY/SELL) | `style.rs:615-636` (`trade_btn`) |
| Watchlist row | `chart_renderer/ui/watchlist_panel.rs:825-960` |
| Symbol-picker frame | `chart_renderer/ui/trendline_filter.rs:193-200` |
| Drawing picker grid | `chart_renderer/gpu.rs:15843-15890` |

---

## 12. Files NOT covered (stubs)

These files currently contain a single line and forward to their real implementations elsewhere:

- `chart_renderer/ui/chart_pane.rs`, `drawings.rs`, `indicators.rs`, `oscillators.rs`, `panels.rs`, `picker.rs`, `orders.rs`, `toolbar.rs`, `watchlist.rs` — all 1-line stubs. The real code is in `gpu.rs`, `chart_widgets.rs`, `trendline_filter.rs`, etc.

When grepping for a feature, search the parent `gpu.rs` first (20K lines, contains the bulk of paint code) and the named `*_panel.rs` files second.

---

## 13. Suggested workflow for a visual restyle

1. Decide whether the change is global (touch `style.rs` / `theme.rs` only) or local (touch the call site).
2. Run the project in **design-mode** feature so token edits are live (`dt_f32!`, `dt_u8!` macros at `style.rs:26-114`).
3. For chart-pane elements, edit `chart_renderer/gpu.rs` at the line ranges above. For panels, edit the per-panel file.
4. For new tokens, prefer adding to `style.rs` and replacing the literal at the paint site.

End of index.
