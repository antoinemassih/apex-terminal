#!/usr/bin/env python3
"""Push apex-terminal design tokens to Figma as Variables (all 8 themes as modes)."""
import json, requests, sys, os

FIGMA_TOKEN = os.environ.get("FIGMA_TOKEN", "")
FILE_KEY    = "nrOOadxGEJdfgNV5usDwbz"
URL         = f"https://api.figma.com/v1/files/{FILE_KEY}/variables"
HEADERS     = {"X-Figma-Token": FIGMA_TOKEN, "Content-Type": "application/json"}

# ── helpers ──────────────────────────────────────────────────────────────────
def hex_to_rgba(h):
    h = h.lstrip("#")
    if len(h) == 8:
        r,g,b,a = int(h[0:2],16),int(h[2:4],16),int(h[4:6],16),int(h[6:8],16)
        return {"r":r/255,"g":g/255,"b":b/255,"a":a/255}
    r,g,b = int(h[0:2],16),int(h[2:4],16),int(h[4:6],16)
    return {"r":r/255,"g":g/255,"b":b/255,"a":1.0}

# ── themes ───────────────────────────────────────────────────────────────────
THEMES = {
    "Midnight": {
        "background":"#0d0d0d","bull":"#2ecc71","bear":"#e74c3c",
        "bullVolume":"#2ecc7140","bearVolume":"#e74c3c40","wick":"#555555",
        "grid":"#262626","axisText":"#666666","crosshair":"#1a1a2e",
        "ohlcLabel":"#cccccc","borderActive":"#2a6496","borderInactive":"#1a1a1a",
        "toolbarBackground":"#111111","toolbarBorder":"#222222",
    },
    "Nord": {
        "background":"#2e3440","bull":"#a3be8c","bear":"#bf616a",
        "bullVolume":"#a3be8c40","bearVolume":"#bf616a40","wick":"#4c566a",
        "grid":"#3b4252","axisText":"#81a1c1","crosshair":"#434c5e",
        "ohlcLabel":"#d8dee9","borderActive":"#88c0d0","borderInactive":"#3b4252",
        "toolbarBackground":"#2e3440","toolbarBorder":"#3b4252",
    },
    "Monokai": {
        "background":"#272822","bull":"#a6e22e","bear":"#f92672",
        "bullVolume":"#a6e22e40","bearVolume":"#f9267240","wick":"#75715e",
        "grid":"#3e3d32","axisText":"#a59f85","crosshair":"#49483e",
        "ohlcLabel":"#f8f8f2","borderActive":"#e6db74","borderInactive":"#3e3d32",
        "toolbarBackground":"#1e1f1c","toolbarBorder":"#3e3d32",
    },
    "Solarized Dark": {
        "background":"#002b36","bull":"#859900","bear":"#dc322f",
        "bullVolume":"#85990040","bearVolume":"#dc322f40","wick":"#586e75",
        "grid":"#073642","axisText":"#839496","crosshair":"#073642",
        "ohlcLabel":"#93a1a1","borderActive":"#2aa198","borderInactive":"#073642",
        "toolbarBackground":"#002b36","toolbarBorder":"#073642",
    },
    "Dracula": {
        "background":"#282a36","bull":"#50fa7b","bear":"#ff5555",
        "bullVolume":"#50fa7b40","bearVolume":"#ff555540","wick":"#6272a4",
        "grid":"#343746","axisText":"#bd93f9","crosshair":"#44475a",
        "ohlcLabel":"#f8f8f2","borderActive":"#ff79c6","borderInactive":"#343746",
        "toolbarBackground":"#21222c","toolbarBorder":"#343746",
    },
    "Gruvbox": {
        "background":"#282828","bull":"#b8bb26","bear":"#fb4934",
        "bullVolume":"#b8bb2640","bearVolume":"#fb493440","wick":"#665c54",
        "grid":"#3c3836","axisText":"#d5c4a1","crosshair":"#3c3836",
        "ohlcLabel":"#ebdbb2","borderActive":"#fe8019","borderInactive":"#3c3836",
        "toolbarBackground":"#1d2021","toolbarBorder":"#3c3836",
    },
    "Catppuccin": {
        "background":"#1e1e2e","bull":"#a6e3a1","bear":"#f38ba8",
        "bullVolume":"#a6e3a140","bearVolume":"#f38ba840","wick":"#585b70",
        "grid":"#313244","axisText":"#b4befe","crosshair":"#313244",
        "ohlcLabel":"#cdd6f4","borderActive":"#cba6f7","borderInactive":"#313244",
        "toolbarBackground":"#181825","toolbarBorder":"#313244",
    },
    "Tokyo Night": {
        "background":"#1a1b26","bull":"#9ece6a","bear":"#f7768e",
        "bullVolume":"#9ece6a40","bearVolume":"#f7768e40","wick":"#565f89",
        "grid":"#24283b","axisText":"#7aa2f7","crosshair":"#292e42",
        "ohlcLabel":"#c0caf5","borderActive":"#7dcfff","borderInactive":"#24283b",
        "toolbarBackground":"#16161e","toolbarBorder":"#24283b",
    },
}

COLOR_KEYS = [
    "background","bull","bear","bullVolume","bearVolume","wick","grid",
    "axisText","crosshair","ohlcLabel","borderActive","borderInactive",
    "toolbarBackground","toolbarBorder",
]
theme_list = list(THEMES.keys())

# ── build payload ─────────────────────────────────────────────────────────────
collections, modes, variables, mode_values = [], [], [], []

# 1. COLORS – multi-mode (one mode per theme)
col_colors  = "temp_col_colors"
init_mode   = f"temp_mode_{theme_list[0].replace(' ','_')}"

collections.append({
    "action": "CREATE", "id": col_colors,
    "name": "Colors", "initialModeId": init_mode,
})

theme_modes = {}
for i, name in enumerate(theme_list):
    mid = f"temp_mode_{name.replace(' ','_')}"
    theme_modes[name] = mid
    if i == 0:
        modes.append({"action": "UPDATE", "id": mid, "name": name})
    else:
        modes.append({"action": "CREATE", "id": mid, "name": name, "variableCollectionId": col_colors})

for key in COLOR_KEYS:
    vid = f"temp_var_color_{key}"
    variables.append({"action":"CREATE","id":vid,"name":f"theme/{key}",
                       "variableCollectionId":col_colors,"resolvedType":"COLOR"})
    for tname, tcolors in THEMES.items():
        mode_values.append({"variableId":vid,"modeId":theme_modes[tname],
                             "value":hex_to_rgba(tcolors[key])})

# 2. SEMANTIC COLORS – single mode
col_sem = "temp_col_semantic"
mid_sem = "temp_mode_semantic"
SEMANTIC = {
    "danger":"#e05560","success":"#2ecc71","warning":"#f59e0b",
    "accent":"#7aa2f7","text":"#cccccc","text-dim":"#666666",
    "text-muted":"#888888","overlay":"#000000b3",
}
collections.append({"action":"CREATE","id":col_sem,"name":"Semantic","initialModeId":mid_sem})
modes.append({"action":"UPDATE","id":mid_sem,"name":"Default"})
for key, hex_val in SEMANTIC.items():
    vid = f"temp_var_sem_{key}"
    variables.append({"action":"CREATE","id":vid,"name":f"semantic/{key}",
                       "variableCollectionId":col_sem,"resolvedType":"COLOR"})
    mode_values.append({"variableId":vid,"modeId":mid_sem,"value":hex_to_rgba(hex_val)})

# 3. TYPOGRAPHY – font sizes as FLOAT
col_type = "temp_col_type"
mid_type = "temp_mode_type"
TYPE_SIZES = {"axis":8,"label":9,"small":10,"base":11,"body":12,"md":13,"lg":14,"xl":16}
collections.append({"action":"CREATE","id":col_type,"name":"Typography","initialModeId":mid_type})
modes.append({"action":"UPDATE","id":mid_type,"name":"Default"})
for key, val in TYPE_SIZES.items():
    vid = f"temp_var_type_{key}"
    variables.append({"action":"CREATE","id":vid,"name":f"fontSize/{key}",
                       "variableCollectionId":col_type,"resolvedType":"FLOAT"})
    mode_values.append({"variableId":vid,"modeId":mid_type,"value":float(val)})

# 4. SPACING
col_space = "temp_col_spacing"
mid_space = "temp_mode_spacing"
SPACING = {str(v):v for v in [1,2,3,4,5,6,8,10,12,14,16,20,24]}
collections.append({"action":"CREATE","id":col_space,"name":"Spacing","initialModeId":mid_space})
modes.append({"action":"UPDATE","id":mid_space,"name":"Default"})
for key, val in SPACING.items():
    vid = f"temp_var_space_{key}"
    variables.append({"action":"CREATE","id":vid,"name":f"spacing/{key}",
                       "variableCollectionId":col_space,"resolvedType":"FLOAT"})
    mode_values.append({"variableId":vid,"modeId":mid_space,"value":float(val)})

# 5. BORDER RADIUS
col_rad = "temp_col_radius"
mid_rad = "temp_mode_radius"
RADII = {"sm":2,"md":3,"lg":4,"xl":6,"2xl":8}
collections.append({"action":"CREATE","id":col_rad,"name":"Border Radius","initialModeId":mid_rad})
modes.append({"action":"UPDATE","id":mid_rad,"name":"Default"})
for key, val in RADII.items():
    vid = f"temp_var_rad_{key}"
    variables.append({"action":"CREATE","id":vid,"name":f"radius/{key}",
                       "variableCollectionId":col_rad,"resolvedType":"FLOAT"})
    mode_values.append({"variableId":vid,"modeId":mid_rad,"value":float(val)})

payload = {
    "variableCollections": collections,
    "variableModes":        modes,
    "variables":            variables,
    "variableModeValues":   mode_values,
}

print(f"Collections : {len(collections)}")
print(f"Modes       : {len(modes)}")
print(f"Variables   : {len(variables)}")
print(f"Mode values : {len(mode_values)}")
print("Posting to Figma…")

resp = requests.post(URL, headers=HEADERS, json=payload)
print(f"Status: {resp.status_code}")
try:
    out = resp.json()
    if resp.status_code == 200:
        print("✅ Success!")
        print(json.dumps(out, indent=2)[:1000])
    else:
        print("❌ Error:")
        print(json.dumps(out, indent=2))
except Exception as e:
    print("Raw:", resp.text[:2000])
