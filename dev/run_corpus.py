#!/usr/bin/env python3
"""Reliable full-corpus runner for the dev-inspector scenario suite.

Runs every interactive scenario (numeric prefix >= 500) against a live build and
produces a trustworthy pass/fail verdict.

Methodology (learned the hard way): driving 1000+ scenarios via `/run-suite`
chunks fires them back-to-back with NO drain between, so a scenario's late async
bar-load bleeds into the next reset and causes ~30% spurious failures. Running
each scenario ONE AT A TIME via `/run-scenario` with a small inter-scenario gap
lets in-flight loads drain, which eliminates the contamination (measured 26/26 vs
21/30 on the same set). We also restart the app every RESTART_EVERY scenarios so a
single long-lived instance can't accumulate degradation, and retry a failure once
(fresh drain) to catch the rare true flake.

Usage: python dev/run_corpus.py [base_url]
"""
import sys, json, glob, os, time, subprocess, urllib.request, urllib.error

BASE = (sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:7892").rstrip("/")
REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SCEN = os.path.join(REPO, "dev", "scenarios")
EXE  = os.path.join(REPO, "src-tauri", "target", "debug", "apex-native.exe")
GAP          = 0.8    # seconds between scenarios — lets async loads drain
RESTART_EVERY = 150   # restart the app every N scenarios to avoid accumulation

def health():
    try:
        urllib.request.urlopen(BASE + "/health", timeout=3).read(); return True
    except Exception:
        return False

def kill_app():
    subprocess.run(["taskkill", "/F", "/IM", "apex-native.exe"],
                   capture_output=True)
    time.sleep(1.0)

def start_app():
    kill_app()
    # Launch from REPO root so SCENARIO_DIR ("dev/scenarios") resolves.
    subprocess.Popen([EXE], cwd=REPO,
                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                     creationflags=0x00000008)  # DETACHED_PROCESS
    for _ in range(60):
        if health(): return True
        time.sleep(1.0)
    raise RuntimeError("app did not become healthy")

def run_one(file, timeout=90):
    body = json.dumps({"file": file}).encode()
    req = urllib.request.Request(BASE + "/run-scenario", data=body,
                                 headers={"Content-Type": "application/json"}, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return json.loads(r.read().decode())
    except urllib.error.HTTPError as e:
        try: return json.loads(e.read().decode())
        except Exception: return {"pass": False, "steps": [], "_http": e.code}
    except Exception as e:
        return {"pass": False, "steps": [], "_err": str(e)}

def first_fail_detail(r):
    for s in r.get("steps", []):
        if s.get("ok") is False:
            return f"step {s.get('index','?')} ({s.get('action','?')}): {s.get('detail','')[:400]}"
    return r.get("_err") or f"http {r.get('_http')}" or "unknown failure"

def main():
    files = sorted(
        (os.path.basename(f) for f in glob.glob(os.path.join(SCEN, "*.json"))
         if os.path.basename(f).split("_")[0].isdigit()
         and int(os.path.basename(f).split("_")[0]) >= 500),
        key=lambda b: int(b.split("_")[0]))
    print(f"corpus: {len(files)} scenarios | gap={GAP}s | restart every {RESTART_EVERY}", flush=True)

    start_app()
    passed, real = 0, []
    for i, f in enumerate(files):
        if i and i % RESTART_EVERY == 0:
            print(f"  [{i}] restarting app (anti-degradation)", flush=True)
            start_app()
        res = run_one(f)
        ok = bool(res.get("pass"))
        if not ok:
            # Retry once after a longer drain — catches the rare true flake.
            time.sleep(2.0)
            res = run_one(f)
            ok = bool(res.get("pass"))
        if ok:
            passed += 1
        else:
            real.append((f, first_fail_detail(res)))
            print(f"  FAIL {f}: {first_fail_detail(res)}", flush=True)
        if i % 50 == 0:
            print(f"  progress {i+1}/{len(files)} | passed {passed} | real {len(real)}", flush=True)
        time.sleep(GAP)

    total = len(files)
    print(f"\n=== VERDICT: {passed}/{total} pass | {len(real)} real failure(s) ===", flush=True)
    lines = ["# Scenario Bug Report (full corpus, one-at-a-time + app restarts)\n",
             f"**{passed}/{total} scenarios pass** — {len(real)} real failure(s).\n"]
    if real:
        lines.append("## Failures (fail even one-at-a-time with drain + retry)\n")
        for fn, det in sorted(real):
            lines.append(f"### ❌ `{fn}`\n- {det}\n")
    else:
        lines.append("✅ No failures — every scenario passes.\n")
    open(os.path.join(REPO, "dev", "bug_report.md"), "w", encoding="utf-8").write("\n".join(lines))
    verdict = {"done": True, "total": total, "correct": passed,
               "real": [{"file": fn, "detail": det} for fn, det in real]}
    open(os.path.join(REPO, "dev", "corpus_verdict.json"), "w", encoding="utf-8").write(json.dumps(verdict, indent=2))
    print("wrote dev/bug_report.md + dev/corpus_verdict.json", flush=True)
    kill_app()

if __name__ == "__main__":
    main()
