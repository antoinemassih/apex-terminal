#!/usr/bin/env bash
# legacy_tone_ratchet — Phase 4 guardrail (formerly misnamed "Sx ratchet").
#
# HONESTY NOTE (2026-08-02 rewrite): the old version of this script grepped the
# legacy colour pattern, called itself an "Sx ratchet", and had a stale
# BASELINE=4 while the live count was 18 — so it failed on every run and taught
# people to ignore it. It now does two things, both truthfully labelled:
#
#   CHECK 1 — legacy tone ratchet: count of raw `color_alpha(theme/t.<tone>)`
#             sites in the UI layers. May only go DOWN. Baseline is the live
#             count at rewrite time; lower it as sites migrate to `tint()` /
#             `palette_ct()` (the script tells you when).
#
#   CHECK 2 — Sx adoption floor: count of production `Sx::new()` sites in
#             src-tauri/src/ui_kit (excluding #[cfg(test)] modules and the
#             sx/recipes.rs demo). May only go UP. A drop means someone ripped
#             out Sx usage — that is a regression, not a cleanup.
#
# Usage:  bash scripts/sx_ratchet.sh        # exit 1 on any regression
# CI:     wired into the design-gates workflow alongside check-design-system.sh.

set -euo pipefail
cd "$(dirname "$0")/.."

# ── CHECK 1: legacy color_alpha(theme.<tone>) sites — may only decrease ──────
# Live count at 2026-08-02 rewrite: 18. Lower this as sites migrate.
LEGACY_BASELINE=18

LEGACY_PATTERN='color_alpha\((theme|t)\.(accent|dim|text|bull|bear|warn|bg|toolbar_border|toolbar_bg)'
LEGACY_ROOTS="src-tauri/src/chart/renderer/ui src-tauri/src/ui_kit/widgets"

legacy_count=$(grep -rhoE "$LEGACY_PATTERN" $LEGACY_ROOTS 2>/dev/null | wc -l | tr -d ' ')

# ── CHECK 2: Sx::new() production adoption in ui_kit — may only increase ─────
# Floor at 2026-08-02 capture: 19 (26 raw, minus 1 in sx/recipes.rs demo,
# minus 6 inside #[cfg(test)] modules). Raise this as adoption grows.
SX_FLOOR=19

SX_ROOT="src-tauri/src/ui_kit"

# Drop hits at/after the first `#[cfg(test)]` line of each file (same Rust
# tail-position convention check-design-system.sh and dev/quality_gate.py rely
# on), and skip the sx/recipes.rs demo file.
sx_count=$(
  { grep -rn --include="*.rs" "Sx::new(" "$SX_ROOT" 2>/dev/null || true; } \
  | grep -v 'sx/recipes\.rs' \
  | awk -F: '
      {
        f = $1; ln = $2 + 0
        if (!(f in cut)) {
          cut[f] = 0; n = 0
          while ((getline l < f) > 0) {
            n++
            if (l ~ /^[ \t]*#\[cfg\(test\)\]/) { cut[f] = n; break }
          }
          close(f)
        }
        if (cut[f] == 0 || ln < cut[f]) print
      }
    ' \
  | wc -l | tr -d ' '
)

# ── Report + verdicts ────────────────────────────────────────────────────────
echo "legacy_tone_ratchet:"
echo "  legacy color_alpha(theme.<tone>) sites : $legacy_count (ceiling $LEGACY_BASELINE — may only decrease)"
echo "  Sx::new() production sites in ui_kit   : $sx_count (floor $SX_FLOOR — may only increase)"

fail=0

if [ "$legacy_count" -gt "$LEGACY_BASELINE" ]; then
  echo "FAIL: $((legacy_count - LEGACY_BASELINE)) new raw color_alpha(theme/t.<tone>) site(s) added."
  echo "      Use tint(t, Tone::X, alpha) (gpu side) or palette_ct(t).base(Tone::X) (ui_kit)."
  echo "      Offending files:"
  grep -rlE "$LEGACY_PATTERN" $LEGACY_ROOTS 2>/dev/null | sed 's/^/        /'
  fail=1
elif [ "$legacy_count" -lt "$LEGACY_BASELINE" ]; then
  echo "RATCHET DOWN: $((LEGACY_BASELINE - legacy_count)) legacy site(s) migrated since baseline."
  echo "  -> lower LEGACY_BASELINE in scripts/sx_ratchet.sh to $legacy_count to lock it in."
fi

if [ "$sx_count" -lt "$SX_FLOOR" ]; then
  echo "FAIL: Sx adoption dropped below the floor ($sx_count < $SX_FLOOR)."
  echo "      Sx::new() sites were removed from ui_kit production code. Adoption"
  echo "      may only grow — restore the sites or migrate them to something"
  echo "      strictly better, then justify lowering SX_FLOOR in review."
  fail=1
elif [ "$sx_count" -gt "$SX_FLOOR" ]; then
  echo "ADOPTION UP: $((sx_count - SX_FLOOR)) new Sx::new() site(s) since capture."
  echo "  -> raise SX_FLOOR in scripts/sx_ratchet.sh to $sx_count to lock it in."
fi

if [ "$fail" -ne 0 ]; then
  exit 1
fi

echo "OK"
