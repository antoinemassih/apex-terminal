//! Stage 5 — Export the live Figma file → `FigmaThemeExport` JSON.
//!
//! Closes the loop: after a designer edits the scaffolded variables/components,
//! this reads them back into the exact envelope the Rust PULL transformer
//! (`design_system::import`) consumes, so the app's *Import from Figma* button
//! can turn the finished design into a `.apextheme`.
//!
//! The shape must match `design_system::import::model::FigmaThemeExport`:
//!   - `cmd_palette` colours live in the `CmdPalette` collection here but are
//!     re-emitted as `ColorScheme` variables named `cmd_palette/<slot>` (where
//!     the importer expects them), mirrored across every colour mode.
//!   - the four string-enum style fields are FLOAT integers (the importer maps
//!     them back to their canonical strings).

import { COLLECTION, MODES } from "./tokens";

interface RGBA { r: number; g: number; b: number; a: number }
type FigmaValueJson = RGBA | number | boolean | string;

interface VariableJson {
  collection: string;
  name: string;
  type: VariableResolvedDataType;
  valuesByMode: { [mode: string]: FigmaValueJson };
  description?: string;
}

interface ModeMetaJson { id: string; name: string; is_dark: boolean }
interface CollectionJson {
  name: string;
  defaultMode: string;
  modes: string[];
  meta: { [mode: string]: ModeMetaJson };
}

interface FigmaThemeExportJson {
  schema: number;
  generator: string;
  colorCollection: CollectionJson;
  styleCollection: CollectionJson;
  variables: VariableJson[];
  components: { [recipeKey: string]: unknown };
}

/** Tone names the engine `ColorSpec`/`ToneRef` understands. */
const TONE_NAMES = ["accent", "bull", "bear", "warn", "text", "dim", "border", "surface", "bg"];

function isRGBA(v: VariableValue): v is RGBA {
  return typeof v === "object" && v !== null && "r" in v && "g" in v && "b" in v;
}

/** Read a variable's concrete value for a mode. Aliases are skipped (returns null). */
function readValue(variable: Variable, modeId: string): FigmaValueJson | null {
  const raw = variable.valuesByMode[modeId];
  if (raw === undefined) return null;
  if (isRGBA(raw)) return { r: raw.r, g: raw.g, b: raw.b, a: "a" in raw ? raw.a : 1 };
  if (typeof raw === "number" || typeof raw === "boolean" || typeof raw === "string") return raw;
  return null; // VariableAlias — not supported in the export contract.
}

function modeNameById(collection: VariableCollection): Map<string, string> {
  const m = new Map<string, string>();
  for (const mode of collection.modes) m.set(mode.modeId, mode.name);
  return m;
}

/** Build the per-mode meta. `is_dark` is inferred from the `bg` colour luminance. */
function buildMeta(collection: VariableCollection, bgByMode: Map<string, RGBA>): { [mode: string]: ModeMetaJson } {
  const meta: { [mode: string]: ModeMetaJson } = {};
  for (const mode of collection.modes) {
    const bg = bgByMode.get(mode.name);
    const isDark = bg ? bg.r * 0.299 + bg.g * 0.587 + bg.b * 0.114 < 0.5 : true;
    meta[mode.name] = { id: mode.name.toLowerCase(), name: mode.name, is_dark: isDark };
  }
  return meta;
}

function collectionInfo(collection: VariableCollection, meta: { [mode: string]: ModeMetaJson }): CollectionJson {
  const defaultName = modeNameById(collection).get(collection.defaultModeId) ?? collection.modes[0]?.name ?? "";
  return {
    name: collection.name,
    defaultMode: defaultName,
    modes: collection.modes.map((m) => m.name),
    meta,
  };
}

function variablesOf(collection: VariableCollection): Variable[] {
  return collection.variableIds
    .map((id) => figma.variables.getVariableById(id))
    .filter((v): v is Variable => v != null);
}

// ── Component → RecipeSpec (best-effort) ────────────────────────────────────────

function radiusTierFromName(name: string): string | null {
  const leaf = name.split("/")[1];
  const tiers = ["none", "xs", "sm", "md", "lg", "pill"];
  return tiers.includes(leaf) ? leaf : null;
}

/** Reconstruct a minimal RecipeSpec from a variant component's bound fill + corner. */
function recipeFromComponent(comp: ComponentNode): unknown | null {
  const base: { [k: string]: unknown } = {};

  // Fill → tone ref (only when bound to a recognised ColorScheme tone variable).
  const fills = comp.fills;
  if (Array.isArray(fills) && fills.length > 0 && fills[0].type === "SOLID") {
    const bound = fills[0].boundVariables?.color;
    if (bound) {
      const v = figma.variables.getVariableById(bound.id);
      if (v && TONE_NAMES.includes(v.name)) {
        base.fill = { kind: "tone", tone: v.name };
      }
    }
  }

  // Corner → radius tier (bound to a radii/<tier> variable).
  const cornerBind = comp.boundVariables?.topLeftRadius;
  if (cornerBind) {
    const v = figma.variables.getVariableById(cornerBind.id);
    const tier = v ? radiusTierFromName(v.name) : null;
    if (tier) base.radius = tier;
  }

  return Object.keys(base).length > 0 ? base : null;
}

function collectComponents(): { [recipeKey: string]: unknown } {
  const out: { [recipeKey: string]: unknown } = {};
  for (const page of figma.root.children) {
    if (page.type !== "PAGE") continue;
    for (const node of page.children) {
      if (node.type !== "COMPONENT_SET") continue;
      for (const child of node.children) {
        if (child.type !== "COMPONENT") continue;
        const key = child.getPluginData("recipeKey");
        if (!key) continue;
        // First variant wins per recipe key (the representative styled state).
        if (out[key]) continue;
        const spec = recipeFromComponent(child);
        if (spec) out[key] = spec;
      }
    }
  }
  return out;
}

// ── Public entry ────────────────────────────────────────────────────────────────

/** Read the current file's variables + components into a FigmaThemeExport JSON string. */
export function buildFigmaThemeExportJson(): string {
  const collections = figma.variables.getLocalVariableCollections();
  const colorCol = collections.find((c) => c.name === COLLECTION.color);
  const styleCol = collections.find((c) => c.name === COLLECTION.style);
  const cmdCol = collections.find((c) => c.name === COLLECTION.cmd);

  if (!colorCol || !styleCol) {
    throw new Error("Missing ColorScheme/StyleSystem collections — run Generate first.");
  }

  const variables: VariableJson[] = [];
  const colorModeName = modeNameById(colorCol);
  const styleModeName = modeNameById(styleCol);

  // bg-per-mode for is_dark inference.
  const bgByMode = new Map<string, RGBA>();

  // Colour collection variables.
  for (const v of variablesOf(colorCol)) {
    const valuesByMode: { [mode: string]: FigmaValueJson } = {};
    for (const modeId of Object.keys(v.valuesByMode)) {
      const name = colorModeName.get(modeId);
      if (!name) continue;
      const val = readValue(v, modeId);
      if (val === null) continue;
      valuesByMode[name] = val;
      if (v.name === "bg" && isRGBA(v.valuesByMode[modeId])) bgByMode.set(name, val as RGBA);
    }
    variables.push({ collection: COLLECTION.color, name: v.name, type: v.resolvedType, valuesByMode });
  }

  // CmdPalette → re-emit as ColorScheme `cmd_palette/<slot>` across all colour modes.
  if (cmdCol) {
    const cmdModes = cmdCol.modes;
    for (const v of variablesOf(cmdCol)) {
      const firstVal = cmdModes.length > 0 ? readValue(v, cmdModes[0].modeId) : null;
      if (firstVal === null) continue;
      const valuesByMode: { [mode: string]: FigmaValueJson } = {};
      for (const mode of colorCol.modes) valuesByMode[mode.name] = firstVal;
      variables.push({
        collection: COLLECTION.color,
        name: `cmd_palette/${v.name}`,
        type: "COLOR",
        valuesByMode,
      });
    }
  }

  // Style collection variables.
  for (const v of variablesOf(styleCol)) {
    const valuesByMode: { [mode: string]: FigmaValueJson } = {};
    for (const modeId of Object.keys(v.valuesByMode)) {
      const name = styleModeName.get(modeId);
      if (!name) continue;
      const val = readValue(v, modeId);
      if (val === null) continue;
      valuesByMode[name] = val;
    }
    const out: VariableJson = { collection: COLLECTION.style, name: v.name, type: v.resolvedType, valuesByMode };
    if (v.description) out.description = v.description;
    variables.push(out);
  }

  const colorMeta = buildMeta(colorCol, bgByMode);
  const styleMeta = buildMeta(styleCol, bgByMode);

  const exportObj: FigmaThemeExportJson = {
    schema: 1,
    generator: "apex-figma-sync plugin export",
    colorCollection: collectionInfo(colorCol, colorMeta),
    styleCollection: collectionInfo(styleCol, styleMeta),
    variables,
    components: collectComponents(),
  };

  return JSON.stringify(exportObj, null, 2);
}

void MODES; // keep the import referenced for future per-mode logic.
