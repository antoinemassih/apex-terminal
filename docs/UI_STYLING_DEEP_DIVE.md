# Apex Terminal — UI & Styling Deep Dive

Complete extraction of the styling architecture, design tokens, component system, and UI patterns.

---

## Overview

Apex Terminal is a trading terminal UI built with **Egui** (GPU-accelerated immediate-mode UI) + **Tauri** (Rust desktop app wrapper). The styling architecture is **two-axis**:

1. **ColorScheme** (palette only) — 9 semantic tone tokens
2. **StyleSystem** (dimensions only) — typography, spacing, strokes, radii, alphas, shadows

These two axes are independent: you can swap a color scheme without changing dimensions, and vice versa. The composable utility layer is called **Sx** (similar to Tailwind CSS).

---

## 1. Directory Structure (All UI/Styling Files)

```
apex-terminal/
├── src-tauri/
│   ├── design.toml                        # Design token defaults
│   └── src/
│       ├── design_system/
│       │   ├── builtin.rs (74KB)          # 15 built-in ColorSchemes
│       │   ├── color_scheme.rs            # ColorScheme struct + Rgba type
│       │   ├── style_system.rs (36KB)     # StyleSystem struct (all dimension tokens)
│       │   ├── baseline.rs                # Baseline StyleSystem
│       │   ├── loader.rs                  # JSON/TOML theme loading
│       │   ├── registry.rs                # Theme registry + selection
│       │   ├── snapshot.rs                # Per-frame token snapshots
│       │   ├── hot_reload.rs              # Live design-mode updates
│       │   ├── equivalence_tests.rs       # Design system validation
│       │   └── export.rs                  # Export themes to JSON
│       │
│       └── ui_kit/
│           ├── style.rs (250+ lines)      # Token helper functions (stateless)
│           ├── tokens.rs                  # Token enums (Size, Variant, Density)
│           ├── cursor.rs                  # Cursor definitions
│           ├── icons.rs (16KB)            # Icon glyph registry
│           ├── symbols.rs                 # Symbol rendering
│           ├── [fonts].ttf (20+ files)    # Embedded fonts (Inter, JetBrains Mono, etc.)
│           │
│           ├── sx/
│           │   └── style.rs (250+ lines)  # Sx builder + Fill/BorderSpec enum
│           │
│           └── widgets/ (100+ components)
│               ├── button.rs (65KB)
│               ├── button_style.rs (28KB)
│               ├── theme.rs (19KB)
│               ├── input.rs (27KB)
│               ├── select.rs (39KB)
│               ├── modal.rs (27KB)
│               ├── sheet.rs (15KB)
│               ├── tabs.rs (46KB)
│               ├── panel.rs (4.7KB)
│               ├── panel_section.rs (40KB)
│               ├── panel_sub_section.rs (16KB)
│               ├── panel_list_row.rs (37KB)
│               ├── panel_card.rs (4.2KB)
│               ├── panel_key_value_row.rs
│               ├── panel_empty.rs
│               ├── panel_loading.rs
│               ├── panel_error.rs
│               ├── panel_toolbar.rs
│               ├── table.rs (25KB)
│               ├── table_header.rs
│               ├── pane_grid.rs (45KB)
│               ├── badge.rs (5.2KB)
│               ├── tag.rs (10KB)
│               ├── count_chip.rs (8.7KB)
│               ├── status_pill.rs (6.1KB)
│               ├── checkbox.rs
│               ├── radio.rs
│               ├── toggle_group.rs
│               ├── switch.rs
│               ├── slider.rs
│               ├── range_slider.rs (11KB)
│               ├── number_stepper.rs
│               ├── color_picker.rs (28KB)
│               ├── tooltip.rs (15KB)
│               ├── alert.rs (7.9KB)
│               ├── toast.rs
│               ├── skeleton.rs
│               ├── spinner.rs
│               ├── header.rs (11KB)
│               ├── label.rs (4.7KB)
│               ├── polished_label.rs (10KB)
│               ├── breadcrumb.rs
│               ├── sidebar.rs (18KB)
│               ├── form_section.rs
│               ├── form_field.rs
│               ├── form_row.rs (11KB)
│               ├── separator.rs (5.1KB)
│               ├── scroll_area.rs (7.2KB)
│               ├── context_menu.rs (18KB)
│               ├── segmented_control.rs (12KB)
│               ├── pagination.rs
│               ├── stepper.rs
│               ├── tree.rs (15KB)
│               ├── heatmap_grid.rs
│               ├── sparkline.rs (6.1KB)
│               ├── shadow.rs (10KB)
│               ├── shadow_pipeline.rs (36KB)
│               ├── motion.rs (26KB)
│               ├── text_engine.rs (34KB)
│               ├── icon_placement.rs (16KB)
│               ├── trade_card.rs (5.8KB)
│               ├── progress.rs (10KB)
│               ├── risk_reward_bar.rs (3.7KB)
│               ├── selectable_row.rs (6.5KB)
│               ├── kbd.rs (4.3KB)
│               └── indicator.rs (7.6KB)
│
├── figma/
│   ├── aperture.tokens.json (8.4KB)       # W3C DTCG design tokens
│   ├── component-inventory.md             # Component → code mapping
│   └── README.md                          # Token import workflow
│
└── docs/
    ├── DESIGN_SYSTEM.md
    ├── UI_AUDIT.md
    ├── UI_EXTRACTION.md
    └── STATE_SYSTEM.md
```

---

## 2. Design Token System (`design.toml`)

### Font Sizes
```toml
[font]
xxs          = 8.3px    # Extra-small eyebrow labels
xs           = 11.3px   # Table cells, column headers
sm_tight     = 12.9px   # Dense body text
sm           = 13.7px   # Small body
md           = 13.6px   # Medium (primary reading size)
input        = 14.4px   # Form inputs
lg           = 15.1px   # Large heading
xl           = 16.9px   # Extra-large display
xxl          = 17.0px   # Display heading
display      = 29.9px   # Hero price/numbers
display_lg   = 38.8px   # Largest display
```

### Spacing Scale
```toml
[spacing]
xs       = 2px     # Micro gap (hairlines)
xs_mid   = 6px     # Between xs (4px) and sm (8px)
sm       = 4px     # Tight gaps
md       = 6px     # Default gap
lg       = 8px     # Button padding standard
xl       = 10px    # Panel padding
xxl      = 12px    # Large padding
xxxl     = 20px    # Extra-large spacing
```

### Corner Radii
```toml
[radius]
xs    = 2px     # Minimal rounding (inputs)
sm    = 3px     # Buttons, chips
md    = 4px     # Cards
lg    = 8px     # Large panels
# full = 999px  # Pills, nav tabs, footer
```

### Stroke / Border Weights
```toml
[stroke]
hair    = 0.3px    # Sub-pixel hairline (dividers)
thin    = 0.5px    # Subtle borders
medium  = 0.8px    # Mid-weight
std     = 1.0px    # Standard border
bold    = 1.5px    # Emphasis
thick   = 2.0px    # Heavy emphasis
xheavy  = 5.0px    # Very thick
```

### Alpha / Opacity Tiers (0–255 scale)
```toml
[alpha]
faint   = 10     # Near-invisible overlay (hover shimmer)
ghost   = 15     # Barely visible
soft    = 20     # Disabled states
subtle  = 25     # Low-emphasis overlay
tint    = 30     # Icon/chip accent tint
muted   = 40     # Primary dimming (~60% opacity)
line    = 50     # Structural lines
dim     = 60     # Border/line dimming
strong  = 80     # Selected row fill
active  = 100    # Interactive element alpha
heavy   = 120    # Near-opaque overlay
solid   = 200    # High-opacity element
```

### Component-Specific Measurements
```toml
[toolbar]
height                = 36px
height_compact        = 28px
btn_min_height        = 24px
btn_padding_x         = 7px
right_controls_width  = 150px

[panel]
margin_x              = 10px
margin_top            = 10px
margin_bottom         = 8px
width_sm              = 240px
width_md              = 260px
width_default         = 280px
width_lg              = 300px
width_xl              = 320px

[table]
header_height         = 12px
row_height            = 20px
row_height_compact    = 18px
item_height           = 36px

[card]
margin_left           = 9px
margin_right          = 6px
margin_y              = 5px
radius                = 4px
stripe_width          = 3px
width_sm              = 200px
height_sm             = 48px
height_md             = 52px

[badge]
font_size             = 8px
height                = 16px

[button]
action_height         = 24px
trade_height          = 30px
small_height          = 18px

[icon_button]
icon_padding          = 5px
min_size              = 26px

[tooltip]
corner_radius         = 8px
padding               = 8px
stat_label_size       = 8px
stat_value_size       = 10px
```

### Shadow Presets
```toml
[shadow_preset.card]
offset = [0, 2]     # Modest lift
blur   = 4
spread = 0
alpha  = 60

[shadow_preset.modal]
offset = [0, 8]     # Deep overlay
blur   = 28
spread = 2
alpha  = 80

[shadow_preset.tooltip]
offset = [0, 2]     # Subtle
blur   = 0
spread = 0
alpha  = 60

[shadow_preset.dropdown]
offset = [0, 8]     # Menu/popover
blur   = 24
spread = 1
alpha  = 40
```

---

## 3. Color Palette System

### Core 9-Tone Palette (Aperture Default, `aperture.tokens.json`)
```json
{
  "accent":  "#EF5B3B",                  // Flamingo red — brand, CTAs, ticker symbols
  "bull":    "#4EC07A",                  // Emerald green — positive, up %, bid side
  "bear":    "#D8503E",                  // Valencia red — negative, down %, ask side
  "warn":    "#F5C64A",                  // Cream Can gold — caution, pending
  "text":    "#F4ECE0",                  // Merino beige — primary text, active tabs
  "dim":     "#B6AD9D",                  // Nomad tan — secondary text, inactive
  "border":  "rgba(255, 255, 255, 0.06)", // White 6% — hairline dividers
  "surface": "#1A1816",                  // Cod Gray — panel surface, footer dock
  "bg":      "#000000"                   // Black — app canvas
}
```

### Extended Color Tokens
```json
{
  "dim-2":                    "#76705F",          // Pablo — deepest dim (balances)
  "surface-raised":           "#272822",          // Heavy Metal — toolbar elevation
  "surface-enclosure":        "#1A1612",          // Zeus — pill-group fill
  "enclosure-translucent":    "rgba(26,22,18,0.6)", // 60% Zeus
  "border-variant":           "#1F1D1A",          // Stronger secondary border
  "bull-bright":              "#34D399",          // Shamrock — depth numerics (green)
  "bear-bright":              "#F87171",          // Froly — ask-side tint numerics (red)
  "info":                     "#4F8CFF",          // Dodger Blue — selection/info
  "accent-2":                 "#7C5CF3"           // Cornflower Purple — secondary accent
}
```

### Semantic Overlay Colors
```json
{
  "hover":      "rgba(255,255,255,0.05)",  // 5% white tint
  "selected":   "rgba(255,255,255,0.07)",  // 7% white tint
  "bull-row":   "rgba(78,192,122,0.1)",    // Green-tinted row
  "bear-row":   "rgba(216,80,62,0.1)"      // Red-tinted row
}
```

### Status Colors (`design.toml`)
```toml
[status]
ok    = [120, 180, 120, 255]    # Success green
warn  = [255, 165, 0, 255]      # Warning orange
error = [224, 85, 96, 255]      # Error red
info  = [100, 200, 255, 255]    # Info blue

[semantic]
hover_tint             = [255, 255, 255, 16]    # Hover overlay
focus_ring             = [100, 200, 255, 200]   # Focus ring (bright blue)
disabled_fg            = [140, 140, 150, 160]   # Disabled text
disabled_bg            = [40, 40, 46, 200]      # Disabled background
sentiment_positive     = [80, 200, 120, 255]    # Bull/positive
sentiment_neutral      = [180, 180, 195, 255]   # Neutral
sentiment_negative     = [224, 85, 96, 255]     # Bear/negative
order_state_recon      = [167, 139, 250, 255]   # Pending order
order_state_ctrl       = [255, 100, 100, 255]   # Controlled order
order_cancel_bg        = [232, 156, 156, 255]   # Cancel background
order_cancel_fg        = [70, 25, 25, 255]      # Cancel text
```

### Drawing / Chart Palette
```toml
[drawing.palette]
#0 = [70, 130, 255, 255]    # Blue
#1 = [80, 200, 120, 255]    # Green
#2 = [255, 160, 60, 255]    # Orange
#3 = [180, 100, 255, 255]   # Purple
```

### Chat Author Colors (8 avatar colors)
```toml
[chat_author_palette.colors]
#0 = [74, 158, 255, 255]    # Blue
#1 = [46, 204, 113, 255]    # Green
#2 = [243, 156, 18, 255]    # Orange
#3 = [155, 89, 182, 255]    # Purple
#4 = [231, 76, 60, 255]     # Red
#5 = [26, 188, 156, 255]    # Teal
#6 = [241, 196, 15, 255]    # Yellow
#7 = [52, 152, 219, 255]    # Light Blue
```

### Pane Tint Colors (Panel separation)
```toml
[color.pane_tints]
#0 = [62, 120, 180, 30]     # Blue tint, ~12% opacity
#1 = [180, 100, 255, 30]    # Purple tint
#2 = [46, 204, 113, 30]     # Green tint
#3 = [255, 191, 0, 30]      # Amber tint
```

---

## 4. Typography System

### Font Family Assignments
```
UI text:      Inter (proportional)
All numerics: JetBrains Mono (monospace) — prices, table cells, data
```

### Available Embedded Fonts
- **Inter** — Regular, Bold, SemiBold
- **JetBrains Mono** — Regular, Bold
- **IBM Plex Sans** — Regular, SemiBold
- **IBM Plex Mono** — Regular, Bold
- **Geist** — Medium
- **DM Sans** — Medium
- **Source Serif 4** — Regular, Bold
- **Plus Jakarta Sans** — Medium
- **Space Grotesk** — Medium

### Font Size Scale (`aperture.tokens.json`)
```json
{
  "2xs":        "9px",    // Eyebrow labels
  "xs":         "10px",   // Table cells, column headers
  "sm":         "12px",   // Data values, body
  "md":         "13px",   // Default UI, tab labels (primary reading size)
  "lg":         "16px",   // Large text
  "xl":         "22px",   // Card values
  "display-sm": "35px",
  "display-md": "44px",
  "display-lg": "59px",
  "display-xl": "79px",
  "display-2xl":"110px"   // Hero price / NAV
}
```

### Typography Composite Styles
```json
{
  "body": {
    "fontFamily": "Inter", "fontWeight": "Regular", "fontSize": "12", "letterSpacing": "0%"
  },
  "label": {
    "fontFamily": "Inter", "fontWeight": "Semi Bold", "fontSize": "13", "letterSpacing": "0%"
  },
  "eyebrow-upper": {
    "fontFamily": "Inter", "fontWeight": "Bold", "fontSize": "9",
    "letterSpacing": "14%", "textCase": "uppercase"
  },
  "data": {
    "fontFamily": "JetBrains Mono", "fontWeight": "Bold", "fontSize": "12", "letterSpacing": "0%"
  },
  "cell-upper": {
    "fontFamily": "JetBrains Mono", "fontWeight": "Bold", "fontSize": "10",
    "letterSpacing": "10%", "textCase": "uppercase"
  },
  "display": {
    "fontFamily": "Inter", "fontWeight": "Medium", "fontSize": "79",
    "letterSpacing": "-5.5%", "lineHeight": "83.6"
  }
}
```

### Typography by Component
| Component | Font | Size | Weight | Case | Color |
|-----------|------|------|--------|------|-------|
| Panel section header | JetBrains Mono | 10px | Bold | UPPERCASE | `dim` |
| Panel list row — primary | JetBrains Mono | 11px | Regular | normal | `text` |
| Panel list row — secondary | JetBrains Mono | 10px | Regular | normal | `color_muted(dim)` |
| Table column header | JetBrains Mono | 10px | Bold | UPPERCASE | `dim` |
| Table data cell | JetBrains Mono | 11px | Bold | normal | varies (bull/bear/text/dim) |
| Button label | Inter | 13px | Semi-bold | normal | per variant |
| Tooltip | Inter | 9px | Regular | normal | `text` |
| Eyebrow / section label | Inter | 9px | Bold | UPPERCASE | `dim` |
| Display / hero price | Inter | 59–79px | Medium | normal | `text` |
| Badge | Inter | 8px | Bold | normal | `text` |

---

## 5. Corner Radii Mapping

| Component | Token | Value | Notes |
|-----------|-------|-------|-------|
| Input field | `radius_md()` | 6px | Default form inputs |
| Button | `radius_sm()` | 4px | Standard button |
| Chip / Tag | `radius_sm()` | 4px | Small badge/label |
| Card | `radius_card` | 14px | P&L / sentiment cards |
| Panel | `radius_lg` | 20px | Side panels, footer dock |
| Pill / Navigation | `radius_full` | 99px | Nav tabs, pill buttons |
| Meridien mode | 0px | 0px | Sharp squared corners |

---

## 6. Spacing Patterns

### Sx Builder Utilities
```rust
Sx::new()
    .p_xs()     // 4px all sides (tight)
    .p_sm()     // 8px all sides
    .p_md()     // 12px all sides (default panel)
    .p_lg()     // 16px all sides (generous)
    .px_md()    // 12px left/right only
    .py_sm()    // 8px top/bottom only
    .gap_xs()   // 4px between items
    .gap_sm()   // 8px (standard button row)
    .gap_md()   // 12px (content gap)
    .gap_lg()   // 16px (panel padding)
```

### Default Component Padding
| Component | Padding | Gap |
|-----------|---------|-----|
| Button | 10px L/R, 6px T/B | n/a |
| Panel | 16px L/R, 12px T/B | 12px |
| PanelListRow | 12px L/R, 4px T/B | 8px |
| Card | 12px all | 8px |
| Tooltip | 8px all | 4px |
| Input | 10px L/R, 8px T/B | n/a |

---

## 7. Border & Stroke Patterns

### Width Tiers
```rust
stroke_hair()    // 0.3px — lightest separator
stroke_thin()    // 0.5px — subtle borders
stroke_medium()  // 0.8px — mid-weight
stroke_std()     // 1.0px — standard
stroke_bold()    // 1.5px — emphasis
stroke_thick()   // 2.0px — heavy (focus rings)
```

### By Component
| Component | Width | Alpha | Notes |
|-----------|-------|-------|-------|
| Input (idle) | `stroke_thin()` 0.5px | 50 | Semi-opaque |
| Input (focus) | `stroke_bold()` 1.5px | 200 | Bright blue focus_ring |
| Button (Secondary) | `stroke_thin()` | opaque | Outline variant |
| Panel body | `stroke_thin()` | 15 | Hairline |
| PanelSection rule | `stroke_thin()` | 100 | Section divider |
| Table header | `stroke_thin()` | 50 | Column separator |
| Separator | `stroke_thin()` | 36 | Hairline divider |
| Card | 0 | n/a | No outline, shadow only |

---

## 8. Shadow Patterns

### Presets
| Preset | Offset | Blur | Spread | Alpha | Usage |
|--------|--------|------|--------|-------|-------|
| card | (0, 2px) | 4px | 0 | 60 | Floating cards, modest lift |
| modal | (0, 8px) | 28px | 2px | 80 | Modal overlays |
| tooltip | (0, 2px) | 0 | 0 | 60 | Tooltips, very subtle |
| dropdown | (0, 8px) | 24px | 1px | 40 | Menus, popovers |

### Shadow Color
- **Dark themes:** `Color32::BLACK` (pure black)
- **Light themes:** `Color32::from_rgb(120, 120, 124)` (soft gray, prevents harsh look)

---

## 9. Layout Structure

### Main Screen Layout
```
┌─────────────────────────────────────────────────────────────────┐
│  TopNav (pill-bar: broker | tab ToolGroup | right ToolGroup)    │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  ┌──────────────────────────────────────────┐  ┌──────────────┐ │
│  │                                          │  │  Right Rail  │ │
│  │  MainContent (rounded-lg panel)          │  │              │ │
│  │  ┌────────────────────────────────────┐  │  │  Watchlist   │ │
│  │  │ HeaderStrip (ticker tags + buttons)│  │  │  DOM Ladder  │ │
│  │  ├────────────────────────────────────┤  │  │  Positions   │ │
│  │  │ Toolbar (timeframe, indicators)    │  │  │  Orders      │ │
│  │  ├────────────────────────────────────┤  │  │  Signals     │ │
│  │  │ ChartArea (GPU chart + price ladder│  │  └──────────────┘ │
│  │  │ + Sentiment/Intraday cards + P&L)  │  │                   │
│  │  └────────────────────────────────────┘  │                   │
│  └──────────────────────────────────────────┘                   │
│                                                                   │
├─────────────────────────────────────────────────────────────────┤
│  BottomDock (rounded-lg: pill ToolGroup + P&L card)             │
└─────────────────────────────────────────────────────────────────┘
```

### Right Rail Panels (SidePanelShell)
Each side panel has:
- Resizable width: Narrow 240px / Medium 300px / Wide 400px
- Close button (`Icon::X`)
- Optional tab strip (5 treatments)
- Scrollable body with `PanelSection` groups
- Resize handles

---

## 10. Component Token Reference

### Button
**Variants:** Primary, Secondary, Ghost, Danger, Link, Chrome, Chip, Tab, TextOnly, DynamicTint

**Sizes:**
- `Xs` — 18px height, tight padding
- `Sm` — 20px height, small padding
- `Md` — 24px height (standard)
- `Lg` — 32px height, loose padding

**States:** Idle, Hover, Active, Pressed, Disabled, Loading (inline spinner)

### Tag / Badge
**TagTone:** Neutral, Accent, Bull, Bear, Warn

**Variants:** Filled (solid tone bg), Outline (tone border + transparent), Dot (circular), Closable (with X)

### Tabs (5 treatments)
1. **Line** — underline accent, clean
2. **Segmented** — pill group with enclosure
3. **Filled** — active = opaque bg + inverse text
4. **Card** — active = card with shadow
5. **Pane** — horizontal pill group (footer style)

### PanelSection
```rust
PanelSection::new("SECTION TITLE")
    .count(N)                   // Numeric badge trailing
    .action("label", tone)      // Trailing action button
    .collapsible(&mut expanded) // Chevron toggle
    .title_color(color)         // Custom title tint
    .show(ui, theme, |ui, t| { ... })
```
- Title: mono 10px UPPERCASE, `dim` color by default
- Border: hairline at `alpha_line`

### PanelListRow
```rust
PanelListRow::new()
    .leading(|ui, t| ...)           // Left slot (avatar, swatch)
    .primary("label")               // Main text (mono_sm, text color)
    .secondary("label")             // Subtitle (mono_xs, dim color)
    .trailing(|ui, t| ...)          // Right slot (price, button)
    .trailing_buttons(&[...])       // Icon strip
    .selected(bool)                 // Accent stripe + bg tint
    .hoverable(bool)                // Suppress hover for display rows
    .row_tint(color, alpha)         // Buy/sell tape coloring
    .show(ui, theme)
```
- Height: 22px dense, 32px spacious
- Selected: `rgba(accent, 24%)` bg + 2px left accent stripe

### PanelSubSection
- Nested under PanelSection
- Caret toggle + uppercase title + count chip
- `.header_trailing(|ui,t| ...)` — right-aligned group controls
- Hairline rule below header (both expanded and collapsed)

### SegmentedControl
- Connected button group
- Active = solid fill + inverse text
- Enclosure options: `.none`, `.bordered`, `.frosted`, `.sharp`

---

## 11. Data Table Patterns

### Column Definition
```rust
let cols = [
    Column::new("Symbol").min_width(80).sortable(true),
    Column::new("Price").width(80).align(ColAlign::Right),
    Column::new("Change %").width(80).align(ColAlign::Right),
];

Table::new(&cols, &rows, &mut state)
    .row_height(22.0)
    .alternate_rows(true)
    .row_render(|ui, theme, row, col_idx, col_rect| { ... })
    .show(ui, theme);
```

### Header Styling
- Font: `cell-upper` (mono 10px bold, UPPERCASE, 10% letter-spacing)
- Background: L1 elevation
- Border: hairline at `alpha_line`
- Padding: `gap_xs` (4px) horizontal and vertical

### Row Styling
- Height: 20px standard, 18px compact, 24px spacious
- Padding: `gap_md` (12px) L/R, `gap_2xs` (2px) T/B
- Font: `data` (mono 12px bold) for numerics
- Hover: `color_alpha(text, alpha_subtle)` background
- Selected: `color_alpha(accent, alpha_strong)` bg + left stripe

### Cell Alignment
- Left: symbol names, text
- Right: prices, quantities, percentages (monospace)
- Center: status badges, indicators

### Color Semantics in Tables
| Cell content | Color |
|---|---|
| Positive change % | `bull` green |
| Negative change % | `bear` red |
| Neutral | `text` white |
| Volume / metadata | `dim` gray |
| Status badge | semantic tone (green/orange/red) |

---

## 12. Color Semantics Master Reference

| Meaning | Color Token | Hex | Usage |
|---------|-------------|-----|-------|
| Positive / Bull / Long / Gain / Bid | `bull` | #4EC07A | ↑ %change, profit, buy-side |
| Negative / Bear / Short / Loss / Ask | `bear` | #D8503E | ↓ %change, loss, sell-side |
| Caution / Pending / Warning | `warn` | #F5C64A | ⚠ pending order, alert |
| Brand / Primary Action | `accent` | #EF5B3B | CTAs, focus ring, ticker symbol |
| Disabled / Inactive / Muted | `dim` | #B6AD9D | Inactive tabs, placeholders |
| Primary Text | `text` | #F4ECE0 | Labels, headings, data |
| Borders / Hairlines | `border` + alpha | rgba(255,255,255,0.06) | Dividers |
| Panel Surface | `surface` | #1A1816 | Panel bg |
| App Canvas | `bg` | #000000 | Root background |

### Row Tinting
| State | Fill |
|-------|------|
| Watchlist buy tape | `rgba(bull, 10%)` — emerald tint |
| Watchlist sell tape | `rgba(bear, 10%)` — red tint |
| Hover | `rgba(text, 5%)` — white shimmer |
| Selected | `rgba(accent, 24%)` + 2px left stripe |
| Pinned | `rgba(accent, 12%)` |

### State Overlays (Applied OVER idle backgrounds)
| State | Overlay |
|-------|---------|
| Hover | `color_alpha(text, 14)` |
| Active | `color_alpha(text, 28)` |
| Selected | `color_alpha(accent, 40)` |
| Disabled | `color_alpha(text, 8)` + 50% text dimming |
| Ghost hover | `color_alpha(text, 10)` (icon buttons only) |

---

## 13. State Variants

### Button States
```rust
pub enum ButtonState {
    Idle,     // Resting
    Hover,    // Pointer hovering
    Active,   // Marked as active/selected
    Pressed,  // Mouse button down
    Disabled, // Non-interactive
    Loading,  // Async pending (inline spinner)
}
```

### Row States
| State | Background | Text |
|-------|-----------|------|
| Normal | transparent | `text` |
| Hover | `color_alpha(text, 5%)` | `text` |
| Selected | `color_alpha(accent, 24%)` + left stripe | `text` |
| Disabled | `color_alpha(disabled, 8%)` | `dim` |

### Input States
| State | Border | Background |
|-------|--------|-----------|
| Default | `border` thin | opaque |
| Hover | slightly darkened | opaque |
| Focus | bright blue `focus_ring` bold | opaque |
| Disabled | dim border | semi-transparent |
| Error | red border + red bg tint | |

### Tab States
| State | Background | Text |
|-------|-----------|------|
| Idle | transparent | `dim` |
| Hover | subtle bg tint | lightened |
| Active | solid fill (`text` color) | `bg` (inverse) |

---

## 14. Animation & Motion

### Easing Functions (`motion.rs`)
```rust
Easing::CubicInOut   // Default — smooth ease-in-out
Easing::CubicOut     // Fast-to-slow release
Easing::CubicIn      // Slow-to-fast acceleration
Easing::Elastic      // Spring-like bounce
Easing::Linear       // Constant speed
```

### Standard Durations
| Interaction | Duration | Easing |
|-------------|----------|--------|
| Hover → Pressed | 50ms | ease-out |
| Hover → Idle (release) | 150ms | ease-out |
| Collapse / Expand | 200ms | ease-in-out |
| Fade in/out | 200ms | ease-in-out |
| Modal / Panel slide in | 300ms | ease-out |

### Visual Effects
- **Hover shimmer:** semi-transparent white overlay (`alpha_faint` = 10)
- **Focus glow:** brightened state on keyboard focus
- **Pulse:** opacity oscillation for loading states
- **Chevron rotate:** PanelSection collapse toggle animation

---

## 15. Theme Catalog (15 Built-In Themes)

All defined in `src/design_system/builtin.rs`:

| # | Name | Dark | Accent | Bull | Bear | Text | Surface |
|---|------|------|--------|------|------|------|---------|
| 0 | Midnight | ✓ | #3E78B4 | #3E78B4 | #B4413A | #DCD1E6 | #1A1816 |
| 1 | Nord | ✓ | #88C0D0 | #A3BE8C | #BF616A | #DCEBE4 | #2E3440 |
| 2 | Monokai | ✓ | #E6DB74 | #A6E22E | #F92672 | #F8F8F0 | #272822 |
| 3 | Dracula | ✓ | #BD93F9 | #50FA7B | #FF5555 | #F8F8F2 | #282A36 |
| 4 | Solarized Dark | ✓ | #268BD2 | #859900 | #DC322F | #FDF6E3 | #073642 |
| 5 | Solarized Light | ✗ | #268BD2 | #859900 | #DC322F | #073642 | #EEE8D5 |
| 6 | Alto (light) | ✗ | #5B87C5 | #4DBB85 | #C55141 | #2A2A2C | #F0F0F4 |
| 7 | Mariner (dark) | ✓ | #5B87C5 | #4DBB85 | #C55141 | #DDD5CA | #1A1816 |
| 8 | Meridien (dark) | ✓ | #EF5B3B | #4EC07A | #D8503E | #F4ECE0 | #272822 |
| ... | [6 more] | — | — | — | — | — | — |
| — | Bauhaus (light) | ✗ | #1E5AB4 | #16823E | #BE323C | #1C1C20 | #F0F0F4 |

All themes are swappable at runtime via the theme registry. Light themes (Bauhaus, Peach, Ivory, Newsprint, Alto, Solarized Light) require theme-aware shadow colors.

---

## 16. Figma Token Mapping (`figma/aperture.tokens.json`, W3C DTCG format)

### Spacing Tokens
```json
{
  "2xs": "2",
  "xs": "4",
  "sm": "8",
  "md": "12",
  "lg": "16",
  "xl": "24",
  "tab-overlap": "-3"   // Negative for pill tab overlap
}
```

### Radius Tokens
```json
{
  "xs": "6",
  "sm": "12",
  "md": "16",
  "lg": "20",
  "card": "14",
  "full": "999"
}
```

### Stroke Tokens
```json
{
  "hair": "0.667",
  "thin": "1",
  "std": "1.5",
  "bold": "2"
}
```

### Shadow Effects
```json
{
  "card":  { "x": "0", "y": "10", "blur": "30", "color": "rgba(0,0,0,0.4)" },
  "modal": { "x": "0", "y": "30", "blur": "80", "color": "rgba(0,0,0,0.6)" }
}
```

---

## 17. Figma Component → Code Mapping

| Figma Component | Code File | Key Props |
|---|---|---|
| Button | `widgets/button.rs` | `intent` (Primary/Secondary/Ghost/Danger/Link/Chrome/Chip/Tab), `size` (Xs/Sm/Md/Lg), `state` |
| ToolGroup | (composition) | `enclosure` (None/Bordered/Frosted/Sharp) |
| Tag | `widgets/tag.rs` | `tone` (Accent/Bull/Bear/Warn/Neutral), `variant` (Filled/Outline), `size`, `closable`, `dot` |
| Badge | `widgets/badge.rs` | `kind` (Count/Dot/Text), `tone` |
| StatusPill | `widgets/status_pill.rs` | `tone`, `size`, `dot` |
| CountChip | `widgets/count_chip.rs` | `tone` |
| Kbd | `widgets/kbd.rs` | `size` (Xs/Sm) |
| Alert | `widgets/alert.rs` | `variant` (Info/Success/Warning/Error), `closable` |
| Toast | `widgets/toast.rs` | `severity` |
| Tabs | `widgets/tabs.rs` | `treatment` (Line/Segmented/Filled/Card/Pane), `size` (Sm/Md) |
| SegmentedControl | `widgets/segmented_control.rs` | `size`, `connected` |
| Switch | `widgets/switch.rs` | `size` (Sm/Md), `state` (On/Off/Disabled) |
| Checkbox | `widgets/checkbox.rs` | `state` (Checked/Unchecked/Disabled) |
| Input | `widgets/input.rs` | `state` (Default/Hover/Focus/Disabled/Error) |
| Select | `widgets/select.rs` | `state` (Default/Open/Disabled) |
| Slider | `widgets/slider.rs` | `state` |
| PanelCard | `widgets/panel_card.rs` | `tone`, `stripe` |
| PanelSection | `widgets/panel_section.rs` | `tone` (Default/Accent/Bull/Bear/Danger) |
| PanelListRow | `widgets/panel_list_row.rs` | `state` (Default/Hover/Selected) |
| Table | `widgets/table.rs` | (composition) |
| Tooltip | `widgets/tooltip.rs` | (auto-positioned) |
| Modal | `widgets/modal.rs` | (overlay) |
| ProgressBar | `widgets/progress.rs` | (animated wave) |

---

## 18. Hard Rules (from `CLAUDE.md`)

### 1. Never Hardcode `&THEMES[0]`
```rust
// ❌ Wrong
let theme = &crate::chart_renderer::gpu::THEMES[0];

// ✅ Correct — accept theme as parameter
pub fn my_widget(ui: &mut Ui, theme: &dyn ComponentTheme) { ... }
```

### 2. Never Hardcode Black Shadows
```rust
// ❌ Breaks on light themes (Bauhaus, Peach, Ivory, Newsprint)
Color32::from_rgba_unmultiplied(0, 0, 0, 60)

// ✅ Use theme's shadow color
let s = t.shadow_color();
Color32::from_rgba_unmultiplied(s.r(), s.g(), s.b(), 60)
```

### 3. Use Design Token Functions, Not Literals
| Literal | Token |
|---------|-------|
| `FontId::monospace(11.0)` | `mono_sm()` |
| `FontId::monospace(13.0)` | `mono_md()` |
| `vec2(4.0, 4.0)` | `vec2(gap_xs(), gap_xs())` |
| `Stroke::new(0.5, c)` | `Stroke::new(stroke_thin(), c)` |
| `Stroke::new(1.0, c)` | `Stroke::new(stroke_std(), c)` |
| `from_rgba_unmultiplied(_,_,_,60)` | `color_alpha(c, alpha_muted())` |
| `CornerRadius::same(4)` | `CornerRadius::same(radius_sm() as u8)` |

### 4. Use `ui_kit::Button` Not Raw `egui::Button`
```rust
// ❌ Wrong
egui::Button::new(label).fill(t.toolbar_bg).stroke(...)

// ✅ Correct
Button::new(label)
    .variant(Variant::Ghost)
    .size(Size::Sm)
    .show(ui, t)
```

### 5. Sacred Code — Do Not Refactor
**`src/chart/renderer/render/pane/core.rs`** — GPU-optimized chart paint pipeline. No mechanical sweeps. Changes only with benchmark coverage.

---

## 19. Summary

The apex-terminal design system is a professional two-axis theme architecture:

1. **ColorScheme** — 9 semantic tone tokens (accent, bull, bear, warn, text, dim, border, surface, bg)
2. **StyleSystem** — typography scale, spacing rhythm, corner radii, stroke widths, alpha tiers, shadows, elevation

**Key architectural strengths:**
- Figma ↔ Code token sync via W3C DTCG JSON (`figma/aperture.tokens.json`)
- 15 built-in themes (dark, light, hi-contrast) with full light-theme parity
- Per-component style traits (`ButtonStyle`, `ComponentTheme`)
- Sx utility layer (composable builder, Tailwind-like patterns)
- Stateless color axis + stateless dimension axis (swap independently)
- Zero hardcoded colors or sizes — all token-driven
- ~100 widgets all wired to the token system

**Design token sources:**
- `design.toml` — raw dimension values
- `figma/aperture.tokens.json` — W3C DTCG tokens, Figma sync
- `src/design_system/builtin.rs` — 15 full color schemes
- `src/ui_kit/style.rs` — token accessor functions used throughout code
