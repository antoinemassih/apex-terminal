//! Token-shape types + parsing helpers shared by the generator stages.
//!
//! `apex.tokens.json` is a Tokens-Studio-style document: top-level groups whose
//! leaves are `{ value, type, description }`. Colours are hex or `rgba(...)`;
//! dimensions/numbers are stringified numbers; booleans/enums are strings.

import APEX_TOKENS from "../apex.tokens.json";

export type RGBA = { r: number; g: number; b: number; a: number };

/** A single Tokens-Studio leaf. */
export interface TokenLeaf {
  value: unknown;
  type: string;
  description?: string;
}

/** The parsed token document (loosely typed — we navigate by group name). */
export type TokenDoc = { [group: string]: any };

export const TOKENS: TokenDoc = APEX_TOKENS as TokenDoc;

/** Figma variable collection names — must match the engine import contract. */
export const COLLECTION = {
  color: "ColorScheme",
  style: "StyleSystem",
  cmd: "CmdPalette",
} as const;

export const MODES = { aperture: "Aperture", meridien: "Meridien" } as const;

/** The 11 cmd_palette slots, in engine array order. */
export const CMD_PALETTE_SLOTS = [
  "symbol", "widget", "overlay", "theme", "timeframe", "layout", "play",
  "alert", "ai", "dynamic", "calc",
] as const;

/**
 * The four StyleSystem fields the engine treats as DTCG strings but that Figma
 * (and the PULL importer) represent as FLOAT integers. Keyed by `group/leaf`.
 * Mapping is identical to `design_system::import::model`.
 */
export const ENUM_FIELDS: { [path: string]: { [name: string]: number } } = {
  "treatments/focus_ring": { none: 0, outline: 1, glow: 2 },
  "treatments/surface_bevel": { none: 0, raised: 1, inset: 2 },
  "chrome/pane_active_indicator": { none: 0, top_stripe: 1, header_fill: 2, both: 3 },
  "chrome/button_group": { none: 0, bordered: 1, frosted: 2, sharp: 3 },
};

export function enumDescription(path: string): string {
  const map = ENUM_FIELDS[path];
  if (!map) return "";
  const parts = Object.keys(map).map((k) => `${map[k]}=${k}`);
  return `enum ${path.split("/")[1]}: ${parts.join(" ")}`;
}

/** Clamp a 0..255 byte and normalise to 0..1. */
function byte(n: number): number {
  return Math.min(255, Math.max(0, n)) / 255;
}

/** Parse a colour token value (`#rgb`, `#rrggbb`, `#rrggbbaa`, or `rgba(...)`). */
export function parseColor(value: string): RGBA {
  const v = value.trim();
  if (v.startsWith("rgba(") || v.startsWith("rgb(")) {
    const inner = v.slice(v.indexOf("(") + 1, v.indexOf(")"));
    const parts = inner.split(",").map((s) => parseFloat(s.trim()));
    return {
      r: byte(parts[0] ?? 0),
      g: byte(parts[1] ?? 0),
      b: byte(parts[2] ?? 0),
      a: parts.length >= 4 ? Math.min(1, Math.max(0, parts[3])) : 1,
    };
  }
  let h = v.replace(/^#/, "");
  if (h.length === 3) {
    h = h.split("").map((c) => c + c).join("");
  }
  const hb = (i: number) => parseInt(h.slice(i, i + 2), 16) / 255;
  if (h.length === 6) return { r: hb(0), g: hb(2), b: hb(4), a: 1 };
  if (h.length === 8) return { r: hb(0), g: hb(2), b: hb(4), a: hb(6) };
  return { r: 0, g: 0, b: 0, a: 1 };
}

/** Parse a numeric token value (stringified number). */
export function parseNumber(value: unknown): number {
  if (typeof value === "number") return value;
  const n = parseFloat(String(value));
  return Number.isFinite(n) ? n : 0;
}

/** Parse a boolean token value (`"true"`/`"false"` or bool). */
export function parseBool(value: unknown): boolean {
  return value === true || value === "true";
}
