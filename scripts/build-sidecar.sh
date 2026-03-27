#!/usr/bin/env bash
# Compiles ococo-api into a standalone exe and places it where Tauri expects it.
# Run this before `tauri build` whenever ococo-api changes.
#
# Requires: npm, pkg (installed globally via `npm i -g pkg`)
#   npm install -g pkg

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
API_DIR="$REPO_ROOT/ococo-api"
OUT_DIR="$REPO_ROOT/src-tauri/binaries"

echo "==> Building ococo-api TypeScript..."
cd "$API_DIR"
npm run build

echo "==> Bundling into standalone binary with pkg..."
# Detect host triple so the binary name matches what Tauri expects
TRIPLE=$(rustc -vV 2>/dev/null | sed -n 's/^host: //p')
if [ -z "$TRIPLE" ]; then
  echo "ERROR: rustc not found. Install Rust to detect the target triple."
  exit 1
fi

echo "    Target triple: $TRIPLE"

# pkg target map
case "$TRIPLE" in
  x86_64-pc-windows-msvc)   PKG_TARGET="node22-win-x64" ;  EXT=".exe" ;;
  aarch64-pc-windows-msvc)  PKG_TARGET="node22-win-arm64"; EXT=".exe" ;;
  x86_64-apple-darwin)      PKG_TARGET="node22-macos-x64"; EXT="" ;;
  aarch64-apple-darwin)     PKG_TARGET="node22-macos-arm64"; EXT="" ;;
  x86_64-unknown-linux-gnu) PKG_TARGET="node22-linux-x64"; EXT="" ;;
  *)
    echo "ERROR: Unsupported triple '$TRIPLE'. Add a case to this script."
    exit 1
    ;;
esac

BINARY_NAME="ococo-api-${TRIPLE}${EXT}"

npx --yes @yao-pkg/pkg dist/index.js \
  --target "$PKG_TARGET" \
  --output "$OUT_DIR/$BINARY_NAME"

echo "==> Done: $OUT_DIR/$BINARY_NAME"
echo "    You can now run: cd src-tauri && cargo tauri build"
