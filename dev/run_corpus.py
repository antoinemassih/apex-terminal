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

# Dev-inspector port. Default 7892, overridable via APEX_CORPUS_PORT — 7892 is
# shared with supermodel's harness on this machine, so when a co-tenant holds it
# the apex app can never bind (its bind retries forever) and start_app times out
# with "app did not become healthy". Point both the launched app (via
# APEX_DEV_INSPECTOR_PORT) and this driver at the same chosen port.
CORPUS_PORT = int(os.environ.get("APEX_CORPUS_PORT", "7892"))
BASE = (sys.argv[1] if len(sys.argv) > 1 else f"http://127.0.0.1:{CORPUS_PORT}").rstrip("/")
REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SCEN = os.path.join(REPO, "dev", "scenarios")
_EXE_DIR = os.path.join(REPO, "src-tauri", "target", "debug")
# Prefer a protected-name copy if one exists (`cp apex-native.exe
# apex-native-corpus.exe` lets a run survive another session's
# `taskkill /IM apex-native.exe` when the repo is shared on one machine); fall
# back to the canonical binary so a clean checkout / CI works with no copy step.
#
# GOTCHA (2026-07-17, cost a full run): the protected copy was NEVER refreshed,
# so once it existed every corpus run silently certified a STALE binary — a
# green 1067/1067 that says nothing about the code you just built. The copy is
# now refreshed from the canonical exe whenever the canonical one is newer, so
# "protected from taskkill" no longer means "frozen in time".
_CANON = os.path.join(_EXE_DIR, "apex-native.exe")
_PROT  = os.path.join(_EXE_DIR, "apex-native-corpus.exe")

def _refresh_protected_copy():
    if not os.path.exists(_CANON):
        return _PROT if os.path.exists(_PROT) else _CANON
    if (not os.path.exists(_PROT)) or os.path.getmtime(_CANON) > os.path.getmtime(_PROT):
        import shutil
        subprocess.run(["taskkill", "/F", "/IM", "apex-native-corpus.exe"], capture_output=True)
        time.sleep(1.0)
        try:
            shutil.copy2(_CANON, _PROT)
            print(f"corpus: refreshed {os.path.basename(_PROT)} from freshly built exe", flush=True)
        except OSError as e:
            print(f"corpus: WARNING could not refresh protected copy ({e}); "
                  f"falling back to canonical exe", flush=True)
            return _CANON
    return _PROT

EXE = _refresh_protected_copy()
GAP          = 0.8    # seconds between scenarios — lets async loads drain
RESTART_EVERY = 150   # restart the app every N scenarios to avoid accumulation

def health():
    try:
        urllib.request.urlopen(BASE + "/health", timeout=3).read(); return True
    except Exception:
        return False

def kill_app():
    # Kill both the protected-name copy and the canonical binary, so a run
    # cleanly restarts its own app and reclaims :7892 regardless of which name
    # is in use. Harmless if a name isn't running.
    subprocess.run(["taskkill", "/F", "/IM", "apex-native-corpus.exe"], capture_output=True)
    subprocess.run(["taskkill", "/F", "/IM", "apex-native.exe"], capture_output=True)
    time.sleep(1.0)

def start_app():
    kill_app()
    # Launch from REPO root so SCENARIO_DIR ("dev/scenarios") resolves.
    # DEBUG (2026-07-17): app stdout/stderr used to go to DEVNULL, so an app
    # crash mid-corpus left NO trace and the run just stalled. Capture it.
    _applog = open(os.path.join(REPO, "dev", "corpus_app.log"), "ab", buffering=0)
    # Launch the app on the chosen inspector port (see CORPUS_PORT) so a
    # supermodel process holding 7892 doesn't block the run.
    _env = dict(os.environ)
    _env["APEX_DEV_INSPECTOR_PORT"] = str(CORPUS_PORT)
    subprocess.Popen([EXE], cwd=REPO, env=_env,
                     stdout=_applog, stderr=_applog,
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
    # GOTCHA (fixed 2026-07-17): the server emits each step as
    # {"step": N, "pass": bool, ...} — this read `ok` / `index`, which NEVER
    # exist, so every real failure fell through to the useless "http None"
    # instead of the actual assertion text. The bug report has been hiding the
    # one thing it exists to show. Accept both spellings.
    for s in r.get("steps", []):
        if s.get("pass") is False or s.get("ok") is False:
            idx = s.get("step", s.get("index", "?"))
            return f"step {idx} ({s.get('action','?')}): {str(s.get('detail',''))[:400]}"
    if r.get("_err"):
        return r["_err"]
    if r.get("_http") is not None:
        return f"http {r['_http']}"
    return "unknown failure"

def competing_load():
    """Processes that make this machine untrustworthy for a corpus run.

    WHY THIS EXISTS (2026-07-19, cost most of an afternoon): this repo is shared
    with other Claude sessions on ONE machine. A concurrent `cargo build
    --release` (LTO, codegen-units=1) saturates the CPU, the app can no longer
    drain an async bar-load inside the `GAP` window, and the NEXT scenario
    renders with the PREVIOUS scenario's price range still applied — bars land
    ~30x outside a sane viewport and the run reports dozens of
    "N out-of-bounds bar(s)" failures that look exactly like a rendering
    regression in whatever you just committed.

    It is not a regression, and the tell is that it is not reproducible: the
    same binary over the same scenario order passed 1..401 with zero failures on
    one run and failed ~77 of those same scenarios on the next. Hours went into
    bisecting code that was never broken.

    A corpus run is a certification. Certifying under contention produces a
    verdict that means nothing in EITHER direction — a red that indicts innocent
    code, or a green that was luck. So refuse up front instead.
    """
    hits = []
    try:
        out = subprocess.run(
            ["tasklist", "/FO", "CSV", "/NH"], capture_output=True, text=True, timeout=20
        ).stdout
    except Exception:
        return hits  # never let the guard itself break a run
    for line in out.splitlines():
        name = line.split('","')[0].lstrip('"').lower()
        if name in ("cargo.exe", "rustc.exe", "link.exe"):
            hits.append(name)
    return hits


def preflight():
    """Refuse to certify on a contended machine unless explicitly overridden."""
    hits = competing_load()
    if not hits:
        return
    from collections import Counter
    summary = ", ".join(f"{n}x{c}" for n, c in Counter(hits).items())
    print(f"corpus: REFUSING TO RUN — competing build detected ({summary}).", flush=True)
    print("corpus: a concurrent cargo build starves the app and produces "
          "out-of-bounds-bar failures that are NOT code regressions.", flush=True)
    print("corpus: wait for the build to finish, or set APEX_CORPUS_ALLOW_CONTENTION=1 "
          "to run anyway (verdict will be untrustworthy).", flush=True)
    if os.environ.get("APEX_CORPUS_ALLOW_CONTENTION") not in ("1", "true", "True"):
        sys.exit(2)
    print("corpus: WARNING — running under contention by explicit override; "
          "a red verdict here does NOT indict your code.", flush=True)


def main():
    preflight()
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
