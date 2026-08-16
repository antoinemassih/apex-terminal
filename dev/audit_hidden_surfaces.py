#!/usr/bin/env python3
"""Audit surfaces that are normally INVISIBLE, one at a time.

`/design-audit` inspects the widgets present in the current frame. Every modal,
dialog and collapsed panel is therefore unaudited by default — which is how the
settings panel carried 21 touch-target failures while the audit reported `clean`
on all nine styles (AT-161).

This opens each surface, audits it, and closes it again, so the check covers
what the default sweep structurally cannot see.
"""
import json
import sys
import time
import urllib.request

BASE = "http://127.0.0.1:7899"


def post(path, payload):
    req = urllib.request.Request(
        f"{BASE}{path}",
        data=json.dumps(payload).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=10) as r:
        return json.loads(r.read())


def get(path):
    with urllib.request.urlopen(f"{BASE}{path}", timeout=15) as r:
        return json.loads(r.read())


# (label, open command, id prefix that proves it actually opened)
SURFACES = [
    ("orders panel",     {"cmd": "set_order_panel", "open": True},     "orders."),
    ("scanner",          {"cmd": "set_scanner_open", "open": True},    "scanner."),
    ("rrg",              {"cmd": "set_rrg_open", "open": True},        "rrg."),
    ("object tree",      {"cmd": "set_object_tree", "open": True},     "object_tree."),
    ("playbook",         {"cmd": "set_playbook_panel", "open": True},  "play"),
    ("auto-chart",       {"cmd": "set_auto_chart_panel", "open": True}, "auto_chart"),
    ("dom sidebar",      {"cmd": "set_dom_sidebar", "open": True},     "dom"),
    ("hotkey editor",    {"cmd": "open_hotkey_editor"},                "hotkey"),
    ("indicator editor", {"cmd": "open_indicator_editor"},             "indicator"),
]


def main():
    total_fail = 0
    rows = []
    for label, cmd, prefix in SURFACES:
        try:
            post("/cmd", {"cmd": "close_all_dialogs"})
            time.sleep(0.4)
            post("/cmd", cmd)
            time.sleep(0.9)
            tree = get("/widget-tree")
            present = sum(1 for w in tree if w["id"].startswith(prefix))
            a = get("/design-audit")
            clip = a["clipping"]["fail"]
            touch = a["touch_targets"]["fail"]
            empty = a["empty_rects"]["fail"]
            bad = clip + touch + empty
            total_fail += bad
            rows.append((label, present, clip, touch, empty))
        except Exception as e:  # noqa: BLE001 — a surface that will not open is data
            rows.append((label, -1, 0, 0, 0))
            print(f"  {label}: could not drive ({e})", file=sys.stderr)

    print(f"{'surface':<18}{'widgets':>8}{'clip':>6}{'touch':>7}{'empty':>7}")
    for label, present, clip, touch, empty in rows:
        flag = "  <-- never opened" if present == 0 else ""
        print(f"{label:<18}{present:>8}{clip:>6}{touch:>7}{empty:>7}{flag}")
    print(f"\ntotal failures across hidden surfaces: {total_fail}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
