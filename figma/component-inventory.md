# Figma → apex-terminal: Component & Frame naming contract

Name Figma **Components** after `ui_kit` widgets and give them **Variant properties**
whose values match the code enums below. Name **Frames** after layout regions.
When this holds, a screen reads as an *assembly of existing widgets*, not a design to interpret.

- **Component** = a reusable widget that already exists in `src-tauri/src/ui_kit/widgets/`. I reuse it.
- **Frame** = a layout region that *composes* components. I implement it as a layout function.
- Variant property values are **lowercase**, matching the Rust enum variant names.

---

## Components (build these as Figma Components with the listed Variant props)

| Figma component | Variant properties (→ code) |
|---|---|
| **Button** | `intent`: primary · secondary · ghost · danger · link · chrome · chip · tab · neutral-action  •  `size`: xs · sm · md · lg  •  `state`: default · hover · active · disabled  •  `loading`: bool |
| **ToolGroup** | `enclosure`: none · bordered · frosted · sharp  (→ `GroupEnclosure`). Holds Button(intent=tab). Build the **active-tab swatch** (fill `text`/Merino, label `surface-enclosure`/Zeus) + `spacing.tab-overlap` here. |
| **Tag** | `tone`: accent · bull · bear · warn · neutral  •  `variant`: filled · outline  •  `size`: xs · sm  •  `closable`: bool · `dot`: bool |
| **Badge** | `kind`: count · dot · text  •  `tone`: accent · bull · bear · warn · neutral |
| **StatusPill** | `tone`: bull · bear · warn · neutral  •  `size`: xs · sm  •  `dot`: bool |
| **CountChip** | `tone`: accent · bull · bear · warn · neutral |
| **Kbd** | `size`: xs · sm |
| **Alert** | `variant`: info · success · warning · error  •  `closable`: bool |
| **Toast** | `severity`: info · success · warning · error |
| **Tabs** | `treatment`: line · segmented · filled · card · pane  •  `size`: sm · md |
| **SegmentedControl** | `size`: xs · sm · md  •  `connected`: bool |
| **ToggleGroup** | `size`: xs · sm |
| **Switch** | `size`: sm · md  •  `state`: on · off · disabled |
| **Checkbox** / **Radio** | `state`: checked · unchecked · disabled |
| **Input** / **TextArea** / **SearchInput** | `state`: default · hover · focus · disabled · error |
| **Select** | `state`: default · open · disabled |
| **NumberStepper** | `size`: sm · md |
| **Slider** / **RangeSlider** | `state`: default · active · disabled |
| **MetricRow** | `tone`: default · muted · accent · bull · bear · warn |
| **PanelCard** | `tone`: default · accent · bull · bear  •  `stripe`: bool |
| **PanelSection** | `tone`: default · accent · bull · bear · danger |
| **PanelListRow** | `state`: default · hover · selected |
| **SelectableRow** | `state`: default · hover · selected · disabled |
| **Table** / **TableHeader** | (composition; cells use `typography.cell-upper` / `data`) |
| **Tabs · Sidebar · Tree · Breadcrumb · Separator** | layout components |
| **Tooltip · HoverCard · Popover · Modal · Sheet · ConfirmDialog · ContextMenu · MenuItem** | overlays — Modal/Sheet use `shadow.modal` |
| **Progress · Spinner · Skeleton** | feedback |
| **Sparkline · RiskRewardBar** | custom-graphics (drawn, not Sx-composed — match colors only) |

### Token bindings every component must use (no raw values)
- Fill → a `color.*` variable. Border → `color.border` (+ width `stroke.*`).
- Corners → `radius.*` (`radius.full` for pills). Padding/gap → `spacing.*`.
- Text → a `typography.*` style. Shadow → `shadow.*`.

---

## Frames (layout regions — name these, don't make them components)

```
TradingScreen            ← top-level 1920×1202 screen
  TopNav                 ← pill-bar: broker pill | tab ToolGroup | right ToolGroup
  MainContent            ← rounded-lg Cod-Gray panel
    HeaderStrip          ← ticker Tags + action Buttons
    Toolbar              ← timeframe ToolGroup + indicator Buttons + OHLC
    ChartArea            ← chart + display-price headline + Sentiment/Intraday PanelCards + price ladder
  BottomDock  (Footer)   ← rounded-lg dock: footer pill ToolGroup + P&L PanelCard
  RightRail              ← stacked side panels:
    WatchlistPanel · DomPanel · PositionsPanel · OrdersPanel · SignalsPanel
```

Region → code: `TopNav` = `components/toolbar/top_nav.rs`, `MainContent`/chart = `chrome/pane.rs`,
`BottomDock` = `panels/bottom_dock.rs`, `RightRail` = `panels/right_rail.rs`.

---

## The golden rule
If a Figma layer's **fill is a named color variable**, its **corner is a radius variable**, and it's an
instance of a **named component with a known variant**, then I transcribe it 1:1. Anything using a raw
hex / raw px / unnamed group is something I have to *guess* — minimise those.
