/* ===== Right pane (persistent across views) — Watchlist + P&L ===== */
function RightPane({ setTicketOpen }){
  const [tab, setTab] = React.useState("watch");
  const watch = [
    ["NVDA","NVIDIA","1,284.32","+2.26%",true,true],
    ["AAPL","Apple","221.18","-0.83%",false],
    ["MSFT","Microsoft","438.94","+0.72%",true],
    ["TSLA","Tesla","254.18","-3.88%",false],
    ["AMZN","Amazon","187.42","+0.63%",true],
    ["META","Meta","571.90","+1.50%",true],
    ["GOOGL","Alphabet","174.29","-0.41%",false],
    ["AMD","AMD","165.20","-2.42%",false],
    ["AVGO","Broadcom","182.40","+1.18%",true],
    ["NFLX","Netflix","742.61","+0.55%",true],
    ["JPM","JPMorgan","230.84","-0.18%",false],
    ["XOM","Exxon","115.62","+0.94%",true],
  ];
  return (
    <aside className="rightpane">
      <div className="rp-tabs">
        <button className={tab==="watch"?"on":""} onClick={() => setTab("watch")}>WATCH</button>
        <button className={tab==="positions"?"on":""} onClick={() => setTab("positions")}>POS</button>
        <button className={tab==="alerts"?"on":""} onClick={() => setTab("alerts")}>ALERT</button>
      </div>

      <div className="rp-section" style={{ flex: 1, minHeight: 0, overflow: "hidden", display: "flex", flexDirection: "column" }}>
        <div className="rp-h">
          <span>Watchlist · Megacaps</span>
          <span className="count">12</span>
        </div>
        <div style={{ flex: 1, overflowY: "auto", overflowX: "hidden", display: "flex", flexDirection: "column", gap: 1 }}>
          {watch.map(([s,n,p,c,up,active],i) => (
            <div key={i} className={"rp-row "+(active?"active":"")} onClick={() => setTicketOpen(true)}>
              <span className="sym">{s}</span>
              <span className="name">{n}</span>
              <span className="px">{p}</span>
              <span className={"ch "+(up?"up":"dn")}>{c}</span>
            </div>
          ))}
        </div>
      </div>

      <div className="rp-pl">
        <div className="l">P&L · TODAY</div>
        <div className="v">+$290,794</div>
        <div className="s">+1.32% · NAV $3.24M</div>
        <div className="row">
          <div><div className="k">REALIZED</div><div className="v2">+$48k</div></div>
          <div><div className="k">UNREAL.</div><div className="v2">+$242k</div></div>
        </div>
      </div>

      <div className="rp-status">
        <div className="row"><span className="led" style={{ background: "var(--green)" }}></span>Stream OK · <b>4ms</b></div>
        <div className="row"><span className="led" style={{ background: "var(--yellow)" }}></span>NY-4 · <b>L1+L2</b></div>
        <div className="row">BP <b>$5.83M</b> · 2026-04-26</div>
      </div>
    </aside>
  );
}

window.RightPane = RightPane;
