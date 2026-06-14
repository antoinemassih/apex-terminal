//! Heuristics for mapping an arbitrary Figma file onto the apex-terminal engine.
//!
//! Two ladders, best-first:
//!   1. **By name** — if a color/text style or variable is named like an engine
//!      role (`accent`, `surface`, `bull`…), trust it.
//!   2. **By appearance** — otherwise classify frequent colours by
//!      luminance/saturation/hue and assign the still-empty roles.
//!
//! Everything here is a guess: the CLI emits a `figma-mapping.suggested.json`
//! so a human can correct any role it got wrong.

/** Engine ColorScheme roles we try to fill (others fall back to builtin_dark). */
export const COLOR_ROLES = [
  "bg", "surface", "text", "dim", "border", "accent",
  "bull", "bear", "warn", "success", "danger", "warning", "info",
];

/** Substrings (lowercased) that name a colour role. First match wins. */
export const ROLE_KEYWORDS = {
  bg: ["background", "/bg", "bg ", "canvas", "base/0", "neutral/0"],
  surface: ["surface", "panel", "card", "elevated", "layer", "fill/default"],
  text: ["text", "foreground", "/fg", "on-surface", "ink", "content/primary"],
  dim: ["dim", "muted", "secondary", "subtle", "content/secondary", "tertiary"],
  border: ["border", "divider", "outline", "stroke", "hairline"],
  accent: ["accent", "primary", "brand", "interactive", "link"],
  bull: ["bull", "positive", "success", "up", "gain", "profit", "green"],
  bear: ["bear", "negative", "danger", "down", "loss", "error", "red"],
  warn: ["warn", "warning", "caution", "amber", "alert", "yellow"],
  success: ["success"],
  danger: ["danger", "destructive", "critical"],
  warning: ["warning"],
  info: ["info", "informational", "notice"],
};

/** Component-name substring → recipe key (the engine's adopted/registered keys). */
export const RECIPE_REGISTRY = [
  { match: ["button/primary", "btn/primary", "button primary"], key: "button.primary" },
  { match: ["button/ghost", "button ghost"], key: "button.ghost" },
  { match: ["button/danger", "button danger"], key: "button.danger" },
  { match: ["tag", "chip", "pill"], key: "tag" },
  { match: ["tab"], key: "tab.line.active" },
  { match: ["list/row", "row", "list-item", "listitem"], key: "row.list" },
  { match: ["section", "section-header", "group-header"], key: "section.header" },
  { match: ["toast", "snackbar"], key: "toast" },
  { match: ["card", "panel"], key: "card" },
];

/** Engine tone refs usable in a RecipeSpec fill. */
export const TONE_REFS = ["accent", "bull", "bear", "warn", "text", "dim", "border", "surface", "bg"];

/** Relative luminance (0..1) of an {r,g,b} colour in 0..1 floats. */
export function luminance({ r, g, b }) {
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** HSV-ish saturation (0..1). */
export function saturation({ r, g, b }) {
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  return max === 0 ? 0 : (max - min) / max;
}

/** Hue in degrees (0..360). */
export function hue({ r, g, b }) {
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const d = max - min;
  if (d === 0) return 0;
  let h;
  if (max === r) h = ((g - b) / d) % 6;
  else if (max === g) h = (b - r) / d + 2;
  else h = (r - g) / d + 4;
  h *= 60;
  return h < 0 ? h + 360 : h;
}

/** Match a style/variable name to a colour role, or null. */
export function roleFromName(name) {
  const n = String(name || "").toLowerCase();
  for (const role of COLOR_ROLES) {
    const kws = ROLE_KEYWORDS[role] || [];
    if (kws.some((kw) => n.includes(kw))) return role;
  }
  return null;
}

/**
 * Fill still-missing roles from a list of {color, count} candidates by
 * appearance. Mutates and returns `palette` (role -> {r,g,b,a}).
 */
export function fillRolesByAppearance(palette, candidates) {
  // Sort by frequency desc; only consider reasonably-used colours.
  const ranked = [...candidates].sort((a, b) => b.count - a.count).map((c) => c.color);
  const want = (role) => !palette[role];

  // bg = darkest frequent; text = lightest frequent.
  const byLum = [...ranked].sort((a, b) => luminance(a) - luminance(b));
  if (want("bg") && byLum.length) palette.bg = byLum[0];
  if (want("text") && byLum.length) palette.text = byLum[byLum.length - 1];
  // surface = second-darkest distinct from bg.
  if (want("surface")) {
    const cand = byLum.find((c) => c !== palette.bg && luminance(c) < 0.4);
    if (cand) palette.surface = cand;
  }

  // Chromatic roles by hue among saturated colours.
  const chromatic = ranked.filter((c) => saturation(c) > 0.35 && luminance(c) > 0.15);
  const pickHue = (lo, hi) => chromatic.find((c) => { const h = hue(c); return h >= lo && h <= hi; });
  if (want("bull")) { const c = pickHue(90, 165); if (c) palette.bull = c; }   // green
  if (want("bear")) { const c = chromatic.find((c) => { const h = hue(c); return h <= 20 || h >= 340; }); if (c) palette.bear = c; } // red
  if (want("warn")) { const c = pickHue(35, 60); if (c) palette.warn = c; }    // amber
  if (want("accent")) {
    // Accent = most-saturated frequent colour not already claimed.
    const claimed = new Set([palette.bull, palette.bear, palette.warn].filter(Boolean));
    const c = chromatic.find((x) => !claimed.has(x));
    if (c) palette.accent = c;
  }
  if (want("dim") && palette.text) {
    // dim = a mid-luminance neutral, else skip (engine falls back).
    const c = byLum.find((x) => saturation(x) < 0.2 && luminance(x) > 0.25 && luminance(x) < 0.6);
    if (c) palette.dim = c;
  }
  if (want("border")) {
    const c = byLum.find((x) => saturation(x) < 0.25 && luminance(x) > 0.1 && luminance(x) < 0.35 && x !== palette.surface && x !== palette.bg);
    if (c) palette.border = c;
  }
  return palette;
}

/** Nearest engine tone (by hue/luminance) for a colour, for recipe fills. */
export function nearestTone(color, palette) {
  let best = null;
  let bestD = Infinity;
  for (const tone of TONE_REFS) {
    const ref = palette[tone];
    if (!ref) continue;
    const d = Math.abs(luminance(color) - luminance(ref)) + Math.abs((hue(color) - hue(ref)) / 360);
    if (d < bestD) { bestD = d; best = tone; }
  }
  return best;
}

/** Map a component(-set) name to a recipe key, or null. */
export function recipeKeyFromName(name) {
  const n = String(name || "").toLowerCase();
  for (const entry of RECIPE_REGISTRY) {
    if (entry.match.some((m) => n.includes(m))) return entry.key;
  }
  return null;
}

/** Snap a value to the nearest tier given a sorted list of canonical sizes. */
export function nearestTierName(value, tiers) {
  let best = tiers[0];
  let bestD = Infinity;
  for (const [name, v] of tiers) {
    const d = Math.abs(value - v);
    if (d < bestD) { bestD = d; best = name; }
  }
  return best;
}
