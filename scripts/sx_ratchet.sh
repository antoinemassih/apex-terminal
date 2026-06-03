#!/usr/bin/env bash
# Sx migration ratchet — Phase 4 guardrail.
#
# Blocks NEW raw `color_alpha(theme/t.<tone>, …)` sites in the UI layers while
# grandfathering the known remainder. As sites are migrated to `tint()` /
# `palette_ct()`, lower BASELINE to lock in the gain (the script tells you to).
#
# Why a count ratchet rather than a hard ban: ~80 legacy sites remain
# (nested-paren alpha exprs that regex can't safely rewrite, plus style.rs's
# own bridge internals). A hard ban would fail today; the ratchet lets the
# number only ever go down.
#
# Usage:  bash scripts/sx_ratchet.sh        # check (exit 1 if count went up)
# CI:     wire into the pre-merge workflow alongside `cargo check`.

set -euo pipefail
cd "$(dirname "$0")/.."

BASELINE=81

PATTERN='color_alpha\((theme|t)\.(accent|dim|text|bull|bear|warn|bg|toolbar_border|toolbar_bg)'
ROOTS="src-tauri/src/chart/renderer/ui src-tauri/src/ui_kit/widgets"

count=$(grep -rhoE "$PATTERN" $ROOTS 2>/dev/null | wc -l | tr -d ' ')

echo "Sx ratchet: $count un-migrated tone sites (baseline $BASELINE)"

if [ "$count" -gt "$BASELINE" ]; then
  echo "FAIL: $((count - BASELINE)) new raw color_alpha(theme/t.<tone>) site(s) added."
  echo "      Use tint(t, Tone::X, alpha) (gpu side) or palette_ct(t).base(Tone::X) (ui_kit)."
  echo "      Offending files:"
  grep -rlE "$PATTERN" $ROOTS 2>/dev/null | sed 's/^/        /'
  exit 1
fi

if [ "$count" -lt "$BASELINE" ]; then
  echo "RATCHET DOWN: $((BASELINE - count)) site(s) migrated since baseline."
  echo "  -> lower BASELINE in scripts/sx_ratchet.sh to $count to lock it in."
fi

echo "OK"
