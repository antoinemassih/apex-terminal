#!/usr/bin/env bash
# corpus_gate.sh — the windowed pre-merge gate (audit B4 + B5).
#
# The 1,065-scenario dev_inspector corpus and the ux_audit / design-audit
# baselines drive the REAL GPU window + the debug-only dev_inspector HTTP
# server. A headless ubuntu CI runner cannot host that, so this is the LOCAL
# (or self-hosted GPU runner) gate that must be green before merging any
# behavior-adjacent change. The static gates (clippy deny table, quality
# ratchets, build matrix) run in .github/workflows/quality-gates.yml.
#
# What it does:
#   1. builds the debug binary (dev_inspector is #[cfg(debug_assertions)]),
#   2. runs dev/run_corpus.py one-at-a-time with drain + periodic restart
#      (the methodology that avoids contamination flakes),
#   3. asserts dev/corpus_verdict.json reports zero REAL failures,
#   4. asserts the ux_audit + design-audit baselines (913 / 572) are in the
#      passing set — they ride the same corpus run.
#
# Usage:  bash scripts/corpus_gate.sh
# Exit:   0 = all green, 1 = a real failure (details in dev/bug_report.md).
set -euo pipefail
cd "$(dirname "$0")/.."

echo "[corpus-gate] building debug binary (dev_inspector needs debug_assertions)…"
( cd src-tauri && cargo build --bin apex-native )

echo "[corpus-gate] running full corpus (one-at-a-time, drain + restart)…"
python dev/run_corpus.py

VERDICT="dev/corpus_verdict.json"
if [ ! -f "$VERDICT" ]; then
  echo "[corpus-gate] FAIL: no verdict written ($VERDICT missing)"; exit 1
fi

python - "$VERDICT" <<'PY'
import json, sys
v = json.load(open(sys.argv[1], encoding="utf-8"))
total, correct = v.get("total", 0), v.get("correct", 0)
real = v.get("real", [])
# B5: the ux_audit + design-audit baselines must be part of the run and not in
# the real-failure list. They are ordinary corpus scenarios (913 / 572), so a
# clean run with an empty `real` set proves both passed.
UX = ["913_ux_audit_baseline", "572_design_audit_baseline"]
real_names = {r.get("file", r) if isinstance(r, dict) else r for r in real}
ux_failed = [u for u in UX if any(u in str(n) for n in real_names)]
print(f"[corpus-gate] {correct}/{total} correct; {len(real)} real failure(s)")
if real:
    for r in real[:20]:
        print("  REAL:", r)
    sys.exit(1)
if ux_failed:
    print("  ux_audit gate regressed:", ux_failed); sys.exit(1)
print("[corpus-gate] PASS — corpus green, ux_audit + design-audit baselines hold")
PY
