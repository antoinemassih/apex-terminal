/* ===== Trade view — DOM ladder card · chart card · cream watchlist ===== */
const { useState: useStateT, useMemo: useMemoT } = React;

function TradeView({ setView, setTicketOpen }) {
  return (
    <div className="trade">
      <DomCard onTicket={() => setTicketOpen(true)} />
      <CenterChart onTicket={() => setTicketOpen(true)} />
    </div>
  );
}

function DomCard({ onTicket }) {
  const ladder = useMemoT(() => {
    const rows = [];
    for (let i = 12; i >= -12; i--) {
      const px = +(1284.32 + i * 0.05).toFixed(2);
      const sz = Math.round(40 + Math.random() * 480);
      rows.push({ px, sz, last: i === 0, side: i > 0 ? "ask" : i < 0 ? "bid" : "last" });
    }
    return rows;
  }, []);
  const maxSz = Math.max(...ladder.map(r => r.sz));
  return (
    <div className="dom-card">
      <div className="dom-head">
        <span className="ttl">DOM ladder</span>
        <span className="sym">NVDA · 25 deep</span>
      </div>
      <div className="dom-rows">
        {ladder.map((r,i) => (
          <div key={i} className={"dom-row "+r.side+(r.last?" last":"")}>
            {!r.last && <div className="bar" style={{
              [r.side==="ask"?"left":"right"]: 8,
              width: (r.sz/maxSz*70) + "%"
            }}></div>}
            <span className="b">{r.side==="bid"?r.sz:""}</span>
            <span className="px">{r.px.toFixed(2)}</span>
            <span className="a">{r.side==="ask"?r.sz:""}</span>
          </div>
        ))}
      </div>
      <div className="dom-cta">
        <button className="buy" onClick={onTicket}>Buy ↑</button>
        <button className="sell" onClick={onTicket}>Sell ↓</button>
      </div>
    </div>
  );
}

function CenterChart({ onTicket }) {
  const [tf, setTf] = useStateT("15m");
  const tfs = ["1m","5m","15m","1H","4H","1D"];
  const data = useMemoT(() => {
    let p = 1200;
    const arr = [];
    for (let i = 0; i < 110; i++) {
      const o = p;
      const c = p + (Math.random() - 0.42) * 5;
      const h = Math.max(o,c) + Math.random() * 2;
      const l = Math.min(o,c) - Math.random() * 2;
      arr.push({ o, h, l, c, v: 0.3 + Math.random()*0.7 });
      p = c;
    }
    arr[arr.length-1].c = 1284.32;
    return arr;
  }, []);
  return (
    <div className="center">
      <div className="last-strip">
        <span className="sym">NVDA</span>
        <div>
          <div className="px">1,284.<span style={{fontSize:36}}>32</span></div>
          <div className="ch">+28.40 · +2.26%</div>
        </div>
        <div className="ohlc">
          <span>O<b>1,256.40</b></span>
          <span>H<b>1,392.72</b></span>
          <span>L<b>1,252.29</b></span>
          <span>V<b>32.4M</b></span>
        </div>
        <button onClick={onTicket} style={{
          height:44, padding:"0 18px", borderRadius:999,
          background:"var(--on-light)", color:"var(--orange)",
          fontWeight:700, letterSpacing:"0.06em", fontSize:12
        }}>+ NEW TICKET ⌘K</button>
      </div>
      <div className="chart-card">
        <div className="chart-head">
          <div className="left">
            <div className="pillset">
              {tfs.map(t => (
                <button key={t} className={t===tf?"on":""} onClick={() => setTf(t)}>{t}</button>
              ))}
            </div>
          </div>
          <div className="right">
            <button className="dot-btn">∿</button>
            <button className="dot-btn">⊞</button>
            <button className="dot-btn">◔</button>
          </div>
        </div>
        <div className="chart-canvas">
          <Candles data={data} />
          <Axis data={data} />
          <div className="float-ring" style={{ left: 24, top: 16 }}>
            <span className="l">INTRADAY</span>
            <span className="v">+2.26%</span>
          </div>
          <div className="float-ring cream" style={{ right: 88, top: 16 }}>
            <span className="l">SENTIMENT</span>
            <span className="v">68</span>
          </div>
        </div>
      </div>
    </div>
  );
}

function Candles({ data }) {
  const W = 1200, H = 360, pl = 8, pr = 64, pt = 8, pb = 70;
  const cw = W - pl - pr, ch = H - pt - pb;
  const hi = Math.max(...data.map(d => d.h));
  const lo = Math.min(...data.map(d => d.l));
  const x = i => pl + (i + 0.5) * (cw / data.length);
  const y = v => pt + (1 - (v - lo) / (hi - lo)) * ch;
  const cw1 = cw / data.length;
  const bodyW = Math.max(2, cw1 * 0.62);
  const ma = data.map((_, i) => {
    const span = data.slice(Math.max(0, i-19), i+1);
    return span.reduce((s,d) => s + d.c, 0) / span.length;
  });
  const ma50 = data.map((_, i) => {
    const span = data.slice(Math.max(0, i-49), i+1);
    return span.reduce((s,d) => s + d.c, 0) / span.length;
  });
  const vMax = Math.max(...data.map(d => d.v));
  const vTop = H - pb + 6, vBot = H - 4;
  const last = data[data.length-1].c;

  const [hover, setHover] = React.useState(null); // index
  const svgRef = React.useRef(null);
  const onMove = (e) => {
    const r = svgRef.current.getBoundingClientRect();
    const xPx = ((e.clientX - r.left) / r.width) * W;
    const yPx = ((e.clientY - r.top) / r.height) * H;
    if (xPx < pl || xPx > W - pr || yPx < pt || yPx > pt + ch) { setHover(null); return; }
    const idx = Math.min(data.length-1, Math.max(0, Math.round((xPx - pl)/cw1 - 0.5)));
    setHover(idx);
  };
  const hoverPx = hover !== null ? lo + (1 - (((hover !== null ? null : 0))) ) : null; // unused
  const cursorVal = hover !== null && svgRef.current ? (() => {
    return null;
  })() : null;

  return (
    <div className="chart-svg-wrap" onMouseLeave={() => setHover(null)}>
      <svg ref={svgRef} viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none"
           onMouseMove={onMove}>
        {/* horizontal grid */}
        {[0.2,0.4,0.6,0.8].map((t,i) => (
          <line key={i} x1={pl} x2={W-pr} y1={pt+t*ch} y2={pt+t*ch}
                stroke="rgba(255,255,255,0.04)"/>
        ))}
        {/* Filled candlesticks — sharp corners */}
        {data.map((d,i) => {
          const isUp = d.c >= d.o;
          const cx = x(i);
          const yo = y(d.o), yc = y(d.c);
          const top = Math.min(yo, yc);
          const h = Math.max(1, Math.abs(yc - yo));
          const stroke = isUp ? "var(--green)" : "var(--red)";
          const fill = isUp ? "var(--green)" : "var(--red)";
          return (
            <g key={i}>
              {/* high-low wick */}
              <line x1={cx} x2={cx} y1={y(d.h)} y2={y(d.l)} stroke={stroke} strokeWidth="1" />
              {/* body — sharp rectangle */}
              <rect x={cx - bodyW/2} y={top} width={bodyW} height={h}
                    fill={fill} stroke={stroke} strokeWidth="0.5" shapeRendering="crispEdges"/>
            </g>
          );
        })}
        {/* MA20 — solid orange */}
        <polyline fill="none" stroke="var(--orange)" strokeWidth="1.4" opacity="0.95"
                  points={ma.map((v,i) => `${x(i)},${y(v)}`).join(" ")}/>
        {/* MA50 — dashed cream */}
        <polyline fill="none" stroke="var(--cream)" strokeWidth="1" opacity="0.55"
                  strokeDasharray="3 4"
                  points={ma50.map((v,i) => `${x(i)},${y(v)}`).join(" ")}/>
        {/* Volume — thin stems */}
        {data.map((d,i) => {
          const vh = (d.v / vMax) * (vBot - vTop);
          const isUp = d.c >= d.o;
          return (
            <line key={"v"+i} x1={x(i)} x2={x(i)} y1={vBot} y2={vBot - vh}
                  stroke={isUp?"var(--green)":"var(--red)"} strokeWidth="1.4" opacity="0.55"/>
          );
        })}
        {/* last price line */}
        <line x1={pl} x2={W-pr} y1={y(last)} y2={y(last)}
              stroke="var(--orange)" strokeDasharray="3 4" strokeWidth="1"/>
        {/* Crosshair */}
        {hover !== null && (() => {
          const d = data[hover];
          const cx = x(hover);
          const cy = y(d.c);
          return (
            <g pointerEvents="none">
              <line x1={cx} x2={cx} y1={pt} y2={pt+ch}
                    stroke="var(--ink-3)" strokeWidth="0.5" strokeDasharray="2 3" opacity="0.7"/>
              <line x1={pl} x2={W-pr} y1={cy} y2={cy}
                    stroke="var(--ink-3)" strokeWidth="0.5" strokeDasharray="2 3" opacity="0.7"/>
              <circle cx={cx} cy={cy} r="2.5" fill="var(--orange)"/>
              {/* y-axis price tag */}
              <g transform={`translate(${W-pr+2}, ${cy-9})`}>
                <rect x="0" y="0" width="56" height="18" fill="var(--orange)" rx="2"/>
                <text x="4" y="12" fill="var(--on-light)" fontFamily="var(--f-mono)" fontSize="10" fontWeight="600">
                  {d.c.toFixed(2)}
                </text>
              </g>
            </g>
          );
        })()}
      </svg>
      {/* HTML tooltip card */}
      {hover !== null && (() => {
        const d = data[hover];
        const isUp = d.c >= d.o;
        const ch1 = ((d.c - d.o) / d.o * 100);
        const xPct = ((x(hover) - pl) / cw) * 100;
        const place = xPct > 60 ? "left" : "right";
        return (
          <div className={"chart-tip "+place}
               style={place==="right" ? { left: `calc(${xPct}% + 14px)` } : { right: `calc(${100-xPct}% + 14px)` }}>
            <div className="ct-time">15m · 14:30 ET</div>
            <div className="ct-row"><span className="k">O</span><span className="v">{d.o.toFixed(2)}</span></div>
            <div className="ct-row"><span className="k">H</span><span className="v">{d.h.toFixed(2)}</span></div>
            <div className="ct-row"><span className="k">L</span><span className="v">{d.l.toFixed(2)}</span></div>
            <div className="ct-row"><span className="k">C</span><span className={"v "+(isUp?"up":"dn")}>{d.c.toFixed(2)}</span></div>
            <div className="ct-row"><span className="k">Δ</span><span className={"v "+(isUp?"up":"dn")}>{(ch1>=0?"+":"")+ch1.toFixed(2)}%</span></div>
            <div className="ct-row"><span className="k">VOL</span><span className="v mono">{(d.v/1000).toFixed(0)}k</span></div>
          </div>
        );
      })()}
    </div>
  );
}

function Axis({ data }) {
  const hi = Math.max(...data.map(d => d.h));
  const lo = Math.min(...data.map(d => d.l));
  const last = data[data.length-1].c;
  const ratio = (last - lo) / (hi - lo);
  const ticks = 6;
  return (
    <div className="chart-axis">
      {Array.from({ length: ticks }).map((_,i) => {
        const t = i / (ticks-1);
        const v = lo + (1 - t) * (hi - lo);
        return <span key={i} className="tick" style={{ top: `${t*78 + 4}%` }}>
          {v.toLocaleString(undefined, { maximumFractionDigits: 2 })}
        </span>;
      })}
      <span className="last-tag" style={{ top: `${(1-ratio)*78 + 4}%` }}>
        {last.toFixed(2)}
      </span>
    </div>
  );
}

function RightRail({ onTicket }) {
  const wl = [
    { sym: "NVDA", name: "NVIDIA",     px: "1,284", ch: "+2.26%", up: true,  active: true },
    { sym: "AAPL", name: "Apple",      px: "221",   ch: "-0.83%", up: false },
    { sym: "MSFT", name: "Microsoft",  px: "438",   ch: "+0.72%", up: true  },
    { sym: "TSLA", name: "Tesla",      px: "254",   ch: "-3.88%", up: false },
    { sym: "META", name: "Meta",       px: "571",   ch: "+1.85%", up: true  },
    { sym: "AMD",  name: "AMD",        px: "165",   ch: "-2.42%", up: false },
    { sym: "AMZN", name: "Amazon",     px: "185",   ch: "+0.32%", up: true  },
    { sym: "SPY",  name: "S&P 500",    px: "582",   ch: "+0.38%", up: true  },
  ];
  const opts = [
    { side: "C", strike: "1290 0DTE", bid: "2.97", ask: "2.99" },
    { side: "C", strike: "1295 0DTE", bid: "1.66", ask: "1.69" },
    { side: "P", strike: "1280 0DTE", bid: "3.10", ask: "3.13" },
    { side: "P", strike: "1275 0DTE", bid: "5.32", ask: "5.40" },
  ];
  return (
    <div className="rail">
      <div className="watch-card">
        <div className="head">
          <span>Watchlist</span>
          <span style={{marginLeft:"auto", fontSize:11, opacity:0.55}}>8 syms</span>
          <button className="more" style={{position:"static", marginLeft:8}}>···</button>
        </div>
        {wl.map(w => (
          <div key={w.sym} className={"watch-row "+(w.active?"active":"")} onClick={onTicket}>
            <span className="sym">{w.sym}</span>
            <span className="name">{w.name}</span>
            <span className="px">{w.px}</span>
            <span className={"ch "+(w.up?"up":"dn")}>{w.ch}</span>
          </div>
        ))}
      </div>
      <div className="chain-card">
        <div className="head">
          <span>Options · NVDA · 0DTE</span>
        </div>
        {opts.map((o,i) => (
          <div key={i} className="chain-row">
            <span className={"ind "+o.side.toLowerCase()}>{o.side}</span>
            <span className="strike">{o.strike}</span>
            <span className="bid">{o.bid}</span>
            <span className="ask">{o.ask}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

window.TradeView = TradeView;
