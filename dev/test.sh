#!/usr/bin/env bash
# Dev Inspector smoke test — runs all scenarios against a headless build.
# Usage:  ./dev/test.sh [binary_path]
#
# Requires the app to be built first:  cargo build
# Scenario files live in dev/scenarios/*.json

set -euo pipefail

BINARY="${1:-./target/debug/apex-terminal-native}"
API="http://127.0.0.1:7891"
TIMEOUT=30   # seconds to wait for server to come up

echo "━━━ Apex Terminal Dev Inspector Smoke Test ━━━"
echo "Binary: $BINARY"

# Build if binary doesn't exist
if [[ ! -f "$BINARY" ]]; then
    echo "[build] Binary not found, running cargo build..."
    cargo build 2>&1
fi

# Launch headless
echo "[launch] Starting headless binary..."
"$BINARY" --headless &
APP_PID=$!
trap "kill $APP_PID 2>/dev/null; exit 0" EXIT INT TERM

# Wait for the inspector server to come up
echo "[wait] Polling $API/health (timeout ${TIMEOUT}s)..."
for i in $(seq 1 $TIMEOUT); do
    if curl -sf "$API/health" > /dev/null 2>&1; then
        echo "[ok] Server ready after ${i}s"
        break
    fi
    sleep 1
    if [[ $i -eq $TIMEOUT ]]; then
        echo "[fail] Timed out waiting for $API/health"
        exit 1
    fi
done

# Run each scenario file
PASS=0
FAIL=0
for scenario in dev/scenarios/*.json; do
    name=$(basename "$scenario")
    result=$(curl -sf -X POST "$API/run-scenario" \
        -H "Content-Type: application/json" \
        -d "{\"file\":\"$name\"}" 2>&1 || echo '{"pass":false,"error":"curl failed"}')
    if echo "$result" | grep -q '"pass":true'; then
        echo "[PASS] $name"
        PASS=$((PASS+1))
    else
        echo "[FAIL] $name — $result"
        FAIL=$((FAIL+1))
    fi
done

echo ""
echo "Results: $PASS passed, $FAIL failed"
[[ $FAIL -eq 0 ]] && exit 0 || exit 1
