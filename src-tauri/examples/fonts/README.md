# Font candidates for Apex Terminal

This directory holds candidate monospace fonts for evaluation. Run
`cargo run --example font_showcase` (from `src-tauri/`) to compare them
visually side-by-side.

These fonts are **NOT** embedded in the apex-native binary. The directory
exists purely to support the showcase example. The `.ttf`/`.otf` files
are gitignored (see `src-tauri/.gitignore`) so the ~2 MB of font data
does not enter the repo.

## Candidates

| Font | License | Source |
|---|---|---|
| JetBrains Mono | OFL 1.1 | Copied from `src/ui_kit/JetBrainsMono-Regular.ttf` (current default in the main app) |
| Commit Mono | OFL 1.1 | https://github.com/eigilnikolajsen/commit-mono/releases/tag/v1.143 (release zip) |
| Geist Mono | OFL 1.1 | https://github.com/vercel/geist-font (raw `packages/next/dist/fonts/geist-mono/`) |
| IBM Plex Mono | OFL 1.1 | https://github.com/IBM/plex/releases (`@ibm/plex-mono@1.1.0` zip) |
| Cascadia Code | OFL 1.1 | https://github.com/microsoft/cascadia-code/releases/tag/v2407.24 (release zip, only `ttf/static/CascadiaCode-Regular.ttf` extracted) |

## Skipped

- **Iosevka** — only ships as a 100+ MB zip per release with no individual
  TTF download. Not worth the hassle for a comparison; the other 5 cover
  the design space (humanist, geometric, square, programmer-ligature,
  conservative).

## Re-downloading

If the font files are missing (fresh checkout, since they're gitignored),
run:

```bash
mkdir -p examples/fonts
curl -L -o examples/fonts/GeistMono-Regular.ttf "https://github.com/vercel/geist-font/raw/main/packages/next/dist/fonts/geist-mono/GeistMono-Regular.ttf"
curl -L -o examples/fonts/GeistMono-Bold.ttf "https://github.com/vercel/geist-font/raw/main/packages/next/dist/fonts/geist-mono/GeistMono-Bold.ttf"

# CommitMono (release zip)
curl -L -o /tmp/cm.zip "https://github.com/eigilnikolajsen/commit-mono/releases/download/v1.143/CommitMono-1.143.zip"
unzip -j /tmp/cm.zip "CommitMono-1.143/CommitMono-400-Regular.otf" -d examples/fonts/
unzip -j /tmp/cm.zip "CommitMono-1.143/CommitMono-700-Regular.otf" -d examples/fonts/
mv examples/fonts/CommitMono-700-Regular.otf examples/fonts/CommitMono-700-Bold.otf

# IBM Plex (release zip)
curl -L -o /tmp/plex.zip "https://github.com/IBM/plex/releases/download/%40ibm%2Fplex-mono%401.1.0/ibm-plex-mono.zip"
unzip -j /tmp/plex.zip "ibm-plex-mono/fonts/complete/ttf/IBMPlexMono-Regular.ttf" -d examples/fonts/
unzip -j /tmp/plex.zip "ibm-plex-mono/fonts/complete/ttf/IBMPlexMono-Bold.ttf"   -d examples/fonts/

# Cascadia Code (release zip — only one file extracted)
curl -L -o /tmp/cc.zip "https://github.com/microsoft/cascadia-code/releases/download/v2407.24/CascadiaCode-2407.24.zip"
unzip -j /tmp/cc.zip "ttf/static/CascadiaCode-Regular.ttf" -d examples/fonts/

# JetBrains Mono — already in src/ui_kit/
cp src/ui_kit/JetBrainsMono-Regular.ttf examples/fonts/
cp src/ui_kit/JetBrainsMono-Bold.ttf    examples/fonts/
```
