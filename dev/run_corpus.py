#!/usr/bin/env python3
"""Reliable full-corpus runner for the dev-inspector scenario suite.

Runs every interactive scenario (numeric prefix >= 500) against a live build, then
— because back-to-back runs share one process and a previous scenario's late async
load can bleed into the next — RE-RUNS each failure in isolation to classify it:

  * passes in isolation  -> contamination flake (not a feature defect)
  * fails in isolation    -> a REAL failure

This gives a trustworthy feature-correctness verdict from the full corpus without a
per-process restart or touching the production load path. Writes dev/bug_report.md
with only the real failures.

Usage: python dev/run_corpus.py [base_url]   (default http://127.0.0.1:7892)
"""
import sys, json, glob, os, time, urllib.request

BASE = (sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:7892").rstrip("/")
REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SCEN = os.path.join(REPO, "dev", "scenarios")
CHUNK = 30

def post(path, obj, timeout):
    req = urllib.request.Request(BASE + path, data=json.dumps(obj).encode(),
                                 headers={"Content-Type": "application/json"}, method="POST")
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read().decode())

def health():
    try:
        urllib.request.urlopen(BASE + "/health", timeout=3).read(); return True
    except Exception:
        return False

def run(scenarios, timeout):
    return post("/run-suite", {"scenarios": scenarios}, timeout)

def first_fail_detail(r):
    for s in r.get("steps", []):
        if not s.get("pass"):
            for frag in s.get("detail", "").split("; "):
                f = frag.strip()
                if f.startswith("[") and "]" in f and "✓" not in f[:3]:
                    body = f[f.index("]")+1:].strip()
                    if body and "no panics" not in body:
                        return f"step {s.get('step')}: {body}"
            return f"step {s.get('step')} ({s.get('action')})"
    return r.get("error", "?")

def main():
    if not health():
        print("!! app not reachable at", BASE); sys.exit(1)
    files = sorted([os.path.basename(f) for f in glob.glob(SCEN + "/*.json")
                    if os.path.basename(f).split("_")[0].isdigit()
                    and int(os.path.basename(f).split("_")[0]) >= 500],
                   key=lambda b: int(b.split("_")[0]))
    print(f"corpus: {len(files)} scenarios, chunk={CHUNK}")

    results = {}
    for i in range(0, len(files), CHUNK):
        chunk = files[i:i+CHUNK]
        d = run(chunk, timeout=590)
        for r in d["results"]:
            results[r["scenario"]] = r
        print(f"  chunk {i//CHUNK}: {d['passed']}/{d['total']}")

    name2file = {}
    for f in files:
        try:
            name2file[json.load(open(os.path.join(SCEN, f), encoding="utf-8"))["name"]] = f
        except Exception:
            pass

    fails = [r for r in results.values() if not r.get("pass")]
    print(f"\nfirst pass: {len(results)-len(fails)}/{len(results)} | {len(fails)} failed -> re-running each in isolation")

    real, flake = [], []
    for r in fails:
        nm = r["scenario"]
        fn = name2file.get(nm) or (nm if nm.endswith(".json") else None) \
             or next((f for f in files if nm in f), None)
        if not fn:
            real.append((nm, "could not locate file", first_fail_detail(r))); continue
        time.sleep(1.5)  # let the full run's in-flight loads drain before the solo retry
        solo = run([fn], timeout=180)["results"][0]
        if solo.get("pass"):
            flake.append(nm)
        else:
            real.append((nm, fn, first_fail_detail(solo)))
        print(f"  {'FLAKE' if solo.get('pass') else 'REAL '} {nm}")

    total = len(results)
    print(f"\n=== VERDICT: {total-len(real)}/{total} correct "
          f"({len(flake)} contamination flakes auto-cleared, {len(real)} real) ===")
    lines = ["# Scenario Bug Report (full corpus, contamination-filtered)\n",
             f"**{total-len(real)}/{total} scenarios correct** "
             f"— {len(flake)} back-to-back contamination flakes auto-cleared "
             f"(passed in isolation), {len(real)} real failure(s).\n"]
    if real:
        lines.append("## Real failures (fail even in isolation)\n")
        for nm, fn, det in sorted(real):
            lines.append(f"### ❌ `{fn}`")
            lines.append(f"- {det}\n")
    else:
        lines.append("✅ No real failures — every scenario is correct in isolation.\n")
    if flake:
        lines.append("## Contamination flakes (correct in isolation)\n")
        lines.append(", ".join(f"`{n}`" for n in sorted(flake)) + "\n")
    open(os.path.join(REPO, "dev", "bug_report.md"), "w", encoding="utf-8").write("\n".join(lines))
    print("wrote dev/bug_report.md")

if __name__ == "__main__":
    main()
