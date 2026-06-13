//! Stage 2 — Text styles + Effect styles.
//!
//! `apex.tokens.json` carries composite `textStyle` and `effectStyle` groups
//! (Figma text/effect styles are not variables). This stage publishes them so
//! component layers can bind text to a named style and shadows to a named
//! effect. Idempotent by style name.

import { TOKENS, parseColor } from "./tokens";

function parseLetterSpacing(v: string): LetterSpacing {
  const s = String(v).trim();
  if (s.endsWith("%")) return { value: parseFloat(s), unit: "PERCENT" };
  return { value: parseFloat(s) || 0, unit: "PIXELS" };
}

function parseLineHeight(v: string): LineHeight {
  const s = String(v).trim();
  if (s === "AUTO" || s === "") return { unit: "AUTO" };
  return { value: parseFloat(s), unit: "PIXELS" };
}

/** Map a Tokens-Studio fontWeight to a Figma font style name. */
function weightToStyle(weight: string): string {
  const w = (weight || "Regular").trim();
  // Tokens already use Figma style names (Regular/Medium/SemiBold/Bold).
  return w;
}

export async function createTextStyles(): Promise<string> {
  const group = TOKENS.textStyle;
  if (!group) return "text styles: none";
  const existing = figma.getLocalTextStyles();
  let count = 0;

  for (const key of Object.keys(group).filter((k) => !k.startsWith("_"))) {
    const v = group[key].value;
    const family: string = v.fontFamily;
    const style = weightToStyle(v.fontWeight);
    let fontName: FontName = { family, style };
    try {
      await figma.loadFontAsync(fontName);
    } catch {
      // Fall back to Regular if the requested weight isn't installed.
      try {
        fontName = { family, style: "Regular" };
        await figma.loadFontAsync(fontName);
      } catch {
        continue; // Family unavailable in this file — skip rather than throw.
      }
    }

    const name = `apex/${key}`;
    const ts = existing.find((s) => s.name === name) ?? figma.createTextStyle();
    ts.name = name;
    ts.fontName = fontName;
    ts.fontSize = parseFloat(String(v.fontSize)) || 12;
    ts.letterSpacing = parseLetterSpacing(v.letterSpacing ?? "0");
    ts.lineHeight = parseLineHeight(v.lineHeight ?? "AUTO");
    if (v.textCase === "uppercase") ts.textCase = "UPPER";
    count++;
  }
  return `text styles: ${count} published`;
}

export function createEffectStyles(): string {
  const group = TOKENS.effectStyle;
  if (!group) return "effect styles: none";
  const existing = figma.getLocalEffectStyles();
  let count = 0;

  for (const key of Object.keys(group).filter((k) => !k.startsWith("_"))) {
    const v = group[key].value;
    const c = parseColor(String(v.color));
    const shadow: DropShadowEffect = {
      type: "DROP_SHADOW",
      color: { r: c.r, g: c.g, b: c.b, a: c.a },
      offset: { x: parseFloat(String(v.x)) || 0, y: parseFloat(String(v.y)) || 0 },
      radius: parseFloat(String(v.blur)) || 0,
      spread: parseFloat(String(v.spread)) || 0,
      visible: true,
      blendMode: "NORMAL",
    };
    const name = `apex/${key}`;
    const es = existing.find((s) => s.name === name) ?? figma.createEffectStyle();
    es.name = name;
    es.effects = [shadow];
    count++;
  }
  return `effect styles: ${count} published`;
}
