/* ===== Live tiles — denser mosaic, varied shapes, no cropping ===== */
const { useMemo: useMemoTI } = React;

function TilesView({ setView, setTicketOpen }) {
  return (
    <div className="tiles">
      <div className="tiles-h">
        <b>Live tiles</b>
        <span className="pill sm outline">Default workspace</span>
        <span className="pill sm outline">Auto-refresh · 1s</span>
        <div className="right">
          <button className="pill sm outline" style={{ color: "var(--ink-2)", borderColor: "var(--line-2)" }}>+ Add tile</button>
          <button className="pill sm solid orange" onClick={() => setTicketOpen(true)}>+ New ticket</button>
        </div>
      </div>

      <div className="tile-grid">

        {/* ===== Row 1 (h=2) — Hero P&L · NVDA OHLC chart · Sentiment circle · Top mover · NYSE clock ===== */}
        <HeroPL />
        <NvdaChartTile />
        <SentimentCircle />
        <NyseClockTile />

        {/* ===== Row 2 (h=2) — index strip ===== */}
        <IndexTile cls="green"  badge="S&P 500" v="+0.38%" sub="5,824.10" trend />
        <IndexTile cls="paper"  badge="NASDAQ"  v="+0.85%" sub="18,642.20" trend />
        <IndexTile cls="orange" badge="VIX"     v="−3.21%" sub="14.32 · risk off" />
        <IndexTile cls="black"  dark badge="DXY" v="+0.18%" sub="104.18" />
        <IndexTile cls="cream"  badge="GOLD"    v="+0.42%" sub="2,714.40" />
        <IndexTile cls="yellow" badge="10Y · TNX" v="+4bp" sub="4.218%" />

        {/* ===== Row 3 (h=2) — Watchlist tile (wide) · Sector circle · VaR · Realized · Win-rate circle ===== */}
        <WatchlistTile />
        <SectorRingTile />
        <SmallStat cls="orange" badge="VaR · 95%" v="$48k" sub="1.7% NAV" />
        <SmallStat cls="paper"  badge="Realized"   v="+$48k" sub="today" />
        <WinrateCircle />

        {/* ===== Row 4 (h=2) — Liquidity heat · Factor scorecard · TSLA ohlc mini · MSFT ohlc mini ===== */}
        <LiquidityHeat />
        <FactorScorecard />
        <MiniOhlcTile sym="TSLA" name="Tesla" px="256.83" ch="+5.21%" up={true} cls="cream" />
        <MiniOhlcTile sym="AMD"  name="AMD"   px="165.20" ch="-2.42%" up={false} cls="black" dark />

        {/* ===== Row 5 (h=2) — Engagement (block bars) · Lollipop trading hours · Allocation stack ===== */}
        <EngagementBlocks />
        <LollipopHours />
        <AllocationTile />

        {/* ===== Row 6 (h=1) — fast pill row of mini KPIs ===== */}
        <KpiPill cls="green"   l="Open positions" v="12" />
        <KpiPill cls="orange"  l="Pending"        v="2"  />
        <KpiPill cls="paper"   l="Avg hold"       v="2.4d" />
        <KpiPill cls="cream"   l="Largest"        v="NVDA · 18%" />
        <KpiPill cls="yellow"  l="Slippage today" v="0.4 bp" />
        <KpiPill cls="black"   dark l="Brokerage" v="IBKR · MARGIN" />

        {/* ===== Row 7 (h=2) — News feed · Risk gauges (3 circles) · Movers list ===== */}
        <NewsTile />
        <ThreeCircles />
        <MoversTile />

        {/* ===== Action strip ===== */}
        <ActionStrip setTicketOpen={setTicketOpen} />
      </div>
    </div>
  );
}

/* ============================================================
   Tiles
============================================================ */

function HeroPL(){
  return (
    <div className="tile orange span-4 row-2 with-badge">
      <span className="badge-pill">Today's P&L</span>
      <button className="more">⋯</button>
      <div style={{ position:"absolute", top:14, right:54, display:"flex", gap:4 }}>
        <span className="chip solid-blk">1D</span>
        <span className="chip">1W</span>
        <span className="chip">1M</span>
      </div>
      <div className="val" style={{ fontSize: 86 }}>+$290,794</div>
      <div style={{ display:"flex", gap:14, marginTop: 10, flexWrap:"wrap" }}>
        <span className="chip">REALIZED <b style={{marginLeft:4, color:"var(--on-light)"}}>+$48,210</b></span>
        <span className="chip">UNREALIZED <b style={{marginLeft:4, color:"var(--on-light)"}}>+$242,584</b></span>
        <span className="chip">WIN-RATE <b style={{marginLeft:4, color:"var(--on-light)"}}>72%</b></span>
      </div>
    </div>
  );
}

function NvdaChartTile(){
  const data = useMemoTI(() => {
    let p = 1240; const arr = [];
    for (let i = 0; i < 60; i++) {
      const o = p, c = p + (Math.random()-0.45)*5;
      arr.push({ o, h: Math.max(o,c)+Math.random()*1.6, l: Math.min(o,c)-Math.random()*1.6, c });
      p = c;
    }
    return arr;
  }, []);
  return (
    <div className="tile coal span-4 row-2" style={{ padding: 14 }}>
      <div style={{ display:"flex", alignItems:"baseline", gap:10 }}>
        <span style={{ fontWeight:700, fontSize:13, letterSpacing:"0.04em" }}>NVDA</span>
        <span className="mono" style={{ fontSize:12, color:"var(--ink)" }}>1,284.32</span>
        <span className="chip up" style={{ background:"rgba(78,192,122,0.12)" }}>+2.26%</span>
        <span style={{ marginLeft:"auto", display:"flex", gap:4 }}>
          {["1H","4H","1D"].map((t,i) => (
            <span key={t} className={"chip"+(i===2?" solid-blk":"")}>{t}</span>
          ))}
        </span>
      </div>
      <div style={{ flex:1, marginTop:8, position:"relative", minHeight: 0 }}>
        <MiniOhlcSvg data={data} />
      </div>
      <div style={{ display:"flex", justifyContent:"space-between", fontFamily:"var(--f-mono)", fontSize:10, color:"var(--ink-3)", marginTop:4 }}>
        <span>O 1,256 · H 1,392 · L 1,252</span>
        <span>VOL 32.4M</span>
      </div>
    </div>
  );
}

function MiniOhlcSvg({ data }){
  const W = 400, H = 130, pl = 4, pr = 4, pt = 4, pb = 14;
  const cw = W-pl-pr, ch = H-pt-pb;
  const hi = Math.max(...data.map(d=>d.h));
  const lo = Math.min(...data.map(d=>d.l));
  const x = i => pl + (i+0.5)*(cw/data.length);
  const y = v => pt + (1-(v-lo)/(hi-lo))*ch;
  const tw = (cw/data.length)*0.32;
  return (
    <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" style={{ width:"100%", height:"100%" }}>
      {data.map((d,i) => {
        const up = d.c >= d.o;
        const cx = x(i);
        const c = up ? "var(--green)" : "var(--red)";
        return (
          <g key={i} stroke={c} strokeWidth="1" strokeLinecap="butt">
            <line x1={cx} x2={cx} y1={y(d.h)} y2={y(d.l)}/>
            <line x1={cx-tw} x2={cx} y1={y(d.o)} y2={y(d.o)}/>
            <line x1={cx} x2={cx+tw} y1={y(d.c)} y2={y(d.c)}/>
          </g>
        );
      })}
    </svg>
  );
}

function SentimentCircle(){
  const v = 68;
  const r = 56, c = 2*Math.PI*r;
  return (
    <div className="tile green circle row-2 span-2" style={{ alignItems:"center", justifyContent:"center" }}>
      <span style={{ fontSize: 10, letterSpacing:"0.14em", fontWeight: 700, opacity:0.7, marginBottom: 4 }}>SENTIMENT · 24H</span>
      <div style={{ position:"relative", width:130, height:130 }}>
        <svg viewBox="0 0 140 140" width="130" height="130">
          <circle cx="70" cy="70" r={r} fill="none" stroke="rgba(0,0,0,0.18)" strokeWidth="9"/>
          <circle cx="70" cy="70" r={r} fill="none" stroke="var(--on-light)" strokeWidth="9"
                  strokeDasharray={`${(v/100)*c} ${c}`} strokeLinecap="round"
                  transform="rotate(-90 70 70)"/>
        </svg>
        <div style={{ position:"absolute", inset:0, display:"flex", flexDirection:"column", alignItems:"center", justifyContent:"center" }}>
          <div style={{ fontSize:48, fontWeight:500, letterSpacing:"-0.03em", lineHeight:1 }}>{v}</div>
          <div style={{ fontSize:9, marginTop:2, fontWeight:700, letterSpacing:"0.1em" }}>GREEDY</div>
        </div>
      </div>
    </div>
  );
}

function NyseClockTile(){
  return (
    <div className="tile yellow span-2 row-2" style={{ padding: 14, position:"relative" }}>
      <span className="badge-pill">NYSE · OPEN</span>
      <button className="more">⋯</button>
      <div style={{ display:"flex", alignItems:"flex-end", gap:8, marginTop:"auto" }}>
        <div style={{ display:"flex", flexDirection:"column", fontSize:10, fontWeight:700, color:"var(--orange)", lineHeight:1.2 }}>
          <span>AM</span><span style={{ color:"var(--on-light)" }}>PM</span>
        </div>
        <div style={{ fontSize:54, fontWeight:500, letterSpacing:"-0.05em", lineHeight:1 }}>02:14</div>
      </div>
      <div style={{ fontSize:10.5, marginTop:4, opacity:0.65 }}>3h 51m to close</div>
    </div>
  );
}

function IndexTile({ cls, dark, badge, v, sub, trend }){
  return (
    <div className={"tile tight with-badge span-2 "+cls+(dark?" dark":"")}>
      <span className="badge-pill">{badge}</span>
      <button className="more">⋯</button>
      <div className="val" style={{ marginTop:"auto" }}>{v}</div>
      <div className="meta">{sub}</div>
    </div>
  );
}

function SmallStat({ cls, dark, badge, v, sub }){
  return (
    <div className={"tile tight with-badge span-2 row-2 "+cls+(dark?" dark":"")}>
      <span className="badge-pill">{badge}</span>
      <button className="more">⋯</button>
      <div className="val" style={{ fontSize: 56 }}>{v}</div>
      <div className="meta">{sub}</div>
    </div>
  );
}

function WatchlistTile(){
  const wl = [
    ["NVDA","NVIDIA Corp","1,284.32","+2.26%",true,true],
    ["AAPL","Apple Inc","221.18","-0.83%",false],
    ["MSFT","Microsoft Corp","438.94","+0.72%",true],
    ["TSLA","Tesla Inc","254.18","-3.88%",false],
    ["AMZN","Amazon.com","187.42","+0.63%",true],
    ["META","Meta Platforms","571.90","+1.50%",true],
  ];
  return (
    <div className="tile cream span-5 row-2" style={{ padding: "14px 16px" }}>
      <div style={{ display:"flex", alignItems:"baseline", marginBottom: 6 }}>
        <span style={{ fontWeight:700, fontSize:13 }}>Watchlist</span>
        <span style={{ marginLeft:"auto", fontSize:10, opacity:0.55, fontFamily:"var(--f-mono)", letterSpacing:"0.06em" }}>TOP 6</span>
      </div>
      <table className="micro-tbl" style={{ flex:1 }}>
        <thead><tr>
          <th style={{ width: 50 }}>SYM</th>
          <th>NAME</th>
          <th className="r">LAST</th>
          <th className="r" style={{ width: 60 }}>CHG%</th>
        </tr></thead>
        <tbody>
          {wl.map(([s,n,p,c,up,active],i) => (
            <tr key={i}>
              <td style={{ fontWeight:700, color: active?"var(--orange)":"var(--on-light)" }}>{s}</td>
              <td style={{ color:"rgba(26,22,18,0.55)" }}>{n}</td>
              <td className="r">{p}</td>
              <td className={"r "+(up?"up":"dn")}>{c}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function SectorRingTile(){
  /* Sector exposure — 3 nested arcs */
  const segs = [
    [{ c:"#f5c64a", v: 42 }, { c:"#ef5b3b", v: 19 }, { c:"#4ec07a", v: 14 }, { c:"#7c5cf3", v: 12 }, { c:"#d8503e", v: 8 }, { c:"#1a1612", v: 5 }],
  ];
  let off = 0;
  const r = 42, c = 2*Math.PI*r;
  return (
    <div className="tile coal span-3 row-2" style={{ padding: 14 }}>
      <div style={{ display:"flex", alignItems:"baseline", marginBottom: 8 }}>
        <span style={{ fontWeight:700, fontSize:13 }}>Sector exposure</span>
        <span style={{ marginLeft:"auto", fontSize:9, color:"var(--ink-3)", fontFamily:"var(--f-mono)", letterSpacing:"0.1em" }}>NAV %</span>
      </div>
      <div style={{ display:"flex", gap: 12, alignItems:"center", flex:1 }}>
        <div style={{ position:"relative", width: 110, height: 110, flex: "0 0 auto" }}>
          <svg viewBox="0 0 110 110" width="110" height="110">
            <circle cx="55" cy="55" r={r} fill="none" stroke="rgba(255,255,255,0.05)" strokeWidth="14"/>
            {segs[0].map((s,i) => {
              const len = (s.v/100)*c;
              const dash = `${len} ${c-len}`;
              const off2 = -off;
              off += len;
              return <circle key={i} cx="55" cy="55" r={r} fill="none" stroke={s.c} strokeWidth="14"
                             strokeDasharray={dash} strokeDashoffset={off2}
                             transform="rotate(-90 55 55)"/>;
            })}
          </svg>
        </div>
        <div style={{ display:"flex", flexDirection:"column", gap:5, fontSize:10.5, fontFamily:"var(--f-mono)" }}>
          {[
            ["#f5c64a","Tech","42"],
            ["#ef5b3b","Comm","19"],
            ["#4ec07a","Disc","14"],
            ["#7c5cf3","Fin","12"],
            ["#d8503e","HC","8"],
            ["rgba(255,255,255,0.5)","Cash","5"],
          ].map(([c,l,v]) => (
            <span key={l} style={{ display:"inline-flex", alignItems:"center", gap:6, color:"var(--ink-2)" }}>
              <i style={{ width:8, height:8, background:c, borderRadius:2, display:"inline-block" }}></i>
              <b style={{ color:"var(--ink)", fontWeight:600 }}>{l}</b> {v}
            </span>
          ))}
        </div>
      </div>
    </div>
  );
}

function WinrateCircle(){
  const v = 72;
  const r = 46, c = 2*Math.PI*r;
  return (
    <div className="tile orange circle row-2 span-2">
      <span style={{ fontSize: 10, letterSpacing:"0.14em", fontWeight: 700, opacity:0.7, marginBottom: 4 }}>WIN-RATE</span>
      <div style={{ position:"relative", width:120, height:120 }}>
        <svg viewBox="0 0 120 120" width="120" height="120">
          <circle cx="60" cy="60" r={r} fill="none" stroke="rgba(0,0,0,0.18)" strokeWidth="8"/>
          <circle cx="60" cy="60" r={r} fill="none" stroke="var(--on-light)" strokeWidth="8"
                  strokeDasharray={`${(v/100)*c} ${c}`} strokeLinecap="round"
                  transform="rotate(-90 60 60)"/>
        </svg>
        <div style={{ position:"absolute", inset:0, display:"flex", flexDirection:"column", alignItems:"center", justifyContent:"center" }}>
          <div style={{ fontSize:42, fontWeight:500, letterSpacing:"-0.03em", lineHeight:1 }}>{v}<span style={{ fontSize: 18, marginLeft: 1 }}>%</span></div>
          <div style={{ fontSize:8.5, marginTop:2, fontWeight:700, letterSpacing:"0.1em" }}>L50 TRADES</div>
        </div>
      </div>
    </div>
  );
}

function LiquidityHeat(){
  const cells = useMemoTI(() => Array.from({length: 14*5}, () => {
    const r = Math.random();
    if (r < 0.04) return "orange";
    if (r < 0.10) return "red";
    if (r < 0.40) return "white";
    return "dim";
  }), []);
  return (
    <div className="tile coal span-4 row-2" style={{ padding: 14 }}>
      <div style={{ display:"flex", alignItems:"baseline", marginBottom: 6 }}>
        <span style={{ fontWeight:700, fontSize:13 }}>Liquidity matrix</span>
        <span style={{ marginLeft:"auto", fontSize:9, color:"var(--ink-3)", fontFamily:"var(--f-mono)", letterSpacing:"0.1em" }}>14×5 · TOP 196</span>
      </div>
      <div className="heat" style={{ flex: 1 }}>
        {cells.map((c,i) => {
          const map = {
            orange: "var(--orange)",
            red: "var(--red)",
            white: "rgba(255,255,255,0.85)",
            dim: "rgba(255,255,255,0.06)",
          };
          return <div key={i} className="c" style={{ background: map[c] }}></div>;
        })}
      </div>
      <div style={{ display:"flex", gap:14, marginTop:6, fontSize:10, color:"var(--ink-3)", fontFamily:"var(--f-mono)" }}>
        <span><i style={{ width:8, height:8, background:"var(--orange)", display:"inline-block", marginRight:4 }}></i>HIGH</span>
        <span><i style={{ width:8, height:8, background:"rgba(255,255,255,0.85)", display:"inline-block", marginRight:4 }}></i>NORM</span>
        <span><i style={{ width:8, height:8, background:"var(--red)", display:"inline-block", marginRight:4 }}></i>THIN</span>
      </div>
    </div>
  );
}

function FactorScorecard(){
  const factors = [
    ["Momentum", 78, "var(--green)"],
    ["Quality",  62, "var(--yellow)"],
    ["Value",    34, "var(--red)"],
    ["Size",     51, "var(--orange)"],
    ["Vol",      88, "var(--violet)"],
  ];
  return (
    <div className="tile coal span-3 row-2" style={{ padding: 14 }}>
      <div style={{ display:"flex", alignItems:"baseline", marginBottom: 8 }}>
        <span style={{ fontWeight:700, fontSize:13 }}>Factor scorecard</span>
        <span style={{ marginLeft:"auto", fontSize:9, color:"var(--ink-3)", fontFamily:"var(--f-mono)", letterSpacing:"0.1em" }}>NORM 0–100</span>
      </div>
      <div style={{ display:"flex", flexDirection:"column", gap: 8, flex:1, justifyContent:"center" }}>
        {factors.map(([l,v,c]) => (
          <div key={l} style={{ display:"grid", gridTemplateColumns:"68px 1fr 32px", alignItems:"center", gap:10 }}>
            <span style={{ fontSize: 11, color:"var(--ink-2)" }}>{l}</span>
            <div style={{ height: 6, borderRadius: 3, background:"rgba(255,255,255,0.05)", position:"relative", overflow:"hidden" }}>
              <div style={{ position:"absolute", left:0, top:0, bottom:0, width: v+"%", background: c, borderRadius: 3 }}></div>
            </div>
            <span style={{ fontFamily:"var(--f-mono)", fontSize:10.5, color:"var(--ink)", textAlign:"right" }}>{v}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function MiniOhlcTile({ sym, name, px, ch, up, cls, dark }){
  const data = useMemoTI(() => {
    let p = 100; const arr = [];
    for (let i = 0; i < 36; i++) {
      const o = p, c = p + (Math.random()-0.45)*4;
      arr.push({ o, h: Math.max(o,c)+Math.random()*1.4, l: Math.min(o,c)-Math.random()*1.4, c });
      p = c;
    }
    return arr;
  }, [sym]);
  return (
    <div className={"tile span-2 row-2 "+cls+(dark?" dark":"")} style={{ padding: 12 }}>
      <div style={{ display:"flex", alignItems:"baseline" }}>
        <span style={{ fontWeight:700, fontSize:13 }}>{sym}</span>
        <span className={"chip "+(up?"up":"dn")} style={{ marginLeft:"auto" }}>{ch}</span>
      </div>
      <div style={{ fontSize: 10, opacity: 0.55 }}>{name}</div>
      <div style={{ flex:1, marginTop:6, minHeight: 0 }}>
        <MiniOhlcSvg data={data} />
      </div>
      <div style={{ display:"flex", justifyContent:"space-between", fontFamily:"var(--f-mono)", fontSize: 11, marginTop: 4 }}>
        <b>{px}</b>
        <span style={{ opacity: 0.55 }}>vol 32M</span>
      </div>
    </div>
  );
}

function EngagementBlocks(){
  return (
    <div className="tile yellow span-3 row-2" style={{ padding: 14 }}>
      <div style={{ display:"flex", alignItems:"flex-start" }}>
        <span className="badge-pill">Conviction</span>
        <button className="more" style={{ background:"transparent", border:"1.5px solid var(--on-light)" }}>⋯</button>
      </div>
      <div style={{ fontSize: 9.5, opacity:0.55, marginTop: 4, lineHeight: 1.3 }}>AI-blended order-flow</div>
      <div style={{ fontSize: 50, fontWeight:500, letterSpacing:"-0.04em", lineHeight:1, marginTop:4 }}>+17.4%</div>
      <div style={{ flex:1, display:"flex", alignItems:"flex-end", gap:0, marginTop:6, position:"relative", minHeight: 0 }}>
        <div style={{ width:"58%", height:"75%", background:"var(--on-light)" }}></div>
        <div style={{ width:"30%", height:"45%", background:"rgba(0,0,0,0.25)" }}></div>
      </div>
      <div style={{ display:"flex", justifyContent:"space-between", marginTop:6, fontSize:10 }}>
        <b>+18.5%</b>
        <span style={{ opacity:0.55 }}>Est +15.7%</span>
      </div>
    </div>
  );
}

function LollipopHours(){
  const data = [12,18,9,4,3,28,11,16,15,7,6,9];
  const labels = ["J","F","M","A","M","J","J","A","S","O","N","D"];
  const max = Math.max(...data);
  return (
    <div className="tile cream span-4 row-2" style={{ padding: 0, overflow:"hidden" }}>
      <div style={{ background:"var(--orange)", color:"var(--on-light)", padding:"8px 14px",
        display:"flex", gap:10, fontSize:10, letterSpacing:"0.1em", fontWeight:700 }}>
        <span>MERIDIAN</span>
        <span style={{ marginLeft:"auto" }}>HOURS · 12M</span>
      </div>
      <div style={{ padding:"10px 14px 12px", display:"flex", flexDirection:"column", flex:1, minHeight: 0 }}>
        <div style={{ display:"flex", alignItems:"baseline", justifyContent:"space-between" }}>
          <span style={{ fontSize: 10, fontWeight: 700, letterSpacing:"0.08em" }}>HOURS TRADED</span>
          <span className="chip solid-blk">+8% YOY</span>
        </div>
        <div style={{ fontSize: 44, fontWeight:500, letterSpacing:"-0.04em", lineHeight:1, marginTop:2 }}>554h</div>
        <div className="lolli" style={{ color:"var(--orange)", flex:1, marginTop:4 }}>
          {data.map((v,i) => (
            <div key={i} className={"bar"+(v===max?" tall":"")} style={{ height:"100%" }}>
              <div className="stem" style={{ height:(v/max*100)+"%" }}></div>
              <div className="head"></div>
              <span className="label">{labels[i]}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function AllocationTile(){
  return (
    <div className="tile black dark span-5 row-2" style={{ padding: 14 }}>
      <div style={{ display:"flex", alignItems:"baseline", marginBottom: 6 }}>
        <span style={{ fontWeight:700, fontSize:13 }}>Allocation targets</span>
        <span className="chip solid-blk" style={{ marginLeft: 8 }}>NAV $3.24M</span>
        <button className="pill sm outline dim" style={{ marginLeft:"auto", color:"var(--ink-2)" }}>Rebalance →</button>
      </div>
      <div className="alloc-stack" style={{ height: 16 }}>
        <div style={{ width:"42%", background:"var(--yellow)" }}></div>
        <div style={{ width:"19%", background:"var(--orange)" }}></div>
        <div style={{ width:"14%", background:"var(--green)" }}></div>
        <div style={{ width:"12%", background:"var(--violet)" }}></div>
        <div style={{ width:"8%",  background:"var(--red)" }}></div>
        <div style={{ width:"5%",  background:"rgba(255,255,255,0.4)" }}></div>
      </div>
      <div className="alloc-key" style={{ color:"var(--ink-2)" }}>
        <span><i style={{ background:"var(--yellow)" }}></i>Tech 42 · <b style={{color:"var(--ink)"}}>tgt 40</b></span>
        <span><i style={{ background:"var(--orange)" }}></i>Comm 19 · <b style={{color:"var(--ink)"}}>tgt 18</b></span>
        <span><i style={{ background:"var(--green)" }}></i>Disc 14 · <b style={{color:"var(--ink)"}}>tgt 15</b></span>
        <span><i style={{ background:"var(--violet)" }}></i>Fin 12</span>
        <span><i style={{ background:"var(--red)" }}></i>HC 8</span>
        <span><i style={{ background:"rgba(255,255,255,0.4)" }}></i>Cash 5</span>
      </div>
      <div style={{ flex:1, display:"flex", alignItems:"flex-end", gap: 6, marginTop: 8 }}>
        <button className="pill sm outline dim" style={{ color:"var(--orange)", borderColor:"var(--orange)" }}>+ Tech</button>
        <button className="pill sm outline dim" style={{ color:"var(--ink-2)" }}>− Disc</button>
        <button className="pill sm solid orange">Apply targets</button>
        <span className="round outline dim" style={{ marginLeft:"auto", color:"var(--ink-2)", width:28, height:28, fontSize: 11 }}>↗</span>
      </div>
    </div>
  );
}

function KpiPill({ cls, dark, l, v }){
  return (
    <div className={"tile tight span-2 "+cls+(dark?" dark":"")} style={{ padding: "10px 14px", justifyContent:"center" }}>
      <div style={{ fontSize: 9.5, letterSpacing:"0.12em", fontWeight: 700, opacity: 0.6, textTransform:"uppercase" }}>{l}</div>
      <div style={{ fontSize: 22, fontWeight: 500, letterSpacing:"-0.02em", marginTop: 2 }}>{v}</div>
    </div>
  );
}

function NewsTile(){
  const items = [
    ["09:42","FED","Powell signals Sept cut on hold","+"],
    ["09:36","NVDA","Earnings beat consensus by 12%","+"],
    ["09:31","TSLA","Cybercab delivery date pushed","-"],
    ["09:28","AAPL","India production 18% of global","+"],
    ["09:21","XOM","Brent breaks 86 on supply data","+"],
  ];
  return (
    <div className="tile coal span-5 row-2" style={{ padding: 14 }}>
      <div style={{ display:"flex", alignItems:"baseline", marginBottom: 6 }}>
        <span style={{ fontWeight:700, fontSize:13 }}>News · live feed</span>
        <span className="chip solid-blk" style={{ marginLeft: 8, background:"var(--green)", color:"var(--on-light)" }}>● LIVE</span>
        <span style={{ marginLeft:"auto", fontFamily:"var(--f-mono)", fontSize:10, color:"var(--ink-3)" }}>5/124</span>
      </div>
      <div style={{ flex:1, overflow:"hidden", display:"flex", flexDirection:"column", gap: 4 }}>
        {items.map(([t,sym,h,s],i) => (
          <div key={i} style={{ display:"grid", gridTemplateColumns:"42px 56px 1fr auto", gap:10, alignItems:"baseline",
            fontSize: 11.5, padding: "5px 0", borderTop: i===0?"none":"1px solid var(--line)" }}>
            <span style={{ fontFamily:"var(--f-mono)", color:"var(--ink-3)", fontSize:10 }}>{t}</span>
            <span style={{ fontWeight:700, color:"var(--orange)" }}>{sym}</span>
            <span style={{ color:"var(--ink-2)" }}>{h}</span>
            <span className={s==="+"?"chip up":"chip dn"} style={{ background: s==="+"?"rgba(78,192,122,0.12)":"rgba(216,80,62,0.12)" }}>{s==="+"?"BUL":"BEA"}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function ThreeCircles(){
  const items = [
    ["BETA",  "1.18", "vs SPY",  "var(--orange)"],
    ["VOL",   "14.2", "20D RV",  "var(--yellow)"],
    ["SHARP", "2.41", "TRAIL",   "var(--green)"],
  ];
  return (
    <div className="tile orange span-3 row-2" style={{ padding: 14 }}>
      <div style={{ display:"flex", alignItems:"baseline", marginBottom: 6 }}>
        <span style={{ fontWeight:700, fontSize:13, color:"var(--on-light)" }}>Risk gauges</span>
        <span style={{ marginLeft:"auto", fontFamily:"var(--f-mono)", fontSize:10, opacity:0.6 }}>VAR 95%</span>
      </div>
      <div style={{ display:"grid", gridTemplateColumns:"1fr 1fr 1fr", gap: 6, flex:1, alignItems:"center" }}>
        {items.map(([l,v,sub,c],i) => (
          <div key={i} style={{
            background:"var(--on-light)", color:"var(--ink)",
            borderRadius: "50%",
            aspectRatio: "1 / 1",
            display:"flex", flexDirection:"column", alignItems:"center", justifyContent:"center",
            padding: 8, position:"relative", overflow:"hidden"
          }}>
            <div style={{ position:"absolute", top:6, left:0, right:0, fontSize:8.5, letterSpacing:"0.16em", fontWeight:700, color: c, textAlign:"center" }}>{l}</div>
            <div style={{ fontSize: 24, fontWeight:500, letterSpacing:"-0.03em", lineHeight:1 }}>{v}</div>
            <div style={{ fontSize: 8.5, opacity:0.55, marginTop: 2 }}>{sub}</div>
          </div>
        ))}
      </div>
    </div>
  );
}

function MoversTile(){
  const movers = [
    ["TSLA","+5.21%",true],
    ["NVDA","+2.26%",true],
    ["META","+1.85%",true],
    ["AMD", "−2.42%",false],
    ["AAPL","−0.83%",false],
  ];
  return (
    <div className="tile cream span-4 row-2" style={{ padding: 14 }}>
      <div style={{ display:"flex", alignItems:"baseline", marginBottom: 6 }}>
        <span style={{ fontWeight:700, fontSize:13 }}>Top movers · 1H</span>
        <span style={{ marginLeft:"auto", fontFamily:"var(--f-mono)", fontSize:10, opacity:0.5 }}>S&P500</span>
      </div>
      <div style={{ flex:1, display:"flex", flexDirection:"column", gap:0, justifyContent:"center" }}>
        {movers.map(([s,c,up],i) => (
          <div key={i} style={{ display:"grid", gridTemplateColumns:"60px 1fr 70px", gap:8, alignItems:"center",
            padding:"8px 4px", borderTop: i===0?"none":"1px solid rgba(0,0,0,0.07)", fontSize:12 }}>
            <span style={{ fontWeight:700 }}>{s}</span>
            <div style={{ height: 4, borderRadius: 2, background:"rgba(0,0,0,0.07)", position:"relative", overflow:"hidden" }}>
              <div style={{ position:"absolute", top:0, bottom:0,
                [up?"left":"right"]: "50%",
                width: (parseFloat(c.replace(/[^0-9.]/g,""))*9)+"%",
                background: up?"#1a7a3a":"#b22d1c", borderRadius: 2 }}></div>
            </div>
            <span className="mono" style={{ textAlign:"right", color: up?"#1a7a3a":"#b22d1c", fontSize:11.5 }}>{c}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function ActionStrip({ setTicketOpen }){
  return (
    <div className="tile black dark span-12" style={{ padding: 14, minHeight: 80 }}>
      <div style={{ display:"flex", flexWrap:"wrap", alignItems:"center", gap:8 }}>
        <button className="pill solid orange lg" onClick={() => setTicketOpen(true)}>+ New ticket</button>
        <button className="pill outline lg" style={{ color:"var(--ink)" }}>Compare</button>
        <button className="pill outline lg" style={{ color:"var(--orange)" }}>Hedge book</button>
        <button className="pill solid pink lg">Run scenario</button>
        <span className="round outline lg" style={{ color:"var(--ink-2)" }}>→</span>
        <button className="pill outline lg" style={{ color:"var(--violet)" }}>Pairs scan</button>
        <button className="pill outline lg" style={{ color:"#5fa8d3" }}>Earnings table</button>
        <span className="round solid yellow lg">↓</span>
        <button className="pill solid green lg">Buy NVDA · 100</button>
        <button className="pill outline lg" style={{ color:"var(--ink)", marginLeft:"auto" }}>Tax lots</button>
        <button className="pill solid white lg">Download statements</button>
      </div>
    </div>
  );
}

window.TilesView = TilesView;
