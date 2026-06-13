//! Stage 4 — Per-region layout frames.
//!
//! Generates a starter `TradingScreen` with the app's shell regions as named,
//! auto-layout frames (`top_nav`, `toolnav`, `account_strip`, `workspace_rail`,
//! `central`, `right_rail`, `bottom_dock`). Region names are kept **stable and
//! semantic** because the future `ShellProfile` concept will consume them. Fills
//! bind to the `ColorScheme` variables; this is the seed a designer fleshes out.

import { COLLECTION } from "./tokens";
import { getVar } from "./variables";

function boundFill(tone: string): Paint[] {
  const variable = getVar(COLLECTION.color, tone);
  const paint: SolidPaint = { type: "SOLID", color: { r: 0.1, g: 0.1, b: 0.1 } };
  if (variable) return [figma.variables.setBoundVariableForPaint(paint, "color", variable)];
  return [paint];
}

function getOrCreatePage(name: string): PageNode {
  const found = figma.root.children.find((c) => c.type === "PAGE" && c.name === name) as PageNode | undefined;
  if (found) return found;
  const page = figma.createPage();
  page.name = name;
  return page;
}

function region(
  name: string,
  tone: string,
  width: number,
  height: number,
  labelFont: FontName | null,
): FrameNode {
  const f = figma.createFrame();
  f.name = name;
  f.resize(width, height);
  f.fills = boundFill(tone);
  f.cornerRadius = 8;
  if (labelFont) {
    const label = figma.createText();
    label.fontName = labelFont;
    label.characters = name;
    label.fontSize = 11;
    label.x = 10;
    label.y = 8;
    label.fills = boundFill("dim");
    f.appendChild(label);
  }
  return f;
}

export async function createFrames(): Promise<string> {
  const page = getOrCreatePage("Apex — Layout");

  const prior = page.children.find((c) => c.name === "TradingScreen");
  if (prior) prior.remove();

  let labelFont: FontName | null = { family: "Inter", style: "Regular" };
  try {
    await figma.loadFontAsync(labelFont);
  } catch {
    labelFont = null;
  }

  const W = 1280;

  const root = figma.createFrame();
  root.name = "TradingScreen";
  root.resize(W, 800);
  root.fills = boundFill("bg");
  root.layoutMode = "VERTICAL";
  root.itemSpacing = 8;
  root.paddingTop = 8;
  root.paddingBottom = 8;
  root.paddingLeft = 8;
  root.paddingRight = 8;
  root.primaryAxisSizingMode = "FIXED";
  root.counterAxisSizingMode = "FIXED";

  // Top chrome rows.
  root.appendChild(region("top_nav", "surface", W - 16, 44, labelFont));
  root.appendChild(region("toolnav", "surface", W - 16, 30, labelFont));
  root.appendChild(region("account_strip", "surface", W - 16, 26, labelFont));

  // Middle: workspace_rail | central | right_rail.
  const middle = figma.createFrame();
  middle.name = "MainContent";
  middle.layoutMode = "HORIZONTAL";
  middle.itemSpacing = 8;
  middle.fills = [];
  middle.resize(W - 16, 600);
  middle.primaryAxisSizingMode = "FIXED";
  middle.counterAxisSizingMode = "FIXED";
  middle.appendChild(region("workspace_rail", "surface", 56, 584, labelFont));
  middle.appendChild(region("central", "bg", W - 16 - 56 - 320 - 16, 584, labelFont));
  middle.appendChild(region("right_rail", "surface", 320, 584, labelFont));
  root.appendChild(middle);

  // Bottom dock.
  root.appendChild(region("bottom_dock", "surface", W - 16, 80, labelFont));

  root.x = 0;
  root.y = 0;
  return `frames: TradingScreen with 7 named regions on "Apex — Layout"`;
}
