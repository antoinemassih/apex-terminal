#!/usr/bin/env python3
"""Smoke-test the NEW subsystem driver commands + capture fields end-to-end
against a running apex-native dev-inspector (127.0.0.1:7892).

Validates: SynthGamma, SetDomSidebar, SeedDraftOrder, SetOrderPanel,
SetScannerOpen/SeedScannerResults, SetRrgOpen/SetRrgTail, SeedHeatmapCells,
plus the order-manager safety telemetry (paper_mode / mgr_working_count).

Run AFTER the app is up. Exits non-zero on any failure.
"""
import json, sys, time, urllib.request

BASE = "http://127.0.0.1:7892"

def post(path, body=None):
    data = json.dumps(body or {}).encode()
    req = urllib.request.Request(BASE + path, data=data, method="POST",
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=10) as r:
        return r.read().decode()

def get(path):
    with urllib.request.urlopen(BASE + path, timeout=10) as r:
        return json.loads(r.read().decode())

def cmd(name, **kw):
    body = {"cmd": name}; body.update(kw)
    return post("/cmd", body)

def state():
    return get("/state")

def wait_frames(n=4, ms=250):
    time.sleep(ms/1000.0 * n)

fails = []
def check(label, cond, detail=""):
    tag = "PASS" if cond else "FAIL"
    print(f"  [{tag}] {label}  {detail}")
    if not cond: fails.append(label)

def jp(d, path, default=None):
    cur = d
    for part in path.split("."):
        if isinstance(cur, list):
            cur = cur[int(part)]
        elif isinstance(cur, dict):
            cur = cur.get(part)
        else:
            return default
        if cur is None: return default
    return cur

print("== health ==")
h = get("/health"); print("  health:", h if isinstance(h, dict) else "ok")

print("== reset + load SPY ==")
post("/reset"); wait_frames(3)
cmd("SwapPaneSymbol", pane=0, symbol="SPY"); time.sleep(1.2); wait_frames(3)

# 1) Safety telemetry present + safe by default
s = state()
check("order_manager.paper_mode == true", jp(s, "order_manager.paper_mode") is True,
      f"got {jp(s,'order_manager.paper_mode')}")
check("order_manager.mgr_working_count == 0", jp(s, "order_manager.mgr_working_count") == 0,
      f"got {jp(s,'order_manager.mgr_working_count')}")

# 2) SynthGamma populates levels (fixes the 3 persistent reds)
cmd("SynthGamma", pane=0); wait_frames(4); s = state()
glc = jp(s, "panes.0.gamma_level_count", 0)
check("SynthGamma -> gamma_level_count > 0", glc > 0, f"got {glc}")
check("SynthGamma -> show_gamma true", jp(s, "panes.0.show_gamma") is True)
check("gamma walls present", jp(s,"panes.0.gamma_call_wall",0) > 0 and jp(s,"panes.0.gamma_put_wall",0) > 0,
      f"call={jp(s,'panes.0.gamma_call_wall')} put={jp(s,'panes.0.gamma_put_wall')}")

# 3) DOM sidebar auto-populates a mock ladder
cmd("SetDomSidebar", pane=0, open=True); wait_frames(6); s = state()
dlc = jp(s, "panes.0.dom_level_count", 0)
check("SetDomSidebar -> dom_level_count > 0", dlc > 0, f"got {dlc}")
check("dom_sidebar_open true", jp(s, "panes.0.dom_sidebar_open") is True)
bb, ba = jp(s,"panes.0.dom_best_bid"), jp(s,"panes.0.dom_best_ask")
desc = jp(s,"panes.0.dom_prices_desc")
check("DOM ladder sane (desc + ask>=bid>0)", (desc and bb and ba and ba >= bb and bb > 0),
      f"desc={desc} bid={bb} ask={ba}")

# 4) SeedDraftOrder (visual System-A only) + safety unchanged
cmd("SeedDraftOrder", pane=0, side="buy", price=400.0, qty=100); wait_frames(2)
cmd("SeedDraftOrder", pane=0, side="sell", price=420.0, qty=200); wait_frames(2)
s = state()
odc = jp(s, "panes.0.order_draft_count", 0)
check("SeedDraftOrder x2 -> order_draft_count >= 2", odc >= 2, f"got {odc}")
check("still paper_mode after seeding", jp(s, "order_manager.paper_mode") is True)
check("still 0 mgr working orders (NO live submit)", jp(s, "order_manager.mgr_working_count") == 0,
      f"got {jp(s,'order_manager.mgr_working_count')}")
# cancel all -> the per-frame reconcile REMOVES cancelled local-only drafts
# (core.rs:250-253), so the correct post-state is order_count == 0.
cmd("CancelAllOrders"); wait_frames(4); s = state()
check("CancelAllOrders -> drafts cleaned up (order_count 0)", jp(s,"panes.0.order_count",99) == 0,
      f"got {jp(s,'panes.0.order_count')}")

# 5) Order panel collapse toggle
cmd("SetOrderPanel", pane=0, collapsed=True); wait_frames(2); s = state()
check("SetOrderPanel collapsed=true", jp(s, "panes.0.order_panel_collapsed") is True)

# 6) Scanner seed + open
cmd("SeedScannerResults", count=50); wait_frames(2)
cmd("SetScannerOpen", open=True); wait_frames(3); s = state()
check("SeedScannerResults(50) -> result_count == 50", jp(s,"scanner.result_count") == 50,
      f"got {jp(s,'scanner.result_count')}")
check("scanner.open true", jp(s,"scanner.open") is True)
fdc = jp(s, "scanner.first_def_filtered_count", -1)
check("scanner first_def_filtered_count in [0, result_count]", 0 <= fdc <= 50, f"got {fdc}")

# 7) RRG open + tail + deterministic 11 demo sectors
cmd("SetRrgOpen", open=True); wait_frames(2)
cmd("SetRrgTail", len=8); wait_frames(2); s = state()
check("rrg.open true", jp(s,"rrg.open") is True)
check("rrg.tail_length == 8", jp(s,"rrg.tail_length") == 8, f"got {jp(s,'rrg.tail_length')}")
check("rrg.sector_count == 11 (demo)", jp(s,"rrg.sector_count") == 11, f"got {jp(s,'rrg.sector_count')}")

# 8) Heatmap seed
cmd("SeedHeatmapCells", count=60); wait_frames(2); s = state()
check("SeedHeatmapCells(60) -> cell_count == 60", jp(s,"heatmap.cell_count") == 60,
      f"got {jp(s,'heatmap.cell_count')}")

# 9) Strikes overlay populates (already worked; confirm capture)
post("/reset"); wait_frames(3)
cmd("SwapPaneSymbol", pane=0, symbol="SPY"); time.sleep(1.2); wait_frames(3)
cmd("SetChartFlag", pane=0, flag="ShowStrikesOverlay", value=True); time.sleep(2.5); wait_frames(5)
s = state()
scc = jp(s,"panes.0.strikes_call_count",0); spc = jp(s,"panes.0.strikes_put_count",0)
check("strikes overlay -> call+put rows > 0", (scc + spc) > 0, f"calls={scc} puts={spc}")

print()
if fails:
    print(f"SMOKE FAILED: {len(fails)} check(s) failed: {fails}")
    sys.exit(1)
print("SMOKE PASSED: all subsystem drivers + capture fields work end-to-end.")
