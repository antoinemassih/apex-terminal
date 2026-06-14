#!/usr/bin/env node
//! apex-figma-extract — read an existing Figma file and emit a FigmaThemeExport.
//!
//! Usage:
//!   node src/extract.mjs --file <FILE_KEY> --token <PAT>
//!   node src/extract.mjs --creds <path-to-figma_credentials.md>   # parses PAT + key
//!   FIGMA_PAT=<pat> node src/extract.mjs --file <FILE_KEY>
//!
//! Output (default ./extracted.figma.json) drops straight into the app's
//! "Import from Figma" button or design_system::import. A companion
//! figma-mapping.suggested.json records every guess for human correction.

import fs from "node:fs";
import {
  COLOR_ROLES, roleFromName, fillRolesByAppearance, nearestTone,
  recipeKeyFromName, luminance,
} from "./heuristics.mjs";

const MODE = "Imported";

// ── args / creds ────────────────────────────────────────────────────────────

function parseArgs(argv) {
  const a = {};
  for (let i = 2; i < argv.length; i++) {
    const k = argv[i];
    if (k.startsWith("--")) a[k.slice(2)] = argv[i + 1]?.startsWith("--") || argv[i + 1] === undefined ? true : argv[++i];
  }
  return a;
}

function resolveCreds(args) {
  let token = args.token || process.env.FIGMA_PAT || process.env.FIGMA_TOKEN;
  let file = args.file;
  if (args.creds && fs.existsSync(args.creds)) {
    const md = fs.readFileSync(args.creds, "utf8");
    token = token || md.match(/figd_[A-Za-z0-9_-]+/)?.[0];
    file = file || md.match(/File:\s*`([A-Za-z0-9]+)`/)?.[1];
  }
  return { token, file };
}

// ── Figma REST ──────────────────────────────────────────────────────────────

async function fetchFile(fileKey, token) {
  const url = `https://api.figma.com/v1/files/${fileKey}`;
  const res = await fetch(url, { headers: { "X-Figma-Token": token } });
  if (!res.ok) {
    const body = await res.text().catch(() => "");
    throw new Error(`Figma API ${res.status} ${res.statusText}: ${body.slice(0, 200)}`);
  }
  return res.json();
}

// ── node walk / collection ────────────────────────────────────────────────────

function collect(doc, stylesMeta) {
  const fillFreq = new Map();           // hexKey -> { color, count }
  const styleColor = {};                // styleId -> color
  const fontSizes = [];
  const fontFamilies = new Map();       // family -> count
  const radii = [];
  const spacings = [];
  const componentSets = [];             // { name, color?, radius? }

  const eff = (fill) => {
    const c = fill.color || { r: 0, g: 0, b: 0 };
    const a = (c.a ?? 1) * (fill.opacity ?? 1);
    return { r: c.r, g: c.g, b: c.b, a };
  };
  const noteColor = (color) => {
    const key = `${color.r.toFixed(3)},${color.g.toFixed(3)},${color.b.toFixed(3)}`;
    const e = fillFreq.get(key);
    if (e) e.count++;
    else fillFreq.set(key, { color: { r: color.r, g: color.g, b: color.b, a: color.a ?? 1 }, count: 1 });
  };

  const visit = (node) => {
    if (!node || node.visible === false) return;

    if (Array.isArray(node.fills)) {
      for (const f of node.fills) {
        if (f.type === "SOLID" && f.visible !== false) {
          const color = eff(f);
          noteColor(color);
          if (node.styles?.fill) styleColor[node.styles.fill] = color;
        }
      }
    }
    if (Array.isArray(node.strokes)) {
      for (const s of node.strokes) {
        if (s.type === "SOLID" && s.visible !== false) {
          const color = eff(s);
          noteColor(color);
          if (node.styles?.stroke) styleColor[node.styles.stroke] = color;
        }
      }
    }
    if (node.type === "TEXT" && node.style?.fontSize) {
      fontSizes.push(node.style.fontSize);
      if (node.style.fontFamily) fontFamilies.set(node.style.fontFamily, (fontFamilies.get(node.style.fontFamily) || 0) + 1);
    }
    if (typeof node.cornerRadius === "number" && node.cornerRadius > 0) radii.push(node.cornerRadius);
    if (Array.isArray(node.rectangleCornerRadii)) for (const r of node.rectangleCornerRadii) if (r > 0) radii.push(r);
    if (node.layoutMode && node.layoutMode !== "NONE") {
      if (node.itemSpacing > 0) spacings.push(node.itemSpacing);
      for (const p of ["paddingLeft", "paddingRight", "paddingTop", "paddingBottom"]) if (node[p] > 0) spacings.push(node[p]);
    }
    if (node.type === "COMPONENT_SET" || node.type === "COMPONENT") {
      const key = recipeKeyFromName(node.name);
      if (key) {
        // Representative fill/corner from the node or its first child.
        const rep = node.type === "COMPONENT_SET" ? node.children?.[0] ?? node : node;
        const solid = Array.isArray(rep.fills) ? rep.fills.find((f) => f.type === "SOLID" && f.visible !== false) : null;
        const fill = solid ? eff(solid) : null;
        const radius = typeof rep.cornerRadius === "number" ? rep.cornerRadius : undefined;
        componentSets.push({ name: node.name, key, fill, radius });
      }
    }
    if (Array.isArray(node.children)) for (const c of node.children) visit(c);
  };

  visit(doc);

  // Name the color styles from the file's styles metadata.
  const namedColors = [];
  for (const [styleId, color] of Object.entries(styleColor)) {
    const name = stylesMeta?.[styleId]?.name || styleId;
    namedColors.push({ name, color });
  }

  return { fillFreq, namedColors, fontSizes, fontFamilies, radii, spacings, componentSets };
}

// ── summaries ────────────────────────────────────────────────────────────────

const uniqSorted = (arr) => [...new Set(arr.map((n) => Math.round(n * 100) / 100))].sort((a, b) => a - b);

/** 5-number-ish spread mapped to tier names; returns [ [name, value], ... ]. */
function spread(values, names) {
  const u = uniqSorted(values);
  if (u.length === 0) return [];
  const at = (frac) => u[Math.min(u.length - 1, Math.round(frac * (u.length - 1)))];
  const fracs = names.map((_, i) => i / (names.length - 1 || 1));
  return names.map((name, i) => [name, at(fracs[i])]);
}

// ── build envelope ─────────────────────────────────────────────────────────────

function buildEnvelope(c) {
  const variables = [];
  const suggestions = { ColorScheme: {}, StyleSystem: {}, _notes: [] };

  // Palette: by-name first, then by-appearance.
  const palette = {};
  for (const { name, color } of c.namedColors) {
    const role = roleFromName(name);
    if (role && !palette[role]) {
      palette[role] = color;
      suggestions.ColorScheme[name] = role;
    }
  }
  const beforeAppearance = new Set(Object.keys(palette));
  fillRolesByAppearance(palette, [...c.fillFreq.values()]);
  for (const role of Object.keys(palette)) {
    if (!beforeAppearance.has(role)) suggestions._notes.push(`role '${role}' guessed by appearance (verify)`);
  }

  for (const role of COLOR_ROLES) {
    const color = palette[role];
    if (!color) continue;
    variables.push({
      collection: "ColorScheme", name: role, type: "COLOR",
      valuesByMode: { [MODE]: { r: color.r, g: color.g, b: color.b, a: color.a ?? 1 } },
    });
  }

  // Typography.
  const typeTiers = spread(c.fontSizes, ["size_xs", "size_sm", "size_md", "size_lg", "size_xl"]);
  for (const [name, v] of typeTiers) {
    variables.push({ collection: "StyleSystem", name: `typography/${name}`, type: "FLOAT", valuesByMode: { [MODE]: v } });
  }
  // Dominant UI font family.
  if (c.fontFamilies.size) {
    const family = [...c.fontFamilies.entries()].sort((a, b) => b[1] - a[1])[0][0];
    variables.push({ collection: "StyleSystem", name: "typography/family_ui", type: "STRING", valuesByMode: { [MODE]: family } });
  }

  // Radii (none always 0).
  variables.push({ collection: "StyleSystem", name: "radii/none", type: "FLOAT", valuesByMode: { [MODE]: 0 } });
  for (const [name, v] of spread(c.radii, ["xs", "sm", "md", "lg"])) {
    variables.push({ collection: "StyleSystem", name: `radii/${name}`, type: "FLOAT", valuesByMode: { [MODE]: v } });
  }

  // Spacing.
  for (const [name, v] of spread(c.spacings, ["xs", "sm", "md", "lg", "xl"])) {
    variables.push({ collection: "StyleSystem", name: `spacing/${name}`, type: "FLOAT", valuesByMode: { [MODE]: v } });
  }

  // Components → recipes.
  const components = {};
  for (const cs of c.componentSets) {
    if (components[cs.key]) continue;
    const spec = {};
    if (cs.fill && (cs.fill.r || cs.fill.g || cs.fill.b)) {
      const tone = nearestTone(cs.fill, palette);
      if (tone) spec.fill = { kind: "tone", tone };
    }
    components[cs.key] = spec;
    suggestions._notes.push(`component '${cs.name}' -> recipe '${cs.key}'`);
  }

  const isDark = palette.bg ? luminance(palette.bg) < 0.5 : true;
  const meta = { [MODE]: { id: "imported", name: "Imported", is_dark: isDark } };

  const envelope = {
    schema: 1,
    generator: "apex-figma-extract (REST)",
    colorCollection: { name: "ColorScheme", defaultMode: MODE, modes: [MODE], meta },
    styleCollection: { name: "StyleSystem", defaultMode: MODE, modes: [MODE], meta },
    variables,
    components,
  };
  return { envelope, suggestions, palette };
}

// ── main ────────────────────────────────────────────────────────────────────

async function main() {
  const args = parseArgs(process.argv);
  const { token, file } = resolveCreds(args);
  if (!args.input && (!token || !file)) {
    console.error("Need a Figma PAT and file key.\n  --file <KEY> --token <PAT>\n  --creds <figma_credentials.md>\n  or FIGMA_PAT env var");
    process.exit(2);
  }
  const out = args.out || "extracted.figma.json";
  const mapOut = args.mapping || "figma-mapping.suggested.json";

  let fileJson;
  if (args.input) {
    // Offline mode: read a cached `GET /v1/files/:key` response from disk.
    console.error(`• Reading cached Figma JSON ${args.input} …`);
    fileJson = JSON.parse(fs.readFileSync(args.input, "utf8"));
  } else {
    console.error(`• Fetching Figma file ${file} …`);
    fileJson = await fetchFile(file, token);
  }
  console.error(`• "${fileJson.name}" — walking nodes …`);

  const collected = collect(fileJson.document, fileJson.styles);
  const { envelope, suggestions, palette } = buildEnvelope(collected);

  fs.writeFileSync(out, JSON.stringify(envelope, null, 2));
  fs.writeFileSync(mapOut, JSON.stringify(suggestions, null, 2));

  // Summary.
  const colorCount = envelope.variables.filter((v) => v.collection === "ColorScheme").length;
  const styleCount = envelope.variables.filter((v) => v.collection === "StyleSystem").length;
  console.error("\n── Extraction summary ───────────────────────────");
  console.error(`  palette roles : ${Object.keys(palette).join(", ") || "(none)"}  [${colorCount} colours]`);
  console.error(`  style tokens  : ${styleCount} (type/radii/spacing)`);
  console.error(`  components    : ${Object.keys(envelope.components).length} (${Object.keys(envelope.components).join(", ") || "none"})`);
  console.error(`  fills seen    : ${collected.fillFreq.size} distinct, ${collected.fontSizes.length} text nodes`);
  console.error(`\n  → ${out}            (load via Theme Studio ▸ Import from Figma)`);
  console.error(`  → ${mapOut}  (review the guesses; correct roles here)`);
  if (colorCount < 4) console.error("\n  ⚠ Few colours resolved — this file may use raw fills with no styles. Review the mapping.");
}

// Set exitCode (not process.exit) so Node drains pending sockets cleanly.
main().catch((e) => { console.error("✗ " + e.message); process.exitCode = 1; });
