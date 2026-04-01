#!/bin/bash
# Build Apex Terminal standalone native binary with icon
set -e

cd "$(dirname "$0")/src-tauri"

echo "Building apex-native (release)..."
cargo build --bin apex-native --release

echo "Patching icon..."
RCEDIT="$APPDATA/npm/node_modules/rcedit/bin/rcedit-x64.exe"
"$RCEDIT" target/release/apex-native.exe --set-icon icons/apex-native.ico

echo ""
echo "Done: src-tauri/target/release/apex-native.exe ($(du -h target/release/apex-native.exe | cut -f1))"
