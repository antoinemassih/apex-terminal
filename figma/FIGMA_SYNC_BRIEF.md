# Brief: Bidirectional Figma ⇄ apex-terminal theme sync

**Goal:** an agent builds a tool that (A) **scaffolds a Figma file** from this repo's design system — pushing the design tokens, a component library, and a per-page/per-element layout to start from — and (B) **imports a finished, styled Figma design back** into the app as a `.apextheme` theme pack. The result is a real design loop: scaffold in Figma → designer styles it → pull it back → install/switch live.

---

## 0. READ THIS FIRST — the hard API constraint (do not fight it)

Figma can only be **written from inside Figma** (the Plugin API). From outside (REST / the Figma MCP) you can **read** a file but **cannot create** variables, components, or frames. Therefore the tool is **two halves with different mechanisms**:

| Direction | What it does | Mechanism (the ONLY one that works) |
|---|---|---|
| **PUSH** (engine → Figma) | create variables + component library + layout frames | a **Figma plugin** (TypeScript, Figma Plugin API) the user runs once in the Figma desktop app |
| **PULL** (Figma → engine) | read the finished design, emit a `.apextheme` | a **transformer** using Figma REST API / the Figma MCP `get_figma_data` (read-only) |

Do NOT attempt to push variables/components/frames via REST or MCP — it is not supported. The push side MUST be a plugin. (Tokens-only push can alternatively use the Tokens Studio plugin importing `apex.tokens.json`, but components + layout require our own plugin, so build the plugin to do all three.)

---

## 1. Source of truth (already in the repo — do not re-derive)

- `figma/apex.tokens.json` — **339 tokens**, Tokens Studio format, names matching the engine. Palette (incl. widened info/success/warning/danger + cmd_palette), all StyleSystem sections (typography/spacing/radii/strokes/alphas/elevation/density/shadows/treatments/**chrome**), Aperture + Meridien as the two **modes** (color mode = a `ColorScheme`; dimension mode = a `StyleSystem`). This is the PUSH token source AND the PULL name map.
- `figma/component-inventory.md` — **55 components** × variants × states × **token bindings** × **recipe keys**. This is the PUSH component spec AND the PULL variant→recipe map. Note the recipe-adoption status table (only some keys restyle live today).
- `figma/README.md` — the Figma⇄engine contract + the two-axis modes workflow.
- `docs/theme-authoring/schema/*.schema.json` — exact engine JSON shapes (`manifest`, `colorscheme`, `stylesystem`, `recipes`). The PULL output must validate against these.
- `docs/theme-authoring/token-reference.md` — every token, type, default, what it affects.
- Engine entry points (Rust): `design_system::loader` (`ColorScheme/StyleSystem::from_dtcg`), `export` (`to_dtcg`), `design_system::recipes::RecipeSet`, `design_system::theme_pack::{ThemePack, bundle (write/read .apextheme), validate, PackRegistry}`. The PULL side emits a `ThemePack` and writes a bundle through these.
- **Credentials:** Figma PAT + file key are stored in the session memory file `figma_credentials.md` (read it; do NOT hardcode or commit secrets).
- **Read tool:** the Figma MCP (`mcp__figma__get_figma_data`, `mcp__figma__download_figma_images`) — load schemas via ToolSearch before calling.

---

## 2. PHASE A — PUSH: the Figma scaffolding plugin

**Deliverable:** `figma-plugin/` — a Figma plugin (manifest.json + TypeScript via the Plugin API) that, when run in a Figma file, generates a complete starting point. Bundle copies of `apex.tokens.json` + a machine-readable form of `component-inventory.md` into the plugin (the plugin can't read the repo at runtime).

1. **Variables**: create variable collections (color, radius, spacing, stroke, fontSize, alpha, chrome) from `apex.tokens.json`. Two **modes** per collection (Aperture, Meridien) — color modes = ColorSchemes, dimension modes = StyleSystems. Create the typography text styles + shadow effect styles.
2. **Component library**: for each component in `component-inventory.md`, create a Figma component set with the documented **variants × sizes × states**, binding every fill/corner/padding/text to the matching **variable** (never raw values) per the token-binding tables. Name each variant so it maps back to its **recipe key** (e.g. a Button "Primary" variant ↔ `button.primary`; tab active ↔ `tab.line.active`). Use the inventory's per-component "Figma variant matrix" notes.
3. **Per-page / per-element layout**: generate starter **frames** for the app's layout regions (top_nav, toolnav, account_strip, workspace rail, bottom_dock, right_rail, central) — see the inventory's Frames→layout-regions section — as a page (or pages) the designer fleshes out. Lay out representative elements per region using the component instances. (This is the seed the `ShellProfile` concept will eventually consume — ShellProfile is still a placeholder, so keep frame names stable/semantic.)
4. The plugin output must be **re-runnable** (idempotent-ish): re-running updates variables/components in place rather than duplicating, where feasible.

Constraint: motion/easing and custom-graphics widgets (Sparkline, RiskRewardBar) don't transcribe — colors only; skip their internal drawing.

---

## 3. PHASE B — PULL: finished design → `.apextheme`

**Deliverable:** a transformer (prefer a Rust importer under `design_system/import/` reusing `from_dtcg`/`RecipeSet`/`theme_pack`; a Node pre-step is OK if it only normalizes Figma JSON). Reads a Figma file and emits a validated pack.

1. **Read** the file via Figma REST / MCP `get_figma_data` (file key + PAT from `figma_credentials.md`): pull variables (per mode), text/effect styles, and component-set variant styles.
2. **Map → engine** using the name-matched contract:
   - variables (color mode) → `ColorScheme` fields; dimension modes → `StyleSystem` fields. Names already match `apex.tokens.json`/the schemas → mostly 1:1; put any non-matching names in a small editable `figma-mapping.json` (figma-name → engine-key), don't hardcode exceptions.
   - component variant styles → `RecipeSpec` entries keyed by the recipe key encoded in the variant name (use `component-inventory.md` as the map). Colors as **semantic tone refs**, not raw hex.
3. **Emit** a `ThemePack` (manifest + ColorScheme + StyleSystem + RecipeSet + asset refs) and **write a `.apextheme`** via `theme_pack::bundle`.
4. **Validate** with `theme_pack::validate` (S9 — contrast/structural/sandbox); surface the report; refuse on errors.
5. Output is then installable via Settings ▸ Themes (`PackRegistry::install`) or loadable in Theme Studio.

Format notes from the inventory generation (handle these): Tokens Studio has no array/integer/enum types — `cmd_palette` arrives as 11 named tokens (reassemble to the array), u8 alphas as numbers, and enums (`pane_active_indicator`, `button_treatment`) as integer values with the string form in the variable description (read the description for the canonical string).

---

## 4. Acceptance — the round-trip proof (this is the real test)

Build a **round-trip test** as the definition of done:
1. Take the built-in **Aperture** theme (already expressed in `apex.tokens.json`).
2. PUSH it to a scratch Figma file via the plugin (or simulate the plugin's output JSON if a live Figma run isn't possible in CI).
3. PULL it back through the transformer → `.apextheme` → load `ThemePack`.
4. Assert the recovered ColorScheme + StyleSystem **match the Aperture builtin** (field-by-field, like the existing `data_driven_proof` / DTCG round-trip tests), and the pack **passes `validate`**.

If a live Figma round-trip can't run unattended, split: (a) unit-test the transformer against a saved Figma-export JSON fixture; (b) document the manual plugin-run steps. Be explicit about which parts are automated vs. need a human in Figma.

---

## 5. Execution rules (for the agent)

- Work in an isolated git worktree off `sidebar-nav`. Branch `figma/sync-tool`.
- New code only: `figma-plugin/` (TS) + `design_system/import/` (Rust) + a `figma-mapping.json`. Do NOT touch `chart/renderer/render/pane/core.rs`; do NOT add fields to `Watchlist`/`Chart`.
- Secrets: read the Figma PAT/file key from the session memory `figma_credentials.md`; never commit them (use env/local config; add to `.gitignore` if needed).
- Verify the Rust side with `cargo check`/tests; verify the plugin builds (tsc) — note you cannot run it headless, so rely on the fixture-based transformer test for the automated gate and document the manual Figma steps.
- Report honestly: what's automated, what needs a human Figma run, and the round-trip test result. Don't overstate — a deferred live-Figma step with a passing fixture test is the expected outcome.

---

## 6. Suggested build order
1. PULL transformer + `figma-mapping.json` + fixture round-trip test (highest value, fully automatable, reuses existing engine).
2. PUSH plugin (tokens → variables → components → layout frames).
3. Wire both: "Export to Figma" (emit plugin payload) and "Import from Figma" (run transformer) buttons in Theme Studio, closing the loop with the in-app tool.
