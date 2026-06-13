//! Apex Terminal — Theme Scaffold (PUSH half).
//!
//! Entry point. Runs in the Figma desktop app and scaffolds a complete starting
//! point from `apex.tokens.json` + `component-inventory.json`:
//!   1. variables (ColorScheme / StyleSystem / CmdPalette with Aperture+Meridien),
//!   2. text + effect styles,
//!   3. a component library named to recipe keys, bound to the variables,
//!   4. per-region layout frames.
//!
//! The designer styles the result; the PULL transformer
//! (`design_system::import`) reads the finished file back into a `.apextheme`.

import { createVariables } from "./variables";
import { createTextStyles, createEffectStyles } from "./styles";
import { createComponents } from "./components";
import { createFrames } from "./frames";
import { buildFigmaThemeExportJson } from "./exporter";

type Part = "variables" | "styles" | "components" | "frames" | "all";

interface RunMessage {
  type: "run";
  parts: Part[];
}

interface ExportMessage {
  type: "export-json";
}

type PluginMessage = RunMessage | ExportMessage;

figma.showUI(__html__, { width: 320, height: 360 });

function post(line: string): void {
  figma.ui.postMessage({ type: "log", line });
}

async function run(parts: Part[]): Promise<void> {
  const all = parts.includes("all");
  const log: string[] = [];
  try {
    // Variables first — components and frames bind to them.
    if (all || parts.includes("variables") || parts.includes("components") || parts.includes("frames")) {
      log.push(createVariables());
      post(log[log.length - 1]);
    }
    if (all || parts.includes("styles")) {
      log.push(createEffectStyles());
      post(log[log.length - 1]);
      log.push(await createTextStyles());
      post(log[log.length - 1]);
    }
    if (all || parts.includes("components")) {
      log.push(await createComponents());
      post(log[log.length - 1]);
    }
    if (all || parts.includes("frames")) {
      log.push(await createFrames());
      post(log[log.length - 1]);
    }
    figma.notify("Apex scaffold complete");
    figma.ui.postMessage({ type: "done", summary: log });
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    figma.notify(`Apex scaffold failed: ${message}`, { error: true });
    figma.ui.postMessage({ type: "error", message });
  }
}

function exportJson(): void {
  try {
    const json = buildFigmaThemeExportJson();
    figma.ui.postMessage({ type: "export-ready", json });
    figma.notify("Figma theme export ready — saving JSON");
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    figma.notify(`Export failed: ${message}`, { error: true });
    figma.ui.postMessage({ type: "error", message });
  }
}

figma.ui.onmessage = (msg: PluginMessage) => {
  if (msg.type === "run") {
    void run(msg.parts);
  } else if (msg.type === "export-json") {
    exportJson();
  }
};
