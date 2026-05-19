/* ===== Dashboard view — hero stat + bold info-design widgets ===== */
const { useMemo: useMemoD } = React;

function DashboardView({ setView, setTicketOpen }) {
  return (
    <div className="dash">
      {/* Hero NAV / equity */}
      <div className="tile orange" style={{ gridColumn:"span 6", gridRow:"span 3", padding:"20px 24px" }}>
        <div style={{ display:"flex", alignItems:"flex-start" }}>
          <span className="badge-pill">Net Liquidation</span>
          <button className="more">⋯</button>
        </div>
        <div style={{ marginTop:10, fontSize:13, opacity:0.55, maxWidth:380 }}>
          Calculated across all venues, real-time mark-to-market.
        </div>
        <div style={{ fontSize:120, fontWeight:500, letterSpacing:"-0.05em", lineHeight:0.9, marginTop:14 }}>
          $3.24M
        </div>
        <div style={{ marginTop:14, display:"flex", gap:10, alignItems:"center" }}>
          <span className="pill solid black">+ $42,118 today</span>
          <span className="pill outline" style={{ borderColor:"rgba(0,0,0,0.5)" }}>+1.32%</span>
          <span className="pill outline" style={{ borderColor:"rgba(0,0,0,0.5)" }}>YTD +18.4%</span>
        </div>
        <div className="date-strip" style={{ borderColor:"var(--on-light)", marginTop:"auto" }}>
          <span className="arrow-r" style={{ borderColor:"var(--on-light)" }}>←</span>
          <span className="label">February 2026</span>
          <span className="arrow-r" style={{ borderColor:"var(--on-light)" }}>→</span>
        </div>
      </div>

      {/* P&L block bar */}
      <div className="tile yellow" style={{ gridColumn:"span 4", gridRow:"span 3", padding:"20px 22px" }}>
        <div style={{ display:"flex", alignItems:"flex-start" }}>
          <span className="badge-pill">P&L · 30D</span>
          <button className="more" style={{ background:"transparent", border:"1.5px solid var(--on-light)" }}>⋯</button>
        </div>
        <div style={{ fontSize:88, fontWeight:500, letterSpacing:"-0.05em", lineHeight:0.95, marginTop:18 }}>
          +17.43%
        </div>
        <div style={{ flex:1, display:"flex", alignItems:"flex-end", marginTop:12, position:"relative", minHeight:80 }}>
          <div style={{ width:"58%", height:"75%", background:"var(--on-light)" }}></div>
          <div style={{ width:"30%", height:"45%", background:"rgba(0,0,0,0.25)" }}></div>
          <div style={{ position:"absolute", left:0, bottom:-18, fontSize:12, fontWeight:700 }}>+18.56%</div>
          <div style={{ position:"absolute", left:"60%", bottom:-18, fontSize:12, opacity:0.6 }}>Est +15.78%</div>
        </div>
        <div className="date-strip" style={{ borderColor:"var(--on-light)", marginTop:32 }}>
          <span className="arrow-r" style={{ borderColor:"var(--on-light)" }}>←</span>
          <span className="label">February 2026</span>
          <span className="arrow-r" style={{ borderColor:"var(--on-light)" }}>→</span>
        </div>
      </div>

      {/* Sharpe ring */}
      <div className="tile green" style={{ gridColumn:"span 2", gridRow:"span 3", padding:"16px 18px" }}>
        <span className="badge-pill">Sharpe · YTD</span>
        <button className="more">⋯</button>
        <div style={{ flex:1, display:"flex", alignItems:"center", justifyContent:"center" }}>
          <Ring v={2.41/4} label="2.41" sub="Sortino 3.02" />
        </div>
      </div>

      {/* Positions */}
      <div className="tile coal" style={{ gridColumn:"span 6", gridRow:"span 3", padding:"16px 18px" }}>
        <div style={{ display:"flex", alignItems:"center" }}>
          <span className="badge-pill">Positions</span>
          <span className="pill outline sm dim" style={{ marginLeft:8, color:"var(--ink-2)" }}>12 open</span>
          <span className="pill outline sm dim" style={{ marginLeft:6, color:"var(--ink-2)" }}>2 pending</span>
          <button className="more" style={{ marginLeft:"auto" }}>⋯</button>
        </div>
        <div style={{ marginTop:10, overflow:"auto", flex:1 }}>
          <table style={{ width:"100%", borderCollapse:"collapse", fontFamily:"var(--f-mono)", fontSize:12 }}>
            <thead><tr style={{ color:"var(--ink-3)", fontSize:10, letterSpacing:"0.1em", textTransform:"uppercase" }}>
              <th style={{ textAlign:"left", padding:"6px 8px", fontWeight:600 }}>Sym</th>
              <th style={{ textAlign:"right", padding:"6px 8px", fontWeight:600 }}>Qty</th>
              <th style={{ textAlign:"right", padding:"6px 8px", fontWeight:600 }}>Avg</th>
              <th style={{ textAlign:"right", padding:"6px 8px", fontWeight:600 }}>Last</th>
              <th style={{ textAlign:"right", padding:"6px 8px", fontWeight:600 }}>Δ</th>
            </tr></thead>
            <tbody>
              {[
                ["NVDA",450,1256.40,1284.32,"+2.26%",true],
                ["AAPL",-200,180.92,177.69,"+1.78%",true],
                ["MSFT",300,415.20,421.10,"+1.42%",true],
                ["TSLA",100,244.10,256.83,"+5.21%",true],
                ["META",80,562.40,571.99,"+1.71%",true],
                ["AMD",-150,169.20,165.20,"+2.36%",true],
                ["GOOGL",200,175.10,174.29,"-0.46%",false],
                ["SPY",500,580.10,582.41,"+0.40%",true],
              ].map(([s,q,a,l,p,up],i) => (
                <tr key={i} style={{ borderTop:"1px solid var(--line)" }}>
                  <td style={{ padding:"7px 8px", color:"var(--ink)", fontWeight:600 }}>{s}</td>
                  <td style={{ padding:"7px 8px", textAlign:"right" }}>{q}</td>
                  <td style={{ padding:"7px 8px", textAlign:"right" }}>{a.toFixed(2)}</td>
                  <td style={{ padding:"7px 8px", textAlign:"right" }}>{l.toFixed(2)}</td>
                  <td style={{ padding:"7px 8px", textAlign:"right",
                    color: up?"var(--green)":"var(--red)" }}>{p}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      {/* Top mover */}
      <div className="tile pink" style={{ gridColumn:"span 3", gridRow:"span 2", padding:"16px 18px" }}>
        <span className="badge-pill">Top mover</span>
        <button className="more">⋯</button>
        <div style={{ marginTop:"auto", fontSize:64, fontWeight:500, letterSpacing:"-0.04em", lineHeight:1 }}>TSLA</div>
        <div style={{ marginTop:8, display:"flex", gap:8, alignItems:"center" }}>
          <span className="pill solid black sm">+5.21%</span>
          <span className="mono" style={{ fontSize:12 }}>256.83 · vol 32.4M</span>
        </div>
      </div>

      {/* Lollipop activity */}
      <div className="tile cream" style={{ gridColumn:"span 3", gridRow:"span 2", padding:"14px 16px",
        display:"flex", flexDirection:"column" }}>
        <div style={{ display:"flex", alignItems:"center" }}>
          <span className="badge-pill">Trading hours · YTD</span>
          <button className="more" style={{ background:"transparent", border:"1.5px solid var(--on-light)" }}>⋯</button>
        </div>
        <div style={{ fontSize:48, fontWeight:500, letterSpacing:"-0.04em", lineHeight:1, marginTop:8 }}>554h</div>
        <div className="lolli" style={{ color:"var(--orange)", flex:1, marginTop:6 }}>
          {[12,18,9,4,3,28,11,16,15,7,6,9].map((v,i) => {
            const max = 28; const tall = v === max;
            const labels = ["J","F","M","A","M","J","J","A","S","O","N","D"];
            return (
              <div key={i} className={"bar"+(tall?" tall":"")} style={{ height:"100%" }}>
                <div className="stem" style={{ height: (v/max*100)+"%" }}></div>
                <div className="head"></div>
                <span className="label">{labels[i]}</span>
              </div>
            );
          })}
        </div>
      </div>

      {/* Bottom action strip */}
      <div className="tile black dark" style={{ gridColumn:"span 12", padding:"16px 18px", minHeight:90 }}>
        <div style={{ display:"flex", flexWrap:"wrap", gap:10, alignItems:"center" }}>
          <button className="pill solid orange lg" onClick={() => setTicketOpen(true)}>+ New ticket</button>
          <button className="pill outline lg" style={{ color:"var(--ink)" }}>Rebalance</button>
          <button className="pill outline lg" style={{ color:"var(--green)" }}>Hedge book</button>
          <button className="pill outline lg" style={{ color:"var(--yellow)" }}>Run scenario</button>
          <button className="pill outline lg" style={{ color:"var(--violet)" }}>Export tax lots</button>
          <span className="round outline lg" style={{ color:"var(--ink)", marginLeft:"auto" }}>↗</span>
          <button className="pill solid white lg">Download statements</button>
        </div>
      </div>
    </div>
  );
}

function Ring({ v, label, sub }) {
  const r = 50, c = 2 * Math.PI * r;
  return (
    <div style={{ position:"relative", width:130, height:130 }}>
      <svg viewBox="0 0 130 130" width="130" height="130">
        <circle cx="65" cy="65" r={r} fill="none" stroke="rgba(0,0,0,0.12)" strokeWidth="9"/>
        <circle cx="65" cy="65" r={r} fill="none" stroke="var(--on-light)" strokeWidth="9"
                strokeDasharray={`${v*c} ${c}`} strokeLinecap="round"
                transform="rotate(-90 65 65)"/>
      </svg>
      <div style={{ position:"absolute", inset:0, display:"flex", flexDirection:"column",
        alignItems:"center", justifyContent:"center" }}>
        <div style={{ fontSize:36, fontWeight:500, letterSpacing:"-0.03em", lineHeight:1 }}>{label}</div>
        <div style={{ fontSize:10, marginTop:4, opacity:0.6 }}>{sub}</div>
      </div>
    </div>
  );
}

window.DashboardView = DashboardView;
