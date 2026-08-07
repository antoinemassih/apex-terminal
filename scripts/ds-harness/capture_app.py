#!/usr/bin/env python3
"""capture_app.py — screenshot the RUNNING apex-terminal app via its dev_inspector.

This script does NOT build or launch the app. The orchestrator owns build/run.
It talks to the dev_inspector HTTP harness of an already-running debug build.

════════════════════════════════════════════════════════════════════════════
DISCOVERED PROTOCOL (source: src-tauri/src/dev_inspector/server.rs + mod.rs,
src-tauri/src/native_main.rs, src-tauri/src/chart/renderer/gpu.rs ~L7630)
════════════════════════════════════════════════════════════════════════════

ENABLE MECHANISM
  - The dev_inspector is compiled ONLY into debug builds:
    native_main.rs wraps `dev_inspector::init()` in `#[cfg(debug_assertions)]`.
    There is no env var / feature flag to enable it — a debug build always
    starts it; a release build never has it.
  - Optional `--headless` CLI flag on the app makes the window invisible
    (dev_inspector::set_headless). For screenshots you want a VISIBLE window:
    the capture path BitBlts the desktop DC for the app HWND (gpu.rs pops the
    window AlwaysOnTop first so overlapping windows don't leak in).

PORT
  - Default 7892 (NOT 7891 — the file header comment in server.rs is stale).
    fn inspector_port(): env `APEX_DEV_INSPECTOR_PORT` overrides (shared with
    supermodel's harness on this machine; collision => bind retry loop).
  - Binds 127.0.0.1 only. Requests carrying an `Origin` header are REJECTED
    (anti-browser-CSRF); urllib sends none, so we're fine.

COMMANDS (POST /cmd, JSON body, response {"ok":true} or 400 {"error":...})
  - Set color scheme (palette):  {"cmd":"SetThemeIdx","idx":N,"pane":0}
      -> AppCommand::SetThemeIdx { pane, idx }   (pane defaults to 0)
  - Set style system:            {"cmd":"SetStyleIdx","idx":N}
      -> AppCommand::SetStyleIdx { idx }
  - Toggle UI debug overlay:     {"cmd":"SetUiDebug","on":true|false}
  - Resize window/viewport:      {"cmd":"SetViewportSize","w":1440,"h":900}
      (h optional; width-only sweeps supported)
  - snake_case aliases accepted: set_theme_idx / set_style_idx / set_ui_debug /
    set_viewport_size / resize.
  - There is also POST /set-style-and-audit {"idx":N} which sets style_idx AND
    returns the design-audit JSON in one round trip.

SCREENSHOT (POST /screenshot, body {"name":"foo"})
  - Does NOT return PNG bytes. The HTTP thread queues the request; the RENDER
    thread (which owns the HWND) fulfils it, writing
        <app CWD>/dev/screenshots/<name>.png
    The app runs with CWD = repo root, so on this machine that is
        C:/Users/USER/Documents/development/apex-terminal/dev/screenshots/
  - The handler waits ~4 frames before replying
    {"ok":true,"path":"dev/screenshots/<name>.png"} — the PNG is normally on
    disk by then, but we still poll for the file to be safe.
  - `name` must pass safe_rel(): no "..", no absolute paths, no drive letters.

PROBE ENDPOINTS
  - GET /health -> {"status":"ok","port":7892}
  - GET /state  -> full app_state JSON (symbols, panes, watchlist, ...)

INDEX MAPS (src-tauri/src/design_system/builtin.rs, order of appearance —
verify against the running app if builtin.rs has changed):
  theme_idx (ColorScheme / palette), builtin_color_schemes():
     0 midnight   1 nord      2 monokai    3 solarized  4 dracula
     5 gruvbox    6 catppuccin 7 tokyo-night 8 kanagawa  9 everforest
    10 vesper    11 rose-pine 12 bauhaus   13 peach     14 ivory
    15 newsprint 16 aperture  17 cadence   18 alto      19 mariner  20 lucid
  style_idx (StyleSystem), builtin_style_systems():
     0 meridien   1 aperture  2 octave     3 cadence    4 alto
     5 mariner    6 lucid     7 relay      8 glass
════════════════════════════════════════════════════════════════════════════

Usage:
    python capture_app.py --probe                      # GET /health + /state
    python capture_app.py                              # default DS sweep
    python capture_app.py --pairs 16:1,17:3,18:4,19:5,20:6
    python capture_app.py --pairs aperture:aperture,lucid:lucid
    python capture_app.py --port 7900 --size 1440x900 --out <dir>

--pairs takes comma-separated theme:style entries; each side is an integer
index or a name from the maps above. Output files:

    docs/styling/screenshots/current/<theme>-<style>-<w>x<h>.png

(<w>x<h> is read from the actual captured PNG, since capture size = live
window size; pass --size to ask the app to resize first via SetViewportSize.)

Stdlib only (urllib); Pillow not required (PNG dims read from the IHDR header).
"""

import argparse
import json
import os
import shutil
import struct
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]           # .../apex-terminal
SHOT_DIR = REPO_ROOT / "dev" / "screenshots"              # app writes here (CWD = repo root)
DEFAULT_OUT = REPO_ROOT / "docs" / "styling" / "screenshots" / "current"
DEFAULT_PORT = int(os.environ.get("APEX_DEV_INSPECTOR_PORT", "7892"))

THEME_NAMES = [
    "midnight", "nord", "monokai", "solarized", "dracula", "gruvbox",
    "catppuccin", "tokyo-night", "kanagawa", "everforest", "vesper",
    "rose-pine", "bauhaus", "peach", "ivory", "newsprint",
    "aperture", "cadence", "alto", "mariner", "lucid",
]
STYLE_NAMES = [
    "meridien", "aperture", "octave", "cadence", "alto",
    "mariner", "lucid", "relay", "glass",
]

def live_axis_names(port, fallback_themes, fallback_styles):
    """Ask the APP what colour schemes and styles it has, in index order.

    The hardcoded lists below are a fallback for when the app is not up yet
    (`--list`, unit-ish uses). They are NOT authoritative and they drift:
    THEME_NAMES sat at 21 entries while the app shipped 22, so `--preset
    meridien:meridien` died with "unknown theme 'meridien'" for a scheme that
    had been at index 21 all along.
    """
    try:
        import urllib.request, json
        with urllib.request.urlopen(f"http://127.0.0.1:{port}/state", timeout=3) as r:
            st = json.load(r)
        themes = [str(x) for x in st.get("theme_names") or []]
        styles = [str(x) for x in st.get("style_names") or []]
        if themes and styles:
            return themes, styles
        print("  ! /state has no theme_names/style_names — using the stale local copy")
    except Exception as e:
        print(f"  ! could not read live axis names ({e}) — using the stale local copy")
    return fallback_themes, fallback_styles

# Default sweep: the six DS palettes x their matching style systems
# (aperture/cadence/alto/mariner/lucid have both a palette and a style;
# meridien exists only as a style — paired with the aperture palette here).
DEFAULT_PAIRS = "16:1,17:3,18:4,19:5,20:6,16:0"


def http(method: str, port: int, path: str, body=None, timeout=15.0):
    url = f"http://127.0.0.1:{port}{path}"
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(url, data=data, method=method,
                                 headers={"Content-Type": "application/json"})
    # NB: we deliberately send no Origin header — the server rejects any
    # request that has one (browser-CSRF guard).
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.status, json.loads(r.read().decode() or "null")


def cmd(port: int, body: dict):
    status, resp = http("POST", port, "/cmd", body)
    if status != 200 or not (isinstance(resp, dict) and resp.get("ok")):
        raise RuntimeError(f"/cmd {body} -> {status} {resp}")
    return resp


def png_size(path: Path):
    """Width/height from the PNG IHDR chunk (bytes 16..24) — no Pillow needed."""
    with open(path, "rb") as f:
        head = f.read(24)
    if len(head) < 24 or head[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError(f"not a PNG: {path}")
    w, h = struct.unpack(">II", head[16:24])
    return w, h


def _axis_key(name: str) -> str:
    """Normalise an axis name so slug and display forms both resolve.

    The live lists come from the app's DISPLAY names ("tokyo night",
    "rose pine"); presets have always been written as slugs ("tokyo-night",
    "rose-pine"). Without this, switching the harness to the live lists would
    have silently broken every existing preset that names one of those two.
    """
    import unicodedata
    n = unicodedata.normalize("NFKD", name.strip().lower())
    n = "".join(c for c in n if not unicodedata.combining(c))
    for ch in (" ", "_", "."):
        n = n.replace(ch, "-")
    return n


def resolve(token: str, names: list[str], what: str) -> tuple[int, str]:
    token = token.strip().lower()
    if token.isdigit():
        idx = int(token)
        name = names[idx] if idx < len(names) else f"{what}{idx}"
        return idx, name
    keys = [_axis_key(n) for n in names]
    key = _axis_key(token)
    if key in keys:
        i = keys.index(key)
        return i, names[i]
    raise SystemExit(f"unknown {what} '{token}' (use an index or one of {names})")


def probe(port: int):
    for path in ("/health", "/state"):
        try:
            status, resp = http("GET", port, path)
            text = json.dumps(resp, indent=2)
            if path == "/state" and len(text) > 4000:
                keys = list(resp.keys()) if isinstance(resp, dict) else "?"
                print(f"GET {path} -> {status}; top-level keys: {keys}")
            else:
                print(f"GET {path} -> {status}\n{text}")
        except (urllib.error.URLError, OSError) as e:
            print(f"GET {path} -> UNREACHABLE ({e})")
            print(f"Is a DEBUG build of apex-native running? (port {port}; "
                  "release builds have no dev_inspector)")
            return 1
    return 0


def capture_pair(port: int, theme_idx: int, theme_name: str,
                 style_idx: int, style_name: str, out_dir: Path,
                 settle_ms: int) -> Path:
    cmd(port, {"cmd": "SetThemeIdx", "idx": theme_idx, "pane": 0})
    cmd(port, {"cmd": "SetStyleIdx", "idx": style_idx})

    # VERIFY the switch landed before shooting — do not just sleep.
    #
    # These are QUEUED commands: the HTTP call returns "queued", not "applied".
    # A blind `sleep(settle_ms)` assumes the app drains and repaints in that
    # window, which is false under load (a cargo build running, another app
    # instance alive). The failure is silent and pernicious: the screenshot
    # shows the PREVIOUS theme but is written under the NEW theme's filename,
    # so the reference set gets a PNG named `aperture-aperture` containing
    # Meridien's cream palette and every later comparison is against a lie.
    #
    # `/state` now reports `active_theme_idx` / `active_style_idx`, so poll
    # until the app has actually converged, then settle for the repaint.
    deadline = time.time() + 10.0
    seen = None
    while time.time() < deadline:
        try:
            _, st = http("GET", port, "/state", None, timeout=5.0)
            seen = (st.get("active_theme_idx"), st.get("active_style_idx"))
            if seen == (theme_idx, style_idx):
                break
        except Exception:
            pass                       # transient during a repaint; keep polling
        time.sleep(0.1)
    else:
        raise RuntimeError(
            f"theme/style never converged: asked for {(theme_idx, style_idx)}, "
            f"app reports {seen}. Refusing to write a mislabelled capture."
        )

    time.sleep(settle_ms / 1000.0)     # let repaint settle beyond the 1-frame wait

    shot_name = f"ds_{theme_name}_{style_name}".replace("-", "_")
    src = SHOT_DIR / f"{shot_name}.png"
    if src.exists():
        src.unlink()                   # stale file from an earlier run

    status, resp = http("POST", port, "/screenshot", {"name": shot_name}, timeout=20.0)
    if status != 200:
        raise RuntimeError(f"/screenshot -> {status} {resp}")

    # Server waits ~4 frames before replying, but the PNG write happens on the
    # render thread — poll briefly in case of a race.
    deadline = time.time() + 5.0
    while not src.exists() and time.time() < deadline:
        time.sleep(0.1)
    if not src.exists():
        raise RuntimeError(f"screenshot file never appeared: {src}")
    time.sleep(0.15)                   # let the write finish

    w, h = png_size(src)
    dest = out_dir / f"{theme_name}-{style_name}-{w}x{h}.png"
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src, dest)
    return dest


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--port", type=int, default=DEFAULT_PORT,
                    help=f"dev_inspector port (default {DEFAULT_PORT}; "
                         "env APEX_DEV_INSPECTOR_PORT)")
    ap.add_argument("--pairs", default=DEFAULT_PAIRS,
                    help="comma-separated theme:style entries (index or name), "
                         f'default "{DEFAULT_PAIRS}"')
    ap.add_argument("--out", type=Path, default=DEFAULT_OUT)
    ap.add_argument("--size", default=None,
                    help='optional "WxH" — sends SetViewportSize before the sweep')
    ap.add_argument("--settle-ms", type=int, default=600,
                    help="extra settle after theme/style switch (default 600)")
    ap.add_argument("--probe", action="store_true",
                    help="just GET /health and /state and print the result")
    args = ap.parse_args()
    args.out = Path(args.out).resolve()  # M0 gate fix: relative --out broke relative_to()

    if args.probe:
        sys.exit(probe(args.port))

    # Reachability check first — clearer error than a mid-sweep failure.
    try:
        http("GET", args.port, "/health", timeout=5.0)
    except (urllib.error.URLError, OSError) as e:
        print(f"ERROR: dev_inspector unreachable on 127.0.0.1:{args.port} ({e})",
              file=sys.stderr)
        print("The app must be a running DEBUG build (dev_inspector is "
              "#[cfg(debug_assertions)]).", file=sys.stderr)
        sys.exit(1)

    if args.size:
        w, _, h = args.size.partition("x")
        cmd(args.port, {"cmd": "SetViewportSize", "w": float(w), "h": float(h)})
        time.sleep(0.5)

    pairs = []
    for entry in args.pairs.split(","):
        t, _, s = entry.strip().partition(":")
        if not s:
            raise SystemExit(f"bad pair '{entry}' — expected theme:style")
        themes, styles = live_axis_names(args.port, THEME_NAMES, STYLE_NAMES)
        pairs.append((resolve(t, themes, "theme"),
                      resolve(s, styles, "style")))

    failures = 0
    for (t_idx, t_name), (s_idx, s_name) in pairs:
        try:
            dest = capture_pair(args.port, t_idx, t_name, s_idx, s_name,
                                args.out, args.settle_ms)
            print(f"  -> {dest.relative_to(REPO_ROOT)}  "
                  f"({dest.stat().st_size:,} bytes)")
        except Exception as e:
            failures += 1
            print(f"  !! theme={t_name}({t_idx}) style={s_name}({s_idx}): {e}",
                  file=sys.stderr)

    print(f"\nDone: {len(pairs) - failures}/{len(pairs)} captured -> {args.out}")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
