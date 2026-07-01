#!/usr/bin/env python3
"""Run a stratified sample of scenarios (one representative per NEW family)
against the live app via /run-scenario. Prints pass/fail + failing assertions.
"""
import json, sys, urllib.request, glob, os

BASE = "http://127.0.0.1:7892"
SCEN = os.path.join(os.path.dirname(__file__), "scenarios")

def run(file):
    data = json.dumps({"file": file}).encode()
    req = urllib.request.Request(BASE + "/run-scenario", data=data, method="POST",
                                 headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=120) as r:
            return json.loads(r.read().decode())
    except urllib.error.HTTPError as e:
        # 422 = scenario ran but failed assertions; body still has the report.
        body = e.read().decode()
        try: return json.loads(body)
        except Exception: return {"passed": False, "steps": [], "_http": e.code, "_body": body[:300]}

# Representative sample: first file of each new family + a few legacy.
picks = []
def first(prefixglob):
    fs = sorted(os.path.basename(p) for p in glob.glob(os.path.join(SCEN, prefixglob)))
    return fs[0] if fs else None

for g in ["1000_*","1040_*","1100_*","1140_*","1200_*","1270_*","1300_*","1330_*",
          "1400_*","1420_*","1500_*","1530_*","1600_*",
          "1700_*","1701_*","1702_*","1703_*","1704_*","1705_*"]:
    f = first(g)
    if f: picks.append(f)

print(f"validating {len(picks)} representative scenarios...\n")
fails = []
for f in picks:
    try:
        res = run(f)
    except Exception as e:
        print(f"  [ERROR] {f}: {e}"); fails.append(f); continue
    ok = res.get("passed", res.get("pass"))
    steps = res.get("steps", [])
    nbad = sum(1 for s in steps if s.get("ok") is False)
    tag = "PASS" if ok else "FAIL"
    print(f"  [{tag}] {f}   steps={len(steps)} failed_steps={nbad}")
    if not ok:
        fails.append(f)
        for s in steps:
            if s.get("ok") is False:
                det = s.get("detail") or s.get("message") or ""
                print(f"        - {det[:200]}")

print()
if fails:
    print(f"SAMPLE: {len(fails)}/{len(picks)} families FAILED: {fails}")
    sys.exit(1)
print(f"SAMPLE PASSED: all {len(picks)} representative scenarios pass.")
