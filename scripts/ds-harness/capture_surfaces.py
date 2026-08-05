#!/usr/bin/env python3
"""capture_surfaces.py — screenshot each APP SURFACE for a visual audit.

Sister to `capture_app.py`, which sweeps THEME x STYLE on one screen. This one
holds the theme fixed and sweeps the SCREENS: watchlist tabs, tool panels,
trading surfaces, modals.

════════════════════════════════════════════════════════════════════════════
WHY EVERY SURFACE CARRIES A `require` BLOCK
════════════════════════════════════════════════════════════════════════════
The first version of this sweep drove the UI with `/cmd` and slept. It emitted
thirteen PNGs, and the audit that ran on them was largely wasted:

  - The Settings modal was open in ALL THIRTEEN, covering the chart. Nothing
    had closed it, and no step checked.
  - `02/03/04-watchlist-{chain,heat,scan}.png` were pixel-identical: the tab
    switches never landed, so three of the four tabs have no visual evidence
    they render at all. Three separate review agents each independently
    reported "these images are the same" — the harness told nobody.
  - `07-orders-panel.png` was a byte-for-byte copy of `05-scanner.png`,
    because `OpenOrdersPanel` was writing to the headless ticker's simulated
    state instead of the app's (fixed: `AppCommand::SetDialogOpen`).

Every one of those was a silent pass. The commands returned `{"ok":true}` —
which only ever meant "queued", never "applied" — and the screenshot step
happily wrote a file named after a state the app was not in.

So: a surface is captured ONLY after `/state` confirms the app is actually in
that state. If it never converges, the capture RAISES and no file is written.
A missing file is a bug report; a mislabelled file is a lie that survives into
the audit and costs an entire review round.

This is the same rule `capture_app.py::capture_pair` already learned for
theme/style convergence — generalised from "the palette is right" to "the
screen is right".

Corollary, learned the hard way: anything a scenario can SWITCH, `/state` must
be able to REPORT. The watchlist tab was switchable but not observable, which
is precisely why the tab sweep could fail four times in silence. If you add a
surface here and find you cannot assert it, add the field to `/state` first.

Usage:
    python capture_surfaces.py                     # full sweep -> docs/styling/audit
    python capture_surfaces.py --only 09,13
    python capture_surfaces.py --list
    python capture_surfaces.py --port 7893 --preset aperture:aperture
"""

import argparse
import json
import os
import shutil
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SHOT_DIR = REPO_ROOT / "dev" / "screenshots"
DEFAULT_OUT = REPO_ROOT / "docs" / "styling" / "audit"
DEFAULT_PORT = int(os.environ.get("APEX_DEV_INSPECTOR_PORT", "7892"))

# Shared with capture_app.py — see its header for the full index maps.
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

# Audit on a CERTIFIED pairing, not whatever the app happened to boot with.
#
# The previous audit set was shot on theme_idx=15 + style_idx=0 — a "Custom
# pairing" nobody designed. Colour findings from an uncertified combination
# are not actionable: you cannot tell a palette bug from a pairing nobody
# intended to ship. Geometry findings (overlap, clipping, alignment) survive
# any pairing; colour findings do not.
DEFAULT_PRESET = "aperture:aperture"

# ── Surface catalogue ────────────────────────────────────────────────────────
# `cmds`    — driven before the shot.
# `require` — must hold in /state before the shot is taken. No require block
#             means "the default screen"; we still assert no modal is up, since
#             a stuck modal is exactly what ruined the last set.
#
# Assertion keys:
#   no_dialogs      : bool  — open_dialogs must be empty
#   dialogs_open    : list  — each name must be present
#   dialogs_closed  : list  — each name must be absent
#   watchlist_tab   : str   — list | chain | heat | scan
SURFACES = [
    dict(name="01-default", cmds=[
        {"cmd": "SetWatchlistTab", "tab": "list"}],
        require=dict(no_dialogs=True, watchlist_tab="list")),

    dict(name="02-watchlist-chain", cmds=[
        {"cmd": "SetWatchlistTab", "tab": "chain"}],
        require=dict(no_dialogs=True, watchlist_tab="chain")),

    dict(name="03-watchlist-heat", cmds=[
        {"cmd": "SetWatchlistTab", "tab": "heat"}],
        require=dict(no_dialogs=True, watchlist_tab="heat")),

    dict(name="04-watchlist-scan", cmds=[
        {"cmd": "SetWatchlistTab", "tab": "scan"}],
        require=dict(no_dialogs=True, watchlist_tab="scan")),

    # The DOM ladder — the densest numeric surface in the app and the one that
    # carried the most defects. No dialog state to assert, but the sweep must
    # still confirm no modal is up over it.
    dict(name="09-dom-sidebar", cmds=[
        {"cmd": "SetWatchlistTab", "tab": "list"},
        {"cmd": "SetDomSidebar", "pane": 0, "open": True}],
        require=dict(no_dialogs=True)),

    dict(name="07-orders-panel", cmds=[
        {"cmd": "OpenOrdersPanel"}],
        require=dict(dialogs_open=["orders_panel"], dialogs_closed=["settings"])),

    # 08-order-entry is DELIBERATELY ABSENT. `order_entry_open` is a dead
    # flag: it is declared on `Watchlist`, defaulted, mirrored into
    # `SidebarState` both ways, persisted, and reported by
    # `/state.open_dialogs` — and NOTHING in the UI reads it. There is no
    # order-entry form to photograph.
    #
    # Worth stating plainly, because it is a limitation of this script's whole
    # approach: the assertions verify STATE, not VISIBILITY. Here the state was
    # reachable and the pixels were not, so the capture passed its check and
    # produced a screenshot with no order form in it. That is strictly better
    # than the old silent-wrong-screen failure — the surface is at least
    # named and its state confirmed — but "the flag is set" is not "the user
    # can see it". Restore this entry when the panel actually renders.

    dict(name="13-settings", cmds=[
        {"cmd": "CloseOrderEntry"}, {"cmd": "OpenSettings"}],
        require=dict(dialogs_open=["settings"])),
]


def http(method, port, path, body=None, timeout=15.0):
    url = f"http://127.0.0.1:{port}{path}"
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(url, data=data, method=method,
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.status, json.loads(r.read().decode() or "null")


def cmd(port, body):
    status, resp = http("POST", port, "/cmd", body)
    if status != 200 or not (isinstance(resp, dict) and resp.get("ok")):
        raise RuntimeError(f"/cmd {body} -> {status} {resp}")
    return resp


def check(state, require):
    """Return None if `state` satisfies `require`, else a human-readable reason.

    Deliberately returns the REASON rather than a bool: when a sweep fails at
    3am the difference between 'surface 04 failed' and 'surface 04: watchlist
    tab is chain, wanted scan' is the difference between a fixed bug and
    another round of guessing.
    """
    dialogs = state.get("open_dialogs", [])
    if require.get("no_dialogs") and dialogs:
        return f"expected no open dialogs, found {dialogs}"
    for d in require.get("dialogs_open", []):
        if not any(x == d or x.startswith(d + ".") for x in dialogs):
            return f"dialog '{d}' is not open (open: {dialogs})"
    for d in require.get("dialogs_closed", []):
        if any(x == d or x.startswith(d + ".") for x in dialogs):
            return f"dialog '{d}' is still open (open: {dialogs})"
    want_tab = require.get("watchlist_tab")
    if want_tab is not None:
        got = state.get("watchlist", {}).get("tab")
        if got != want_tab:
            if got is None:
                return ("/state does not report watchlist.tab — the app predates "
                        "that field, so this assertion cannot be checked. Rebuild.")
            return f"watchlist tab is '{got}', wanted '{want_tab}'"
    return None


def converge(port, require, timeout_s=10.0):
    """Poll /state until `require` holds. Raises with the last reason on timeout."""
    deadline = time.time() + timeout_s
    reason = "never polled"
    while time.time() < deadline:
        try:
            _, st = http("GET", port, "/state", None, timeout=5.0)
            reason = check(st, require)
            if reason is None:
                return
        except Exception as e:                 # transient during repaint
            reason = f"/state unreachable: {e}"
        time.sleep(0.1)
    raise RuntimeError(f"state never converged: {reason}")


def capture(port, surface, out_dir, settle_ms):
    name = surface["name"]
    for c in surface.get("cmds", []):
        cmd(port, c)
    # Assert BEFORE the shot. This is the whole point of the script.
    converge(port, surface.get("require", {"no_dialogs": True}))
    time.sleep(settle_ms / 1000.0)             # let the repaint settle

    shot = name.replace("-", "_")
    src = SHOT_DIR / f"{shot}.png"
    if src.exists():
        src.unlink()                           # never let a stale file pose as fresh

    status, resp = http("POST", port, "/screenshot", {"name": shot}, timeout=20.0)
    if status != 200:
        raise RuntimeError(f"/screenshot -> {status} {resp}")

    deadline = time.time() + 5.0
    while not src.exists() and time.time() < deadline:
        time.sleep(0.1)
    if not src.exists():
        raise RuntimeError(f"screenshot file never appeared: {src}")
    time.sleep(0.15)

    out_dir.mkdir(parents=True, exist_ok=True)
    dest = out_dir / f"{name}.png"
    shutil.copy2(src, dest)
    return dest


def resolve(token, names, what):
    token = token.strip().lower()
    if token.isdigit():
        idx = int(token)
        return idx, (names[idx] if idx < len(names) else f"{what}{idx}")
    if token in names:
        return names.index(token), token
    raise SystemExit(f"unknown {what} '{token}' (index or one of {names})")


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--port", type=int, default=DEFAULT_PORT)
    ap.add_argument("--out", type=Path, default=DEFAULT_OUT)
    ap.add_argument("--preset", default=DEFAULT_PRESET,
                    help=f'theme:style to pin (default "{DEFAULT_PRESET}")')
    ap.add_argument("--only", default=None,
                    help="comma-separated surface prefixes, e.g. 09,13")
    ap.add_argument("--settle-ms", type=int, default=500)
    ap.add_argument("--list", action="store_true", help="list surfaces and exit")
    args = ap.parse_args()

    if args.list:
        for s in SURFACES:
            print(f"{s['name']:24} require={s.get('require')}")
        return 0

    out = Path(args.out).resolve()

    try:
        http("GET", args.port, "/health", timeout=5.0)
    except (urllib.error.URLError, OSError) as e:
        print(f"ERROR: dev_inspector unreachable on 127.0.0.1:{args.port} ({e})",
              file=sys.stderr)
        print("The app must be a running DEBUG build.", file=sys.stderr)
        return 1

    t_tok, _, s_tok = args.preset.partition(":")
    t_idx, t_name = resolve(t_tok, THEME_NAMES, "theme")
    s_idx, s_name = resolve(s_tok, STYLE_NAMES, "style")
    cmd(args.port, {"cmd": "SetThemeIdx", "idx": t_idx, "pane": 0})
    cmd(args.port, {"cmd": "SetStyleIdx", "idx": s_idx})
    deadline = time.time() + 10.0
    seen = None
    while time.time() < deadline:
        try:
            _, st = http("GET", args.port, "/state", None, timeout=5.0)
            seen = (st.get("active_theme_idx"), st.get("active_style_idx"))
            if seen == (t_idx, s_idx):
                break
        except Exception:
            pass
        time.sleep(0.1)
    else:
        print(f"ERROR: preset never converged: asked {(t_idx, s_idx)}, app reports {seen}",
              file=sys.stderr)
        return 1
    print(f"preset: {t_name}({t_idx}) / {s_name}({s_idx})")

    # Start from a known-clean UI. The last sweep inherited a modal from
    # whatever ran before it and never noticed.
    cmd(args.port, {"cmd": "CloseAllDialogs"})
    time.sleep(0.3)

    todo = SURFACES
    if args.only:
        keys = [k.strip() for k in args.only.split(",")]
        todo = [s for s in SURFACES if any(s["name"].startswith(k) for k in keys)]
        if not todo:
            print(f"no surface matches --only {args.only}", file=sys.stderr)
            return 1

    failures = []
    for s in todo:
        try:
            dest = capture(args.port, s, out, args.settle_ms)
            print(f"  ok  {s['name']:24} -> {dest.relative_to(REPO_ROOT)}")
        except Exception as e:
            failures.append(s["name"])
            print(f"  !!  {s['name']:24} {e}", file=sys.stderr)

    print(f"\n{len(todo) - len(failures)}/{len(todo)} captured -> {out}")
    if failures:
        print("NOT captured (state never reached — no file written): "
              + ", ".join(failures), file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
