// ============================================================
//  apex-terminal  ·  Figma Design Generator
//  Paste into: Plugins > Development > Open Console
//  Generates: Design Tokens page + App Layouts page + Components page
// ============================================================

(async () => {

// ── fonts ────────────────────────────────────────────────────────────────────
await figma.loadFontAsync({ family: "Roboto Mono", style: "Regular" });
await figma.loadFontAsync({ family: "Roboto Mono", style: "Medium" });
const F  = { family: "Roboto Mono", style: "Regular" };
const FB = { family: "Roboto Mono", style: "Medium" };

// ── themes ───────────────────────────────────────────────────────────────────
const THEMES = {
  "Midnight":      { bg:"#0d0d0d",bull:"#2ecc71",bear:"#e74c3c",wick:"#555555",grid:"#262626",axisText:"#666666",ohlcLabel:"#cccccc",accent:"#2a6496",inactive:"#1a1a1a",tbBg:"#111111",tbBorder:"#222222" },
  "Nord":          { bg:"#2e3440",bull:"#a3be8c",bear:"#bf616a",wick:"#4c566a",grid:"#3b4252",axisText:"#81a1c1",ohlcLabel:"#d8dee9",accent:"#88c0d0",inactive:"#3b4252",tbBg:"#2e3440",tbBorder:"#3b4252" },
  "Monokai":       { bg:"#272822",bull:"#a6e22e",bear:"#f92672",wick:"#75715e",grid:"#3e3d32",axisText:"#a59f85",ohlcLabel:"#f8f8f2",accent:"#e6db74",inactive:"#3e3d32",tbBg:"#1e1f1c",tbBorder:"#3e3d32" },
  "Solarized Dark":{ bg:"#002b36",bull:"#859900",bear:"#dc322f",wick:"#586e75",grid:"#073642",axisText:"#839496",ohlcLabel:"#93a1a1",accent:"#2aa198",inactive:"#073642",tbBg:"#002b36",tbBorder:"#073642" },
  "Dracula":       { bg:"#282a36",bull:"#50fa7b",bear:"#ff5555",wick:"#6272a4",grid:"#343746",axisText:"#bd93f9",ohlcLabel:"#f8f8f2",accent:"#ff79c6",inactive:"#343746",tbBg:"#21222c",tbBorder:"#343746" },
  "Gruvbox":       { bg:"#282828",bull:"#b8bb26",bear:"#fb4934",wick:"#665c54",grid:"#3c3836",axisText:"#d5c4a1",ohlcLabel:"#ebdbb2",accent:"#fe8019",inactive:"#3c3836",tbBg:"#1d2021",tbBorder:"#3c3836" },
  "Catppuccin":    { bg:"#1e1e2e",bull:"#a6e3a1",bear:"#f38ba8",wick:"#585b70",grid:"#313244",axisText:"#b4befe",ohlcLabel:"#cdd6f4",accent:"#cba6f7",inactive:"#313244",tbBg:"#181825",tbBorder:"#313244" },
  "Tokyo Night":   { bg:"#1a1b26",bull:"#9ece6a",bear:"#f7768e",wick:"#565f89",grid:"#24283b",axisText:"#7aa2f7",ohlcLabel:"#c0caf5",accent:"#7dcfff",inactive:"#24283b",tbBg:"#16161e",tbBorder:"#24283b" },
};

// ── color helpers ─────────────────────────────────────────────────────────────
function hexRGB(h) {
  h = h.replace("#","");
  const r=parseInt(h.slice(0,2),16)/255, g=parseInt(h.slice(2,4),16)/255, b=parseInt(h.slice(4,6),16)/255;
  return {r,g,b};
}
function fill(hex, a=1)   { return [{ type:"SOLID", color:hexRGB(hex), opacity:a }]; }
function noFill()          { return []; }
function stroke(hex, a=1) { return [{ type:"SOLID", color:hexRGB(hex), opacity:a }]; }

// ── node factories ────────────────────────────────────────────────────────────
function mkFrame(name, w, h, fillHex, fillA=1) {
  const f = figma.createFrame();
  f.name = name;
  f.resize(w, h);
  f.fills = fillHex ? fill(fillHex, fillA) : noFill();
  f.clipsContent = true;
  return f;
}
function mkRect(name, w, h, fillHex, fillA=1) {
  const r = figma.createRectangle();
  r.name = name; r.resize(w, h);
  r.fills = fillHex ? fill(fillHex, fillA) : noFill();
  return r;
}
function mkText(str, size, colorHex, colorA=1, font=F) {
  const t = figma.createText();
  t.fontName = font; t.fontSize = size;
  t.characters = String(str);
  t.fills = colorHex ? fill(colorHex, colorA) : noFill();
  return t;
}
function border(node, colorHex, a=1, w=1) {
  node.strokes = stroke(colorHex, a);
  node.strokeWeight = w;
  node.strokeAlign = "INSIDE";
  return node;
}
function add(parent, child, x=0, y=0) {
  child.x = x; child.y = y;
  parent.appendChild(child);
  return child;
}

// ── Figma Variable helpers ────────────────────────────────────────────────────
function createVarCollection(name, initialModeId) {
  const c = figma.variables.createVariableCollection(name);
  // rename the default mode
  c.modes[0].name = initialModeId; // use first mode as "initialModeId" label
  return c;
}

// ============================================================
//  STEP 1 — Figma Variables (Design Tokens)
// ============================================================

// — Colors collection (one mode per theme) —
const colorsCol = figma.variables.createVariableCollection("Colors");
const colorKeys = ["bg","bull","bear","wick","grid","axisText","ohlcLabel","accent","inactive","tbBg","tbBorder"];
const themeNames = Object.keys(THEMES);

// rename the default mode to first theme
colorsCol.renameMode(colorsCol.modes[0].modeId, themeNames[0]);

// add remaining theme modes
const themeModeIds = { [themeNames[0]]: colorsCol.modes[0].modeId };
for (let i = 1; i < themeNames.length; i++) {
  const mid = colorsCol.addMode(themeNames[i]);
  themeModeIds[themeNames[i]] = mid;
}

// create variables and set per-theme values
const colorVars = {};
for (const key of colorKeys) {
  const v = figma.variables.createVariable(`theme/${key}`, colorsCol, "COLOR");
  colorVars[key] = v;
  for (const [tname, tval] of Object.entries(THEMES)) {
    const hex = tval[key];
    if (!hex) continue;
    const {r,g,b} = hexRGB(hex);
    v.setValueForMode(themeModeIds[tname], {r,g,b,a:1});
  }
}

// — Semantic colors —
const semCol = figma.variables.createVariableCollection("Semantic");
semCol.renameMode(semCol.modes[0].modeId, "Default");
const SEM = { danger:"#e05560", success:"#2ecc71", warning:"#f59e0b", accent:"#7aa2f7", text:"#cccccc", "text-dim":"#666666" };
for (const [k, hex] of Object.entries(SEM)) {
  const v = figma.variables.createVariable(`semantic/${k}`, semCol, "COLOR");
  const {r,g,b} = hexRGB(hex);
  v.setValueForMode(semCol.modes[0].modeId, {r,g,b,a:1});
}

// — Typography —
const typeCol = figma.variables.createVariableCollection("Typography");
typeCol.renameMode(typeCol.modes[0].modeId, "Default");
const SIZES = { axis:8, label:9, small:10, base:11, body:12, md:13, lg:14, xl:16 };
for (const [k,v] of Object.entries(SIZES)) {
  const vr = figma.variables.createVariable(`fontSize/${k}`, typeCol, "FLOAT");
  vr.setValueForMode(typeCol.modes[0].modeId, v);
}

// — Spacing —
const spacCol = figma.variables.createVariableCollection("Spacing");
spacCol.renameMode(spacCol.modes[0].modeId, "Default");
for (const v of [1,2,3,4,5,6,8,10,12,14,16,20,24]) {
  const vr = figma.variables.createVariable(`spacing/${v}`, spacCol, "FLOAT");
  vr.setValueForMode(spacCol.modes[0].modeId, v);
}

// — Border Radius —
const radCol = figma.variables.createVariableCollection("Border Radius");
radCol.renameMode(radCol.modes[0].modeId, "Default");
for (const [k,v] of Object.entries({sm:2,md:3,lg:4,xl:6,"2xl":8})) {
  const vr = figma.variables.createVariable(`radius/${k}`, radCol, "FLOAT");
  vr.setValueForMode(radCol.modes[0].modeId, v);
}

// ============================================================
//  STEP 2 — App Layout  (Midnight theme, 1440×900)
// ============================================================

const T = THEMES["Midnight"];
const [W, H, TH, WW, PAW, TAH] = [1440, 900, 36, 200, 50, 22];
const PAGE = figma.currentPage;
PAGE.name = "App Layouts";

// — Main frame —
const APP = mkFrame("Workspace / 1-Pane — Midnight", W, H, T.bg);

// ·· Toolbar ··
const TBAR = mkFrame("Toolbar", W, TH, T.tbBg);
border(TBAR, T.tbBorder);
add(TBAR, mkRect("border-bottom", W, 1, T.tbBorder), 0, TH-1);

// Logo mark (simple diamond)
const logo = mkRect("logo-mark", 13, 13, T.accent);
logo.cornerRadius = 2;
add(TBAR, logo, 10, 11);

// Symbol button
const symBtn = mkFrame("symbol-btn", 72, 26, T.tbBg);
symBtn.cornerRadius = 3;
border(symBtn, T.tbBorder);
const symTxt = mkText("AAPL", 12, T.accent, 1, FB);
add(symBtn, symTxt, 8, 5);
add(TBAR, symBtn, 30, 5);

// Timeframes
const TFS = ["1m","5m","15m","1h","4h","1D","1W","1M"];
TFS.forEach((tf, i) => {
  const active = tf === "1D";
  const btn = mkFrame(`tf-${tf}`, 32, 26, active ? T.accent : T.tbBg, active ? 0.15 : 1);
  btn.cornerRadius = 3;
  border(btn, active ? T.accent : T.tbBorder, active ? 0.5 : 1);
  add(btn, mkText(tf, 11, active ? T.accent : T.ohlcLabel), 5, 5);
  add(TBAR, btn, 110 + i*34, 5);
});

// Indicator toggles
const INDS = ["SMA20","EMA50","BOLL","VOL"];
INDS.forEach((ind, i) => {
  const btn = mkFrame(`ind-${ind}`, 46, 26, T.tbBg);
  btn.cornerRadius = 3;
  border(btn, T.tbBorder);
  add(btn, mkText(ind, 10, T.ohlcLabel, 0.6), 4, 5);
  add(TBAR, btn, 396 + i*50, 5);
});

// Layout selector buttons
const LAYOUTS = ["1","2","2H","4","6","9"];
LAYOUTS.forEach((lay, i) => {
  const btn = mkFrame(`layout-${lay}`, 28, 26, T.tbBg);
  btn.cornerRadius = 3;
  border(btn, T.tbBorder);
  add(btn, mkText(lay, 10, T.ohlcLabel, 0.6), 4, 5);
  add(TBAR, btn, 606 + i*32, 5);
});

// Connection dot
const connBtn = mkFrame("conn-btn", 70, 26, T.tbBg);
connBtn.cornerRadius = 3;
border(connBtn, T.tbBorder);
const dot = mkRect("status-dot", 6, 6, "#2ecc71");
dot.cornerRadius = 3;
add(connBtn, dot, 8, 10);
add(connBtn, mkText("CONN", 10, T.ohlcLabel, 0.6), 20, 6);
add(TBAR, connBtn, W - WW - 220, 5);

// Watchlist toggle
const wlToggle = mkFrame("watchlist-toggle", 32, 26, T.tbBg);
wlToggle.cornerRadius = 3;
border(wlToggle, T.tbBorder);
add(wlToggle, mkText("⊞", 13, T.ohlcLabel, 0.6), 7, 4);
add(TBAR, wlToggle, W - WW - 140, 5);

// Window controls
[["−",0],["□",40],["×",80]].forEach(([ch, ox]) => {
  const btn = mkFrame(`wc-${ch}`, 40, TH, T.tbBg);
  add(btn, mkText(ch, 13, T.ohlcLabel, 0.6), 12, 9);
  add(TBAR, btn, W - 120 + ox, 0);
});

add(APP, TBAR, 0, 0);

// ·· Pane (chart area) ··
const PANE_W = W - WW - 1;
const PANE_H = H - TH;
const PANE = mkFrame("Pane — Active", PANE_W, PANE_H, T.bg);
border(PANE, T.accent);

// OHLC label bar
const ohlc = mkText("AAPL  ·  1D  ·  O 175.32  H 181.45  L 174.12  C 180.89  ▲ +5.57 (+3.19%)", 11, T.axisText);
add(PANE, ohlc, 8, 5);

// Chart canvas
const CW = PANE_W - PAW;
const CH = PANE_H - TAH - 30;
const CHART = mkFrame("Chart Canvas", CW, CH, T.bg);
add(PANE, CHART, 0, 28);

// Horizontal grid lines
for (let i = 1; i < 7; i++) {
  const gy = Math.floor(CH * i / 7);
  add(CHART, mkRect(`hgrid-${i}`, CW, 1, T.grid), 0, gy);
}

// Simulated candlesticks (Midnight theme)
const CANDLES = [
  {x:40,  op:160, cl:172, hi:175, lo:157, bull:true},
  {x:60,  op:172, cl:168, hi:176, lo:165, bull:false},
  {x:80,  op:168, cl:180, hi:183, lo:165, bull:true},
  {x:100, op:180, cl:175, hi:184, lo:173, bull:false},
  {x:120, op:175, cl:183, hi:186, lo:173, bull:true},
  {x:140, op:183, cl:179, hi:186, lo:176, bull:false},
  {x:160, op:179, cl:188, hi:191, lo:177, bull:true},
  {x:180, op:188, cl:185, hi:192, lo:183, bull:false},
  {x:200, op:185, cl:195, hi:198, lo:184, bull:true},
  {x:220, op:195, cl:191, hi:199, lo:189, bull:false},
  {x:240, op:191, cl:200, hi:203, lo:190, bull:true},
  {x:260, op:200, cl:196, hi:204, lo:194, bull:false},
  {x:280, op:196, cl:205, hi:208, lo:195, bull:true},
  {x:300, op:205, cl:199, hi:208, lo:197, bull:false},
];
const PM=155, PX=215;
const sy = p => CH - ((p-PM)/(PX-PM))*CH;
CANDLES.forEach(c => {
  const color = c.bull ? T.bull : T.bear;
  const hy=sy(c.hi), ly=sy(c.lo), ty=Math.min(sy(c.op),sy(c.cl)), bh=Math.max(Math.abs(sy(c.cl)-sy(c.op)),2);
  add(CHART, mkRect(`wick-${c.x}`, 1, Math.max(ly-hy,2), T.wick), c.x+7, hy);
  add(CHART, mkRect(`body-${c.x}`, 14, bh, color), c.x, ty);
});

// Price axis
const PAXIS = mkFrame("Price Axis", PAW, PANE_H - TAH, T.bg);
add(PANE, PAXIS, CW, 0);
[160,165,170,175,180,185,190,195,200].forEach(p => {
  const ly = 28 + Math.floor(sy(p)) - 5;
  if (ly > 10 && ly < PANE_H - TAH - 15)
    add(PAXIS, mkText(p.toFixed(2), 8, T.axisText), 3, ly);
});

// Time axis
const TAXIS = mkFrame("Time Axis", PANE_W, TAH, T.bg);
add(PANE, TAXIS, 0, PANE_H - TAH);
["09:30","10:00","10:30","11:00","11:30","12:00","12:30","13:00"].forEach((t,i) => {
  add(TAXIS, mkText(t, 8, T.axisText), 40 + i*160, 4);
});

// LIVE badge
const liveBadge = mkFrame("LIVE", 52, 22, T.bull, 0.12);
liveBadge.cornerRadius = 3;
border(liveBadge, T.bull, 0.4);
add(liveBadge, mkText("● LIVE", 10, T.bull, 1, FB), 6, 4);
add(PANE, liveBadge, CW - 62, PANE_H - TAH - 30);

// Order entry (bottom-left overlay)
const OE = mkFrame("Order Entry", 252, 82, T.bg, 0.92);
OE.cornerRadius = 4;
border(OE, T.tbBorder, 0.3);
// Qty row
add(OE, mkText("QTY", 10, T.axisText, 0.45), 8, 10);
const qBox = mkRect("qty-box", 52, 22, null); qBox.cornerRadius=2; border(qBox, T.tbBorder, 0.35);
add(OE, qBox, 34, 6); add(OE, mkText("100", 12, T.ohlcLabel), 40, 10);
add(OE, mkText("LMT", 10, T.axisText, 0.45), 100, 10);
const lBox = mkRect("lmt-box", 64, 22, null); lBox.cornerRadius=2; border(lBox, T.tbBorder, 0.35);
add(OE, lBox, 124, 6); add(OE, mkText("180.50", 12, T.ohlcLabel), 130, 10);
const lmtToggleBtn = mkFrame("LMT-toggle", 36, 22, T.accent, 0.18);
lmtToggleBtn.cornerRadius = 2; border(lmtToggleBtn, T.accent, 0.5);
add(lmtToggleBtn, mkText("LMT", 10, T.accent, 1, FB), 5, 4);
add(OE, lmtToggleBtn, 206, 6);
// Buy/sell buttons
const SELL = mkFrame("SELL", 116, 34, T.bear, 0.13); SELL.cornerRadius=3; border(SELL, T.bear, 0.35);
add(SELL, mkText("▼ SELL  179.88", 12, T.bear, 1, FB), 8, 9);
add(OE, SELL, 8, 38);
const BUY  = mkFrame("BUY",  116, 34, T.bull, 0.13); BUY.cornerRadius=3;  border(BUY,  T.bull, 0.35);
add(BUY,  mkText("▲ BUY   180.12", 12, T.bull, 1, FB), 8, 9);
add(OE, BUY, 130, 38);
add(PANE, OE, 12, PANE_H - TAH - 96);

add(APP, PANE, 0, TH);

// ·· Watchlist ··
const WL = mkFrame("Watchlist", WW, PANE_H, T.tbBg);
border(WL, T.tbBorder);
add(APP, WL, PANE_W + 1, TH);

// Header
const WL_HDR = mkFrame("WL-Header", WW, 52, T.tbBg);
border(WL_HDR, T.tbBorder);
add(WL_HDR, mkText("WATCHLIST", 9, T.axisText, 0.4), 8, 4);
const TABS = [["STOCKS",true],["CHAIN",false],["SAVED",false]];
TABS.forEach(([tab,active],i) => {
  const t = mkFrame(`tab-${tab}`, WW/3, 28, active ? T.accent : null, active ? 0.12 : 0);
  if (active) { const tl = mkRect("active-line", WW/3, 2, T.accent); add(t, tl, 0, 26); }
  add(t, mkText(tab, 9, active ? T.accent : T.ohlcLabel, active ? 1 : 0.5, active ? FB : F), 4, 7);
  add(WL_HDR, t, (WW/3)*i, 18);
});
add(WL_HDR, mkRect("header-bottom", WW, 1, T.tbBorder), 0, 51);
add(WL, WL_HDR, 0, 0);

// Symbol rows
const SYMS = [
  ["AAPL","180.89","+1.24",true],["TSLA","245.12","-0.87",false],
  ["NVDA","892.44","+3.12",true], ["MSFT","415.67","+0.54",true],
  ["AMZN","189.23","-1.09",false],["GOOGL","172.50","+0.33",true],
  ["SPY", "524.88","+0.21",true], ["QQQ",  "453.71","+0.45",true],
  ["META","512.33","+1.88",true], ["AMD",  "162.44","-0.72",false],
];
SYMS.forEach(([sym,price,chg,bull],i) => {
  const row = mkFrame(`row-${sym}`, WW, 24, null);
  add(row, mkRect("sep", WW, 1, T.tbBorder, 0.15), 0, 23);
  add(row, mkText(sym, 12, T.ohlcLabel, 1, FB), 8, 4);
  add(row, mkText(price, 11, T.ohlcLabel, 0.85), 72, 5);
  add(row, mkText((bull?"+":"")+chg+"%", 10, bull ? T.bull : T.bear), 148, 5);
  add(WL, row, 0, 52 + i*24);
});

// ·· add to page ··
add(PAGE, APP, 0, 0);

// ============================================================
//  STEP 3 — 2-Pane Layout
// ============================================================

const APP2 = mkFrame("Workspace / 2-Pane — Midnight", W, H, T.bg);
add(PAGE, APP2, 0, H + 80);

const TBAR2 = TBAR.clone();
add(APP2, TBAR2, 0, 0);

const PANE_W2 = Math.floor((W - WW - 2) / 2);
["Left","Right"].forEach((side,i) => {
  const P = mkFrame(`Pane-${side}`, PANE_W2, PANE_H, T.bg);
  border(P, i===0 ? T.accent : T.inactive);
  // mini chart fill
  add(P, mkRect("chart-bg", PANE_W2, PANE_H-TAH-28, T.bg), 0, 28);
  for (let g=1;g<5;g++) add(P, mkRect(`g${g}`, PANE_W2-PAW, 1, T.grid), 0, 28+Math.floor((PANE_H-TAH-28)*g/5));
  add(P, mkText(`${i===0?"AAPL":"TSLA"}  ·  1D`, 10, T.axisText), 8, 5);
  add(APP2, P, i*(PANE_W2+1), TH);
});
const WL2 = WL.clone();
add(APP2, WL2, PANE_W2*2+2, TH);

// ============================================================
//  STEP 4 — Components page
// ============================================================

const COMP_PAGE = figma.createPage();
COMP_PAGE.name = "Components";
let cx = 0, cy = 0;

// ·· Context Menu ··
const CTX = mkFrame("Context Menu", 220, 10, T.tbBg);
CTX.cornerRadius = 6; border(CTX, T.tbBorder);
const MENU_ITEMS = [
  {type:"section",text:"ORDERS"},
  {type:"item",   text:"Buy Limit",  color:T.bull},
  {type:"item",   text:"Sell Limit", color:T.bear},
  {type:"item",   text:"OCO Bracket"},
  {type:"item",   text:"Trigger Order"},
  {type:"sep"},
  {type:"section",text:"DRAWINGS"},
  {type:"item",   text:"Draw Trendline"},
  {type:"item",   text:"Draw H-Line"},
  {type:"item",   text:"Drawing Groups  ▶"},
  {type:"sep"},
  {type:"item",   text:"Add Alert at $180.89"},
  {type:"sep"},
  {type:"item",   text:"Copy Price"},
  {type:"item",   text:"Save Screenshot"},
];
let my = 4;
MENU_ITEMS.forEach(item => {
  if (item.type==="sep") { add(CTX, mkRect("sep",220,1,T.tbBorder), 0, my); my+=7; }
  else if (item.type==="section") {
    add(CTX, mkText(item.text, 9, T.accent, 0.7), 14, my+2); my+=20;
  } else {
    const row = mkFrame(`mi-${item.text}`,220,24,null);
    add(row, mkText(item.text, 12, item.color||T.ohlcLabel), 14, 4);
    add(CTX, row, 0, my); my+=24;
  }
});
CTX.resize(220, my+4);
add(COMP_PAGE, CTX, cx, cy); cx += 240;

// ·· Symbol Picker ··
const SP = mkFrame("Symbol Picker", 260, 300, T.tbBg);
SP.cornerRadius = 4; border(SP, T.tbBorder);
const spRow = mkFrame("search-row", 260, 38, T.tbBg);
const spBox = mkRect("input", 236, 26, T.bg); spBox.cornerRadius=3; border(spBox, T.tbBorder);
add(spRow, spBox, 12, 6); add(spRow, mkText("AAPL", 12, T.ohlcLabel), 22, 12);
add(SP, spRow, 0, 0); add(SP, mkRect("sep",260,1,T.tbBorder), 0, 37);
add(SP, mkText("RECENT", 9, T.accent), 8, 44);
[["AAPL","Apple Inc."],["TSLA","Tesla Inc."],["NVDA","NVIDIA Corp."],["MSFT","Microsoft"],["AMZN","Amazon"]].forEach(([s,n],i) => {
  const r = mkFrame(`sp-${s}`, 260, 30, i===0 ? T.accent : null, i===0 ? 0.12 : 0);
  add(r, mkText(s, 12, T.accent, 1, FB), 8, 8);
  add(r, mkText(n, 9, T.axisText, 0.6), 60, 10);
  add(SP, r, 0, 56+i*30);
});
add(COMP_PAGE, SP, cx, cy); cx += 280;

// ·· Order Entry (standalone) ··
const OE2 = mkFrame("Order Entry", 260, 88, T.bg, 0.95);
OE2.cornerRadius = 4; border(OE2, T.tbBorder, 0.3);
add(OE2, mkText("QTY", 10, T.axisText, 0.45), 8, 12);
const qb2=mkRect("qty",52,24,null); qb2.cornerRadius=2; border(qb2,T.tbBorder,0.35); add(OE2,qb2,34,8); add(OE2,mkText("100",12,T.ohlcLabel),40,12);
add(OE2, mkText("LMT", 10, T.axisText, 0.45), 100, 12);
const lb2=mkRect("lmt",68,24,null); lb2.cornerRadius=2; border(lb2,T.tbBorder,0.35); add(OE2,lb2,126,8); add(OE2,mkText("180.50",12,T.ohlcLabel),132,12);
const lt2=mkFrame("LMT",38,24,T.accent,0.18); lt2.cornerRadius=2; border(lt2,T.accent,0.5); add(lt2,mkText("LMT",10,T.accent,1,FB),6,5); add(OE2,lt2,208,8);
const s2=mkFrame("SELL",120,36,T.bear,0.13); s2.cornerRadius=3; border(s2,T.bear,0.35); add(s2,mkText("▼ SELL  179.88",12,T.bear,1,FB),6,10); add(OE2,s2,8,44);
const b2=mkFrame("BUY", 120,36,T.bull,0.13); b2.cornerRadius=3; border(b2,T.bull,0.35); add(b2,mkText("▲ BUY   180.12",12,T.bull,1,FB),6,10); add(OE2,b2,132,44);
add(COMP_PAGE, OE2, cx, cy); cx += 280;

// ·· Connection Panel ··
const CONN = mkFrame("Connection Panel", 280, 200, T.tbBg);
CONN.cornerRadius = 6; border(CONN, T.tbBorder);
add(CONN, mkText("CONNECTIONS", 10, T.accent, 0.8, FB), 14, 12);
add(CONN, mkRect("header-sep", 280, 1, T.tbBorder), 0, 30);
const CONNS=[["WebSocket","ws://data.apex:8080","#2ecc71"],["GPU Renderer","Running (WebGPU)","#2ecc71"],["Broker API","Connected","#2ecc71"]];
CONNS.forEach(([label,detail,color],i) => {
  const row=mkFrame(`conn-${label}`,280,40,null);
  const d=mkRect("dot",8,8,color); d.cornerRadius=4; add(row,d,14,16);
  add(row,mkText(label,11,T.ohlcLabel,1,FB),30,10);
  add(row,mkText(detail,10,T.axisText,0.6),30,24);
  add(row, mkRect("sep",280,1,T.tbBorder,0.15),0,39);
  add(CONN,row,0,31+i*40);
});
add(COMP_PAGE, CONN, cx, cy); cx += 300;

// ·· Orders Panel ··
const OP = mkFrame("Orders Panel", 270, 420, T.tbBg);
border(OP, T.tbBorder);
add(OP, mkText("ORDERS", 11, T.accent, 1, FB), 10, 10);
add(OP, mkText("2d · 1p · 0e · 1c", 10, T.axisText, 0.5), 10, 26);
add(OP, mkRect("sep", 270, 1, T.tbBorder), 0, 42);
// Filter bar
const FILTERS=["All","Active","Exec","Cxl"];
FILTERS.forEach((f,i) => {
  const active=f==="Active";
  const btn=mkFrame(`flt-${f}`,62,28,active?T.accent:null,active?0.12:0);
  if(active){const bl=mkRect("line",62,2,T.accent);add(btn,bl,0,26);}
  add(btn,mkText(f,10,active?T.accent:T.ohlcLabel,active?1:0.6,active?FB:F),12,7);
  add(OP,btn,i*62,43);
});
add(OP,mkRect("flt-sep",270,1,T.tbBorder),0,71);
// Sample order card
const CARD = mkFrame("Order Card — BUY LIMIT", 250, 92, T.bg);
CARD.cornerRadius = 4; border(CARD, T.bull, 0.4);
const accentBar=mkRect("accent-bar",3,92,T.bull); accentBar.cornerRadius=2; add(CARD,accentBar,0,0);
add(CARD, mkText("▲  BUY LIMIT", 11, T.bull, 1, FB), 12, 8);
add(CARD, mkText("AAPL · 1D", 10, T.axisText, 0.7), 100, 9);
const draftBadge=mkFrame("DRAFT",40,18,T.bull,0.09); draftBadge.cornerRadius=2; border(draftBadge,T.bull,0.3);
add(draftBadge,mkText("DRAFT",9,T.bull,1,FB),4,3); add(CARD,draftBadge,200,8);
add(CARD, mkText("180.50", 16, T.bull, 1, FB), 12, 32);
add(CARD, mkText("100 shares", 10, T.axisText, 0.6), 72, 37);
add(CARD, mkText("$18,050.00", 10, T.axisText, 0.5), 170, 37);
const placeBtn=mkFrame("Place Order",230,28,T.bull,0.15); placeBtn.cornerRadius=3; border(placeBtn,T.bull,0.5);
add(placeBtn,mkText("PLACE ORDER", 11, T.bull, 1, FB), 70, 7); add(CARD,placeBtn,10,56);
add(OP,CARD,10,80);
// OCO card
const OCO = mkFrame("Order Card — OCO", 250, 88, T.bg);
OCO.cornerRadius = 4; border(OCO, "#a78bfa", 0.4);
const ocoBBar=mkRect("accent-bar",3,88,"#a78bfa"); ocoBBar.cornerRadius=2; add(OCO,ocoBBar,0,0);
add(OCO,mkText("⇅  OCO BRACKET",11,"#a78bfa",1,FB),12,8);
add(OCO,mkText("AAPL · 100 shares",10,T.axisText,0.6),12,26);
add(OCO,mkText("⇧ TARGET",9,T.axisText,0.5),12,44); add(OCO,mkText("188.00",11,T.bull,1,FB),70,42);
add(OCO,mkText("⇩ STOP",9,T.axisText,0.5),140,44);  add(OCO,mkText("174.00",11,T.bear,1,FB),190,42);
const ocoPlace=mkFrame("Place",230,24,"#a78bfa",0.15); ocoPlace.cornerRadius=3; border(ocoPlace,"#a78bfa",0.5);
add(ocoPlace,mkText("PLACE OCO",11,"#a78bfa",1,FB),76,5); add(OCO,ocoPlace,10,58);
add(OP,OCO,10,184);
add(COMP_PAGE, OP, cx, cy); cx += 290;

// ·· Theme Palette Row ··
cy += 480;
cx = 0;
const themeNames2 = Object.keys(THEMES);
themeNames2.forEach((name, i) => {
  const t2 = THEMES[name];
  const card = mkFrame(`Theme/${name}`, 160, 180, t2.bg);
  card.cornerRadius = 6; border(card, t2.tbBorder);
  add(card, mkText(name, 10, t2.ohlcLabel, 1, FB), 8, 8);
  const swatchKeys=[["bg","background"],["bull","bull"],["bear","bear"],["accent","accent"],["tbBg","toolbar"],["axisText","axis text"]];
  swatchKeys.forEach(([key,label],j) => {
    const sw=mkRect(`sw-${key}`,12,12,t2[key]);
    sw.cornerRadius=2; add(card,sw,8,28+j*22);
    add(card,mkText(label,9,t2.axisText,0.7),24,30+j*22);
    add(card,mkText(t2[key],9,t2.ohlcLabel,0.5),80,30+j*22);
  });
  add(COMP_PAGE, card, cx, cy);
  cx += 172;
});

// ── final ──────────────────────────────────────────────────────────────────
figma.viewport.scrollAndZoomIntoView([APP]);
figma.notify("✅ apex-terminal generated — App Layouts + Components + Variables!", { timeout: 5000 });

})();
