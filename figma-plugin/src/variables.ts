//! Stage 1 — Variables.
//!
//! Creates the `ColorScheme`, `StyleSystem`, and `CmdPalette` variable
//! collections with `Aperture` / `Meridien` modes and populates every variable
//! from `apex.tokens.json`. Variable names match the engine DTCG field names so
//! the PULL importer maps them 1:1 (`bg`, `typography/size_xs`,
//! `shadows/card/blur`, …). Idempotent: existing collections/variables are
//! reused and updated in place rather than duplicated.

import {
  TOKENS, COLLECTION, MODES, CMD_PALETTE_SLOTS, ENUM_FIELDS, enumDescription,
  parseColor, parseNumber, parseBool,
} from "./tokens";

/** Registry of created variables, keyed `"<Collection>:<name>"`, for binding. */
export const varRegistry = new Map<string, Variable>();

function regKey(collection: string, name: string): string {
  return `${collection}:${name}`;
}

/** Look up a variable created during this run by collection + name. */
export function getVar(collection: string, name: string): Variable | undefined {
  return varRegistry.get(regKey(collection, name));
}

function getOrCreateCollection(name: string): VariableCollection {
  const existing = figma.variables.getLocalVariableCollections().find((c) => c.name === name);
  return existing ?? figma.variables.createVariableCollection(name);
}

/** Ensure the collection has exactly the requested modes; return name→modeId. */
function ensureModes(collection: VariableCollection, modeNames: string[]): Record<string, string> {
  const ids: Record<string, string> = {};
  // Rename the auto-created default mode to the first requested mode.
  collection.renameMode(collection.defaultModeId, modeNames[0]);
  ids[modeNames[0]] = collection.defaultModeId;
  for (const name of modeNames.slice(1)) {
    const found = collection.modes.find((m) => m.name === name);
    ids[name] = found ? found.modeId : collection.addMode(name);
  }
  return ids;
}

function getOrCreateVariable(
  collection: VariableCollection,
  name: string,
  type: VariableResolvedDataType,
): Variable {
  const existing = collection.variableIds
    .map((id) => figma.variables.getVariableById(id))
    .find((v): v is Variable => v != null && v.name === name && v.resolvedType === type);
  const v = existing ?? figma.variables.createVariable(name, collection, type);
  varRegistry.set(regKey(collection.name, name), v);
  return v;
}

// ── Colour collection ─────────────────────────────────────────────────────────

function createColorVariables(): void {
  const collection = getOrCreateCollection(COLLECTION.color);
  const modes = ensureModes(collection, [MODES.aperture, MODES.meridien]);

  const aperture = TOKENS.color ?? {};
  const meridien = TOKENS.colorMeridien ?? {};
  const keys = Object.keys(aperture).filter((k) => !k.startsWith("_"));
  for (const key of keys) {
    const v = getOrCreateVariable(collection, key, "COLOR");
    v.setValueForMode(modes[MODES.aperture], parseColor(String(aperture[key].value)));
    const m = meridien[key] ?? aperture[key]; // Meridien lists only differences.
    v.setValueForMode(modes[MODES.meridien], parseColor(String(m.value)));
  }
}

// ── CmdPalette collection (single mode) ─────────────────────────────────────────

function createCmdPaletteVariables(): void {
  const cmd = TOKENS.cmdPalette;
  if (!cmd) return;
  const collection = getOrCreateCollection(COLLECTION.cmd);
  const modes = ensureModes(collection, ["Default"]);
  for (const slot of CMD_PALETTE_SLOTS) {
    if (!cmd[slot]) continue;
    const v = getOrCreateVariable(collection, slot, "COLOR");
    v.setValueForMode(modes["Default"], parseColor(String(cmd[slot].value)));
  }
}

// ── Style collection ────────────────────────────────────────────────────────────

/** Section base group → its Meridien-override group (if any). */
const STYLE_SECTIONS: { base: string; meridien?: string; nested?: boolean }[] = [
  { base: "typography", meridien: "typographyMeridien" },
  { base: "spacing", meridien: "spacingMeridien" },
  { base: "radii", meridien: "radiiMeridien" },
  { base: "strokes", meridien: "strokesMeridien" },
  { base: "alphas" },
  { base: "elevation" },
  { base: "density", meridien: "densityMeridien" },
  { base: "shadows", meridien: "shadowsMeridien", nested: true },
  { base: "treatments", meridien: "treatmentsMeridien" },
  { base: "chrome", meridien: "chromeMeridien" },
];

interface LeafValue {
  type: VariableResolvedDataType;
  aperture: VariableValue;
  meridien: VariableValue;
  description?: string;
}

/** Resolve a single leaf into its Figma type + per-mode values. */
function resolveLeaf(path: string, baseLeaf: any, merLeaf: any | undefined): LeafValue {
  const tokenType: string = baseLeaf.type;
  const aVal = baseLeaf.value;
  const mVal = (merLeaf ?? baseLeaf).value;

  // String-enum fields → FLOAT integer (matches the PULL importer).
  if (ENUM_FIELDS[path]) {
    const map = ENUM_FIELDS[path];
    const toInt = (v: unknown): number =>
      typeof v === "number" || /^\d+$/.test(String(v)) ? parseNumber(v) : (map[String(v)] ?? 0);
    return {
      type: "FLOAT",
      aperture: toInt(aVal),
      meridien: toInt(mVal),
      description: enumDescription(path),
    };
  }
  if (tokenType === "boolean") {
    return { type: "BOOLEAN", aperture: parseBool(aVal), meridien: parseBool(mVal) };
  }
  if (tokenType === "fontFamilies") {
    return { type: "STRING", aperture: String(aVal), meridien: String(mVal) };
  }
  if (tokenType === "string") {
    return { type: "STRING", aperture: String(aVal), meridien: String(mVal) };
  }
  // dimension / number / fontSizes / letterSpacing / spacing / borderRadius / borderWidth
  return { type: "FLOAT", aperture: parseNumber(aVal), meridien: parseNumber(mVal) };
}

function createStyleVariables(): void {
  const collection = getOrCreateCollection(COLLECTION.style);
  const modes = ensureModes(collection, [MODES.aperture, MODES.meridien]);

  for (const section of STYLE_SECTIONS) {
    const base = TOKENS[section.base];
    const mer = section.meridien ? TOKENS[section.meridien] : undefined;
    if (!base) continue;

    for (const key of Object.keys(base).filter((k) => !k.startsWith("_"))) {
      if (section.nested) {
        // e.g. shadows/card/{blur,spread,...}
        const role = base[key];
        const merRole = mer ? mer[key] : undefined;
        for (const sub of Object.keys(role).filter((k) => !k.startsWith("_"))) {
          const name = `${section.base}/${key}/${sub}`;
          const leaf = resolveLeaf(name, role[sub], merRole ? merRole[sub] : undefined);
          writeLeaf(collection, modes, name, leaf);
        }
      } else {
        const name = `${section.base}/${key}`;
        const leaf = resolveLeaf(name, base[key], mer ? mer[key] : undefined);
        writeLeaf(collection, modes, name, leaf);
      }
    }
  }
}

function writeLeaf(
  collection: VariableCollection,
  modes: Record<string, string>,
  name: string,
  leaf: LeafValue,
): void {
  const v = getOrCreateVariable(collection, name, leaf.type);
  if (leaf.description) v.description = leaf.description;
  v.setValueForMode(modes[MODES.aperture], leaf.aperture);
  v.setValueForMode(modes[MODES.meridien], leaf.meridien);
}

/** Create all variable collections + variables. Returns a one-line summary. */
export function createVariables(): string {
  varRegistry.clear();
  createColorVariables();
  createCmdPaletteVariables();
  createStyleVariables();
  return `variables: ${varRegistry.size} created/updated across ColorScheme + StyleSystem + CmdPalette`;
}
