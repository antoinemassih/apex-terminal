#!/usr/bin/env node
//! One-command setup for the Apex Terminal scaffold plugin.
//!
//! Installs deps, typechecks, builds the runtime bundle (dist/code.js), and
//! prints the exact steps to import + run the plugin in Figma. Run with:
//!
//!   cd figma-plugin && npm run setup        (or: node setup.mjs)

import { execSync } from "node:child_process";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const manifest = join(here, "manifest.json");

function run(label, cmd) {
  process.stdout.write(`\n▶ ${label}\n  $ ${cmd}\n`);
  execSync(cmd, { cwd: here, stdio: "inherit" });
}

try {
  // npm ci is reproducible when a lockfile exists; fall back to install.
  run("Installing dependencies", existsSync(join(here, "package-lock.json")) ? "npm ci" : "npm install");
  run("Typechecking (tsc)", "npm run typecheck");
  run("Building runtime bundle (esbuild → dist/code.js)", "npm run build");

  console.log(`
✅ Plugin built.

Next (one-time, in the Figma DESKTOP app — variables/components can only be
created from inside Figma):

  1. Plugins → Development → Import plugin from manifest…
  2. Choose:  ${manifest}
  3. Run:     Plugins → Development → Apex Terminal — Theme Scaffold
  4. Click "Generate everything" to create the variables (Aperture/Meridien
     modes), text + effect styles, component library, and layout frames.

Round-trip back into the app:
  • Restyle in Figma, then in the plugin click "⤓ Export this file → JSON".
  • In the app: Settings → Appearance → Theme Studio → Import from Figma → pick
    the JSON. (To extract an EXISTING design instead, use ../figma-extract.)
`);
} catch (err) {
  console.error(`\n✗ Setup failed: ${err.message}`);
  console.error("  Fix the error above and re-run `npm run setup`.");
  process.exitCode = 1;
}
