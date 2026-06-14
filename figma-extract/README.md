# apex-figma-extract

Standalone CLI that reads an **existing** Figma file via the REST API and extracts
its palette, type scale, radii, spacing, and components into the apex-terminal
`FigmaThemeExport` envelope — ready for the app's **Import from Figma** button or
the Rust importer (`design_system::import`).

This is the "bring an arbitrary design into our system" tool. (To go the other
way — scaffold a Figma file *from* our design system — use `figma-plugin/`.)

## Why a standalone CLI (not a plugin)

Figma is read-only from outside but the document/styles REST endpoints work on
**any** plan with just a PAT — so extraction needs no plugin and no opening the
file in the desktop app. (Writing variables/components still requires the plugin;
that's the other tool.)

## Use

```bash
cd figma-extract

# By PAT + file key:
FIGMA_PAT=figd_xxx node src/extract.mjs --file <FILE_KEY>

# Or parse both from the session credentials file:
node src/extract.mjs --creds /path/to/figma_credentials.md

# Offline: re-run against a cached `GET /v1/files/:key` response:
node src/extract.mjs --input fixtures/sample-figma-file.json

# From a Figma MCP dump (Framelink get_figma_data YAML) — no PAT needed when the
# MCP connection is authenticated. Save the get_figma_data output to a file, then:
node src/extract.mjs --mcp path/to/get_figma_data-dump.txt
```

The `--mcp` mode is dependency-free: it line-parses the dump's `globalVars.styles`
table and scans node references, so it works even when the REST PAT is expired or
the npm registry is unreachable.

Outputs (override with `--out` / `--mapping`):
- `extracted.figma.json` — the `FigmaThemeExport`. Load it via **Theme Studio ▸
  Import from Figma** (it runs `figma_export_to_pack`, validates, and produces a
  `.apextheme`).
- `figma-mapping.suggested.json` — every guess the extractor made (style-name →
  engine role, component → recipe key). **Review and correct it** — extraction is
  heuristic.

## How the mapping works (two ladders, best-first)

1. **By name** — color styles / variables named like an engine role
   (`Accent/Brand`, `Surface/Panel`, `Positive/Bull`…) are trusted. The richest
   signal; name your Figma styles well for a clean import.
2. **By appearance** — for roles with no matching name, frequent colours are
   classified by luminance/saturation/hue (darkest→`bg`, most-saturated→`accent`,
   green→`bull`, red→`bear`, amber→`warn`, …).

Type/radii/spacing come from the 5-number spread of the file's text sizes, corner
radii, and auto-layout gaps; the dominant text font becomes `family_ui`.
Component(-set) names are matched to recipe keys (`Button/Primary`→`button.primary`,
`Tag`→`tag`, …) with the variant's fill mapped to the nearest engine tone.

## Honest limits

- Roles missing from a file fall back to `builtin_dark()` in the engine — partial
  extraction is fine, not an error.
- A file built with **raw fills and no styles** yields weaker results (appearance
  heuristic only); the CLI warns when it resolves fewer than 4 colours.
- Component → `RecipeSpec` extraction captures fill tone (and corner where
  available), not full per-state styling.
- The Figma **variables** REST endpoint is Enterprise-only, so extraction reads
  **styles + node fills**, not Variables. Files authored by our own plugin (which
  uses Variables) are better round-tripped with the plugin's *Export → JSON*.

## Verified

`node src/extract.mjs --input fixtures/sample-figma-file.json` resolves 8 palette
roles, 16 style tokens (type/radii/spacing), and 2 components — and the emitted
envelope matches the shape the Rust importer's `plugin_shaped_export_imports`
test asserts is importable + installable.
