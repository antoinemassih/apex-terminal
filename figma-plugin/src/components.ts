//! Stage 3 — Component library.
//!
//! For each entry in `component-inventory.json` this builds a Figma component
//! set whose variant names carry the **recipe key** (e.g. a Button "Primary"
//! variant is tagged `button.primary`, a Tabs "Line" active variant
//! `tab.line.active`). Fills, strokes, and corners bind to the variables created
//! in stage 1 — never raw values — so the PULL importer reads the variant style
//! back into a `RecipeSpec`. Re-running rebuilds the set in place.
//!
//! Coverage note: this seeds the recipe-bearing + core components (Button, Tag,
//! StatusPill, Badge, Tabs, PanelListRow, PanelSection, Input, Alert, Toast).
//! Extend `component-inventory.json` to grow the library; the generator is fully
//! data-driven.

import INVENTORY from "../component-inventory.json";
import { COLLECTION } from "./tokens";
import { getVar } from "./variables";

interface ComponentSpec {
  name: string;
  props: { [prop: string]: string[] };
  recipeByVariant?: { [value: string]: string };
  fillByVariant?: { [value: string]: string | null };
  strokeByVariant?: { [value: string]: string };
  stripeByVariant?: { [value: string]: string };
  textByVariant?: { [value: string]: string };
  fillAlpha?: number;
  cornerVar?: string;
  heightVar?: string;
}

const SPECS: ComponentSpec[] = (INVENTORY as unknown as { components: ComponentSpec[] }).components;

const CORNER_FIELDS: VariableBindableNodeField[] = [
  "topLeftRadius", "topRightRadius", "bottomLeftRadius", "bottomRightRadius",
];

/** Cartesian product of the variant property values. */
function cartesian(props: { [prop: string]: string[] }): Array<Record<string, string>> {
  const keys = Object.keys(props);
  let acc: Array<Record<string, string>> = [{}];
  for (const key of keys) {
    const next: Array<Record<string, string>> = [];
    for (const partial of acc) {
      for (const val of props[key]) {
        next.push({ ...partial, [key]: val });
      }
    }
    acc = next;
  }
  return acc;
}

/** Look up the fill/stroke/etc. value for a combo by scanning its variant values. */
function pick(map: { [v: string]: any } | undefined, combo: Record<string, string>): any {
  if (!map) return undefined;
  for (const v of Object.values(combo)) {
    if (v in map) return map[v];
  }
  return undefined;
}

function boundSolid(tone: string | null | undefined, alpha?: number): Paint[] {
  if (tone == null) return [];
  const variable = getVar(COLLECTION.color, tone);
  const paint: SolidPaint = {
    type: "SOLID",
    color: { r: 0.5, g: 0.5, b: 0.5 },
    opacity: typeof alpha === "number" ? Math.max(0, Math.min(1, alpha / 255)) : 1,
  };
  if (variable) {
    return [figma.variables.setBoundVariableForPaint(paint, "color", variable)];
  }
  return [paint];
}

function getOrCreatePage(name: string): PageNode {
  const found = figma.root.children.find((c) => c.type === "PAGE" && c.name === name) as PageNode | undefined;
  if (found) return found;
  const page = figma.createPage();
  page.name = name;
  return page;
}

export async function createComponents(): Promise<string> {
  const page = getOrCreatePage("Apex — Components");

  // Load a label font once; skip labels if unavailable.
  let labelFont: FontName | null = { family: "Inter", style: "Regular" };
  try {
    await figma.loadFontAsync(labelFont);
  } catch {
    try {
      labelFont = { family: "Roboto", style: "Regular" };
      await figma.loadFontAsync(labelFont);
    } catch {
      labelFont = null;
    }
  }

  let setCount = 0;
  let originX = 0;

  for (const spec of SPECS) {
    // Rebuild: remove an existing set of the same name.
    const prior = page.children.find((c) => c.name === spec.name);
    if (prior) prior.remove();

    const combos = cartesian(spec.props);
    const components: ComponentNode[] = [];
    let row = 0;

    for (const combo of combos) {
      const comp = figma.createComponent();
      comp.name = Object.entries(combo).map(([k, v]) => `${k}=${v}`).join(", ");
      comp.resizeWithoutConstraints(120, 32);

      // Corner radius — bind to a radius variable when present.
      const radiusVar = spec.cornerVar ? getVar(COLLECTION.style, spec.cornerVar) : undefined;
      if (radiusVar) {
        for (const field of CORNER_FIELDS) comp.setBoundVariable(field, radiusVar);
      } else {
        comp.cornerRadius = 6;
      }

      // Fill.
      comp.fills = boundSolid(pick(spec.fillByVariant, combo), spec.fillAlpha);

      // Stroke (e.g. Input states).
      const strokeTone = pick(spec.strokeByVariant, combo);
      if (strokeTone) {
        comp.strokes = boundSolid(strokeTone);
        comp.strokeWeight = 1;
      }

      // Accent stripe (e.g. Toast) — a thin left bar bound to a tone variable.
      const stripeTone = pick(spec.stripeByVariant, combo);
      if (stripeTone) {
        const stripe = figma.createRectangle();
        stripe.resize(3, 32);
        stripe.x = 0;
        stripe.y = 0;
        stripe.fills = boundSolid(stripeTone);
        comp.appendChild(stripe);
      }

      // Label.
      if (labelFont) {
        const label = figma.createText();
        label.fontName = labelFont;
        label.characters = Object.values(combo).join(" / ");
        label.fontSize = 11;
        label.x = 8;
        label.y = 9;
        const textTone = pick(spec.textByVariant, combo);
        if (textTone) label.fills = boundSolid(textTone);
        comp.appendChild(label);
      }

      // Tag the variant with its recipe key for the PULL importer.
      const recipeKey = pick(spec.recipeByVariant, combo);
      if (recipeKey) comp.setPluginData("recipeKey", recipeKey);

      comp.x = (row % 6) * 140;
      comp.y = Math.floor(row / 6) * 48;
      components.push(comp);
      row++;
    }

    const set = figma.combineAsVariants(components, page);
    set.name = spec.name;
    set.x = originX;
    set.y = 0;
    // Record the component's base recipe key (first defined) on the set.
    const baseKey = spec.recipeByVariant ? Object.values(spec.recipeByVariant)[0] : "";
    if (baseKey) set.setPluginData("recipeKeyBase", baseKey);
    originX += set.width + 80;
    setCount++;
  }

  return `components: ${setCount} component sets built on "Apex — Components"`;
}
