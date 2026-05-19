// lib.jsx — shared helpers, fake market data, charts

// ── Seeded RNG ──────────────────────────────────────────────
function mulberry32(seed){
  return function(){
    let t = seed += 0x6D2B79F5;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// ── Fake candle data ────────────────────────────────────────
function makeCandles(n, seed = 7, start = 437, vol = 0.4){
  const r = mulberry32(seed);
  const out = [];
  let p = start;
  for (let i = 0; i < n; i++){
    const o = p;
    const drift = (r() - 0.5) * vol * 1.6;
    const c = o + drift;
    const h = Math.max(o, c) + r() * vol * 0.7;
    const l = Math.min(o, c) - r() * vol * 0.7;
    const v = 200 + Math.floor(r() * 800);
    out.push({ o, h, l, c, v });
    p = c;
  }
  return out;
}

function makeArea(n, seed=3, start=100, vol=0.6){
  const r = mulberry32(seed);
  const out = []; let p = start;
  for (let i = 0; i < n; i++){
    p += (r() - 0.45) * vol;
    out.push(p);
  }
  return out;
}

// ── Candle chart (SVG) ──────────────────────────────────────
function CandleChart({ candles, width = 800, height = 320, padding = { t: 20, r: 60, b: 24, l: 8 }, accentLast = true, showGrid = true, showVolume = true, lineOverlay = false }){
  const w = width - padding.l - padding.r;
  const h = height - padding.t - padding.b;
  const volH = showVolume ? Math.floor(h * 0.18) : 0;
  const priceH = h - volH - (showVolume ? 8 : 0);

  const min = Math.min(...candles.map(c => c.l));
  const max = Math.max(...candles.map(c => c.h));
  const span = (max - min) || 1;
  const pad = span * 0.06;
  const lo = min - pad, hi = max + pad;
  const yToPx = (y) => padding.t + (1 - (y - lo) / (hi - lo)) * priceH;
  const cw = w / candles.length;
  const bw = Math.max(1.2, cw * 0.62);

  // Volume scale
  const vMax = Math.max(...candles.map(c => c.v));
  const vY = (v) => padding.t + priceH + 8 + (1 - v / vMax) * volH;

  // Grid lines
  const ticks = 5;
  const grid = Array.from({ length: ticks }, (_, i) => {
    const v = lo + (hi - lo) * (i / (ticks - 1));
    return { y: yToPx(v), label: v.toFixed(2) };
  });

  // X ticks (time labels — just a few)
  const tCount = 6;
  const tTicks = Array.from({ length: tCount }, (_, i) => {
    const idx = Math.floor((candles.length - 1) * (i / (tCount - 1)));
    return { idx, x: padding.l + idx * cw + cw/2, label: `${String(9 + Math.floor(idx*6/candles.length)).padStart(2,'0')}:${String((idx*45)%60).padStart(2,'0')}` };
  });

  // Last price highlight
  const last = candles[candles.length - 1];
  const lastY = yToPx(last.c);
  const up = last.c >= last.o;

  // Optional overlay line
  const linePts = lineOverlay ? candles.map((c, i) => `${padding.l + i*cw + cw/2},${yToPx(c.c)}`).join(' ') : null;

  return (
    <svg width={width} height={height} style={{ display:'block' }}>
      {/* Grid */}
      {showGrid && grid.map((g, i) => (
        <line key={i} x1={padding.l} x2={padding.l + w} y1={g.y} y2={g.y} className="grid-line" />
      ))}
      {/* Candles */}
      {candles.map((c, i) => {
        const x = padding.l + i * cw + cw/2;
        const yo = yToPx(c.o), yc = yToPx(c.c);
        const yh = yToPx(c.h), yl = yToPx(c.l);
        const isUp = c.c >= c.o;
        const col = isUp ? 'var(--buy)' : 'var(--sell)';
        return (
          <g key={i}>
            <line x1={x} x2={x} y1={yh} y2={yl} stroke={col} strokeWidth=".7" opacity=".9"/>
            <rect x={x - bw/2} y={Math.min(yo, yc)} width={bw} height={Math.max(1, Math.abs(yc - yo))} fill={col} opacity={isUp ? .85 : .9}/>
          </g>
        );
      })}
      {/* Optional MA line */}
      {linePts && <polyline points={linePts} fill="none" stroke="var(--accent)" strokeWidth="1" opacity=".7"/>}
      {/* Volume */}
      {showVolume && candles.map((c, i) => {
        const x = padding.l + i * cw + cw/2;
        const isUp = c.c >= c.o;
        const y = vY(c.v);
        const baseY = padding.t + priceH + 8 + volH;
        return <rect key={i} x={x - bw/2} y={y} width={bw} height={baseY - y} fill={isUp ? 'var(--buy)' : 'var(--sell)'} opacity=".25"/>;
      })}
      {/* Y axis labels (right) */}
      {grid.map((g, i) => (
        <text key={i} x={padding.l + w + 6} y={g.y + 3} className="axis-text">{g.label}</text>
      ))}
      {/* X labels */}
      {tTicks.map((t, i) => (
        <text key={i} x={t.x} y={padding.t + h + 14} textAnchor="middle" className="axis-text">{t.label}</text>
      ))}
      {/* Last price marker */}
      {accentLast && (
        <g>
          <line x1={padding.l} x2={padding.l + w} y1={lastY} y2={lastY} stroke={up ? 'var(--buy)' : 'var(--sell)'} strokeDasharray="2 3" strokeWidth=".7" opacity=".8"/>
          <rect x={padding.l + w + 1} y={lastY - 8} width={padding.r - 4} height={16}
                fill={up ? 'var(--buy)' : 'var(--sell)'} rx="2"/>
          <text x={padding.l + w + padding.r/2 + 1} y={lastY + 4} textAnchor="middle"
                style={{ fill:'#0c0a08', fontFamily:'var(--mono)', fontSize:10.5, fontWeight:700 }}>{last.c.toFixed(2)}</text>
        </g>
      )}
    </svg>
  );
}

// ── Sparkline ───────────────────────────────────────────────
function Spark({ data, w = 80, h = 22, color = 'var(--ink-2)', fill = false, strokeWidth = 1 }){
  if (!data?.length) return null;
  const min = Math.min(...data), max = Math.max(...data);
  const span = (max - min) || 1;
  const pts = data.map((v, i) => {
    const x = (i / (data.length - 1)) * (w - 2) + 1;
    const y = h - 1 - ((v - min) / span) * (h - 2);
    return [x, y];
  });
  const path = pts.map((p, i) => `${i ? 'L' : 'M'}${p[0]},${p[1]}`).join('');
  const area = `${path} L${w-1},${h-1} L1,${h-1} Z`;
  return (
    <svg width={w} height={h} style={{ display:'block' }}>
      {fill && <path d={area} fill={color} opacity=".15"/>}
      <path d={path} fill="none" stroke={color} strokeWidth={strokeWidth} strokeLinecap="round"/>
    </svg>
  );
}

// ── Area chart (NVDA hero in terminal) ──────────────────────
function AreaChart({ data, width = 600, height = 200, color = 'var(--buy)', padding = { t: 8, r: 10, b: 8, l: 10 }, fillOpacity = .18 }){
  const w = width - padding.l - padding.r;
  const h = height - padding.t - padding.b;
  const min = Math.min(...data), max = Math.max(...data);
  const span = (max - min) || 1;
  const pts = data.map((v, i) => {
    const x = padding.l + (i / (data.length - 1)) * w;
    const y = padding.t + (1 - (v - min) / span) * h;
    return [x, y];
  });
  const path = pts.map((p, i) => `${i ? 'L' : 'M'}${p[0]},${p[1]}`).join('');
  const area = `${path} L${padding.l + w},${padding.t + h} L${padding.l},${padding.t + h} Z`;
  const lastP = pts[pts.length - 1];
  return (
    <svg width={width} height={height} style={{ display:'block' }}>
      <defs>
        <linearGradient id="ac-g" x1="0" x2="0" y1="0" y2="1">
          <stop offset="0%" stopColor={color} stopOpacity={fillOpacity}/>
          <stop offset="100%" stopColor={color} stopOpacity="0"/>
        </linearGradient>
      </defs>
      <path d={area} fill="url(#ac-g)"/>
      <path d={path} fill="none" stroke={color} strokeWidth="1.4"/>
      <circle cx={lastP[0]} cy={lastP[1]} r="3" fill={color}/>
      <circle cx={lastP[0]} cy={lastP[1]} r="6" fill={color} opacity=".25"/>
    </svg>
  );
}

// ── Sample symbols ──────────────────────────────────────────
const SYMS = [
  { sym:'SPY',  name:'SPDR S&P 500',     px: 737.82, ch:+2.49, qty: 500,  avg: 580.10 },
  { sym:'QQQ',  name:'Invesco QQQ',      px: 711.23, ch:+1.84 },
  { sym:'NVDA', name:'NVIDIA Corp',      px: 1284.32,ch:+2.26, qty: 450,  avg: 1256.40 },
  { sym:'AAPL', name:'Apple Inc.',       px: 221.18, ch:-0.83, qty:-200,  avg: 180.92 },
  { sym:'MSFT', name:'Microsoft',        px: 438.94, ch:+0.72, qty: 300,  avg: 415.20 },
  { sym:'TSLA', name:'Tesla Inc.',       px: 254.18, ch:-3.88, qty: 100,  avg: 244.10 },
  { sym:'AMZN', name:'Amazon',           px: 187.42, ch:+0.63 },
  { sym:'META', name:'Meta Platforms',   px: 571.99, ch:+1.85, qty:  80,  avg: 562.40 },
  { sym:'GOOGL',name:'Alphabet',         px: 174.29, ch:-0.41, qty: 200,  avg: 175.10 },
  { sym:'AMD',  name:'Adv. Micro Dev.',  px: 165.20, ch:-2.42, qty:-150,  avg: 169.20 },
  { sym:'AVGO', name:'Broadcom',         px: 182.40, ch:+1.18 },
  { sym:'NFLX', name:'Netflix',          px: 742.61, ch:+0.55 },
  { sym:'JPM',  name:'JPMorgan',         px: 230.84, ch:-0.18 },
  { sym:'XOM',  name:'Exxon Mobil',      px: 115.62, ch:+0.94 },
  { sym:'GLD',  name:'SPDR Gold',        px: 433.77, ch:+0.32 },
];

// Index ticker tape
const TAPE = [
  { sym:'NASDAQ', px:'18,642.20', ch:+0.85 },
  { sym:'DOW',    px:'42,118.40', ch:-0.12 },
  { sym:'RUSSELL',px:'2,284.66',  ch:+0.94 },
  { sym:'VIX',    px:'14.82',     ch:-3.21 },
  { sym:'10Y',    px:'4.218%',    ch:+0.04 },
  { sym:'DXY',    px:'104.18',    ch:+0.18 },
  { sym:'GOLD',   px:'2,714.40',  ch:+0.62 },
  { sym:'WTI',    px:'72.18',     ch:+2.41 },
  { sym:'BTC',    px:'97,184',    ch:+1.84 },
  { sym:'S&P 500',px:'5,824.10',  ch:+0.38 },
];

// Build a DOM ladder around a price
function makeLadder(centerPx, n = 18, seed = 9){
  const r = mulberry32(seed);
  const tick = 0.01;
  const half = Math.floor(n/2);
  const rows = [];
  for (let i = -half; i <= half; i++){
    const px = +(centerPx + i * tick).toFixed(2);
    const isMid = (i === 0);
    const side = i > 0 ? 'ask' : i < 0 ? 'bid' : 'mid';
    const base = 1500 + Math.floor(r()*4500);
    const delta = Math.floor((r() - 0.5) * 12000);
    rows.push({ px, side, vol: base, delta, depth: Math.min(1, base/6000) });
  }
  return rows;
}

// Format helpers
function fmtCh(n, withSign = true){
  const s = n >= 0 ? '+' : '';
  return `${withSign ? s : ''}${n.toFixed(2)}%`;
}
function fmtNum(n){ return n.toLocaleString('en-US', { minimumFractionDigits:2, maximumFractionDigits:2 }); }

Object.assign(window, {
  mulberry32, makeCandles, makeArea, makeLadder,
  CandleChart, Spark, AreaChart,
  SYMS, TAPE, fmtCh, fmtNum
});
