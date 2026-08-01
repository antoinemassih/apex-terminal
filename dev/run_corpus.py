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

# Windows consoles default to cp1252, which cannot encode the check/cross marks
# that scenario assertion messages embed. Printing ONE such failure detail raised
# UnicodeEncodeError and killed the whole runner at scenario 2 of 1068 — i.e. a
# cosmetic encoding issue aborted the entire gate and produced no verdict.
# Reconfigure to UTF-8 and never let an un-encodable glyph be fatal.
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass

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
# SESSION-UNIQUE protected-copy name (2026-07-27). The old shared name
# `apex-native-corpus.exe` meant a co-tenant session starting ITS corpus ran
# `taskkill /F /IM apex-native-corpus.exe` and killed MY running app too — my
# run then got a cascade of WinError 10061 "connection refused" from that
# scenario onward. The port fix (APEX_CORPUS_PORT) solved the bind collision but
# NOT this taskkill collision, because taskkill matches by process NAME, not
# port. Tagging the exe per session (driver PID, overridable via APEX_CORPUS_TAG)
# makes each session's app invisible to the other's name-based taskkill, so two
# corpus runs can coexist on this shared machine.
_SESSION_TAG = os.environ.get("APEX_CORPUS_TAG") or f"s{os.getpid()}"
_PROT  = os.path.join(_EXE_DIR, f"apex-native-corpus-{_SESSION_TAG}.exe")
_PROT_NAME = os.path.basename(_PROT)

def _refresh_protected_copy():
    if not os.path.exists(_CANON):
        return _PROT if os.path.exists(_PROT) else _CANON
    if (not os.path.exists(_PROT)) or os.path.getmtime(_CANON) > os.path.getmtime(_PROT):
        import shutil
        # Only our OWN session-tagged copy — never the bare shared name (that
        # would be a co-tenant's app).
        subprocess.run(["taskkill", "/F", "/IM", _PROT_NAME], capture_output=True)
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
    # Kill ONLY our own session-tagged copy — never the bare shared
    # `apex-native-corpus.exe` (a co-tenant's corpus app) nor `apex-native.exe`
    # (the user's manually-running dev app). This is what lets two corpus runs
    # coexist on one machine; we run on our own APEX_CORPUS_PORT so there's no
    # port conflict to reclaim. Harmless if our name isn't running.
    #
    # GOTCHA (2026-07-29): a flat sleep(1.0) after taskkill is NOT enough. When
    # the app has crashed/degraded, its process lingers as a GPU-driver-held
    # zombie (Windows: process object still enumerable, RAM + inspector port
    # still held) for several seconds AFTER taskkill reports success. Respawning
    # into that window binds nothing — the new instance can't take the port and
    # every subsequent scenario fails with connection-refused, cascading a whole
    # run into a false red. Fix: POLL until no same-named process remains (up to
    # ~24s) so the driver has time to release the handle before we respawn.
    # GOTCHA (2026-08-01): `taskkill /F` DOES NOT KILL THIS APP. It reports
    # "no running instance" (and Stop-Process reports "cannot find a process
    # with that identifier") while tasklist still enumerates the process — so
    # the old loop below "succeeded" instantly, we respawned into a live
    # instance still holding the port, and every scenario came back
    # connection-refused. That reads as a mass test failure but is the harness
    # leaking processes.
    #
    # .NET Process.Kill($true) DOES reap them. Use PowerShell for the kill and
    # keep tasklist only for the wait loop.
    _ps_kill = (
        f"Get-Process -ErrorAction SilentlyContinue "
        f"| Where-Object {{ $_.ProcessName -eq '{_PROT_NAME[:-4]}' }} "
        f"| ForEach-Object {{ try {{ $_.Kill($true) }} catch {{}} }}"
    )
    subprocess.run(["powershell", "-NoProfile", "-Command", _ps_kill], capture_output=True)
    for _ in range(24):
        out = subprocess.run(["tasklist", "/FI", f"IMAGENAME eq {_PROT_NAME}", "/NH"],
                             capture_output=True, text=True).stdout
        if _PROT_NAME.lower() not in out.lower():
            time.sleep(0.5)  # small grace for port teardown after the handle drops
            return
        subprocess.run(["powershell", "-NoProfile", "-Command", _ps_kill], capture_output=True)
        time.sleep(1.0)
    # Fell through — a stubborn zombie. Leave a breadcrumb; start_app's health
    # loop will surface the failure loudly rather than silently cascading.
    print(f"corpus: WARNING {_PROT_NAME} still present after 24s of kill attempts", flush=True)

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
    # CREATE_NO_WINDOW rather than DETACHED_PROCESS.
    #
    # UNRESOLVED (2026-08-01) — read this before trusting a red corpus run.
    # The app's dev-inspector binds its port successfully ("HTTP server on
    # 127.0.0.1:PORT") and its accept loop is then killed by
    # WSACancelBlockingCall (10004); every later accept fails "WSAStartup
    # failed" (10093). The driver reports connection-refused for all 1067
    # scenarios, which LOOKS like a mass test failure but is the app losing
    # winsock, not the scenarios failing. Launching the same exe from
    # PowerShell (Start-Process -RedirectStandardOutput) never shows this.
    #
    # Ruled OUT by experiment: stale/leftover processes (verified none held the
    # port or the log), a stale exe copy (the copy is byte-fresh), the chosen
    # port, and DETACHED_PROCESS itself — switching to CREATE_NO_WINDOW did NOT
    # fix it. Remaining suspects: the stdout/stderr file handle Popen inherits,
    # or something about process creation from Python specifically.
    #
    # CREATE_NO_WINDOW is kept because it is strictly better than
    # DETACHED_PROCESS for cleanup (the child stays in the parent's job/console
    # tree so kills propagate), not because it fixed the bug.
    subprocess.Popen([EXE], cwd=REPO, env=_env,
                     stdout=_applog, stderr=_applog,
                     creationflags=0x08000000)  # CREATE_NO_WINDOW
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
