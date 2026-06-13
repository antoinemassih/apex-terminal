# Apex Terminal — Theme Scaffold (Figma plugin · PUSH half)

This is the **write-into-Figma** half of the bidirectional Figma ⇄ apex-terminal
theme sync tool. Figma can only be *written from inside Figma* (the Plugin API),
so creating variables, a component library, and layout frames must happen here —
the REST API / MCP can only **read** a file. The **read-back / PULL** half is the
Rust transformer at `src-tauri/src/design_system/import/`.

## What it generates

Driven entirely by the bundled `apex.tokens.json` + `component-inventory.json`
(copied from `figma/` — the plugin can't read the repo at runtime):

1. **Variables** — `ColorScheme`, `StyleSystem`, and `CmdPalette` variable
   collections. `ColorScheme` and `StyleSystem` each get **Aperture** and
   **Meridien** modes (a color mode = a `ColorScheme`; a dimension mode = a
   `StyleSystem`). Variable names match the engine DTCG field names exactly
   (`bg`, `typography/size_xs`, `shadows/card/blur`, …) so the PULL transformer
   maps them 1:1. The four string-enum style fields (`focus_ring`,
   `surface_bevel`, `pane_active_indicator`, `button_group`) are stored as
   `FLOAT` integers with the canonical string set in the variable description,
   matching the importer.
2. **Text + effect styles** — the `textStyle` / `effectStyle` composites.
3. **Component library** — a component set per entry in
   `component-inventory.json`, with variants named so each maps back to its
   **recipe key** (e.g. Button `Primary` → `button.primary`, Tabs `Line` active
   → `tab.line.active`). Fills / strokes / corners **bind to the variables** —
   never raw values. Coverage seeds the recipe-bearing + core components
   (Button, Tag, StatusPill, Badge, Tabs, PanelListRow, PanelSection, Input,
   Alert, Toast); extend `component-inventory.json` to grow it.
4. **Layout frames** — a `TradingScreen` with the shell regions (`top_nav`,
   `toolnav`, `account_strip`, `workspace_rail`, `central`, `right_rail`,
   `bottom_dock`) as named auto-layout frames. Region names are kept stable for
   the future `ShellProfile`.

Re-running is idempotent-ish: variables/styles are reused and updated in place;
component sets and the layout frame are rebuilt rather than duplicated.

> Not transcribed (by design): motion/easing and the internal drawing of
> custom-graphics widgets (Sparkline, RiskRewardBar) — colors only.

## Build

```bash
cd figma-plugin
npm install
npm run typecheck   # tsc --noEmit  (the compile gate)
npm run build       # esbuild → dist/code.js  (the runtime bundle manifest.json points to)
```

## Run it in Figma (manual — cannot be done headless)

1. Open the **Figma desktop app** (variables/component creation needs the
   desktop app, not the browser).
2. Open or create a file to scaffold into.
3. **Menu → Plugins → Development → Import plugin from manifest…** and pick
   `figma-plugin/manifest.json`.
4. Run **Plugins → Development → Apex Terminal — Theme Scaffold**.
5. In the panel, click **Generate everything** (or a single stage). Watch the
   log; you'll get a "scaffold complete" notification.
6. The designer styles the result. To round-trip it back into the app, use the
   PULL transformer (see below).

## Round-trip back into the app (PULL) — the full live loop

1. Run the plugin and **Generate everything** (above).
2. The designer restyles the variables / component variants in Figma.
3. In the plugin panel, click **⤓ Export this file → JSON** — the plugin reads
   the live variables (+ component variant fills/corners) and downloads
   `apex-theme.figma.json` in the `FigmaThemeExport` shape.
4. In the app's **Theme Studio**, click **Import from Figma** and pick that
   JSON. The PULL transformer (`design_system::import::figma_export_to_pack`)
   turns it into a `ThemePack`, validates it, and loads it for editing /
   *Export .apextheme* / *Apply to App*.

The importer consumes the same `FigmaThemeExport` envelope this plugin produces,
so push and pull share one contract. The automated acceptance proof is the
fixture round-trip test:

```bash
cd src-tauri
cargo test --lib design_system::import
```

which pushes the built-in **Aperture** theme through the envelope and asserts the
recovered `ColorScheme` + `StyleSystem` match the builtin field-by-field and the
pack passes `theme_pack::validate`.
