/* ===== Main app shell · pill nav + ticket + hotkeys ===== */
const { useState, useEffect, useMemo } = React;

const TAPE_ITEMS = [
  ["NVDA","1284.32","+2.26%",1],["AAPL","177.69","-1.31%",-1],["MSFT","421.10","+0.72%",1],
  ["TSLA","256.83","+2.18%",1],["META","571.99","+1.85%",1],["AMZN","185.07","+0.32%",1],
  ["GOOGL","174.29","-0.41%",-1],["AMD","165.20","-2.42%",-1],["AVGO","182.40","+1.18%",1],
  ["NFLX","742.61","+0.55%",1],["JPM","230.84","-0.18%",-1],["XOM","115.62","+0.94%",1],
  ["BRK.B","452.80","+0.21%",1],["CRM","312.44","-0.66%",-1],["ORCL","178.92","+1.11%",1],
  ["UNH","572.18","-0.88%",-1],["V","295.41","+0.42%",1],["HD","412.07","+0.13%",1],
];
window.TAPE_ITEMS = TAPE_ITEMS;

function Topbar({ view, setView }) {
  const [time, setTime] = useState(new Date());
  useEffect(() => {
    const t = setInterval(() => setTime(new Date()), 1000);
    return () => clearInterval(t);
  }, []);
  const fmt = time.toLocaleTimeString("en-US", { hour: "2-digit", minute: "2-digit", hour12: false });
  const tabs = [
    ["Trade","trade"],
    ["Live tiles","tiles"],
    ["Dashboard","dashboard"],
    ["Research","research"],
    ["Risk","risk"],
  ];
  return (
    <div className="topbar">
      <div className="brand">
        <span className="mark"></span>
        <span className="name">MERIDIAN</span>
        <span className="ver">v.2.4</span>
      </div>
      <div className="tabs">
        {tabs.map(([l,k]) => (
          <button key={k}
            className={view===k ? (k==="trade"?"on orange":"on") : ""}
            onClick={() => setView(k)}>{l}</button>
        ))}
      </div>
      <div className="spacer"></div>
      <div className="search-pill">
        <span>⌕</span>
        <input placeholder="Type a function or symbol…" />
        <kbd>⌘K</kbd>
      </div>
      <div className="icon-pill">
        <span className="dot"></span>
        <span className="who">NYSE</span>
        <span className="clock">{fmt}</span>
      </div>
      <div className="avatar">JM</div>
    </div>
  );
}

function Subnav({ onTicket }) {
  const tfs = ["1m","5m","15m","1H","4H","1D","1W"];
  const [tf, setTf] = useState("15m");
  const [openMenu, setOpenMenu] = useState(null);
  const indicators = [
    ["MA",          ["SMA 20","SMA 50","SMA 200","EMA 9","EMA 21","VWAP","HMA","KAMA"]],
    ["Oscillators", ["RSI 14","Stochastic","MACD","CCI 20","Williams %R","MFI","Awesome Osc"]],
    ["Trends",      ["ADX","Aroon","Ichimoku","SuperTrend","Parabolic SAR","Donchian"]],
    ["Volatility",  ["Bollinger","Keltner","ATR","Std Dev","Chaikin Vol"]],
    ["Volume",      ["OBV","VWAP","Volume Profile","CMF","A/D Line","Volume by Time"]],
  ];
  return (
    <div className="subnav">
      <div className="tf-pill">
        {tfs.map(t => (
          <button key={t} className={tf===t?"on":""} onClick={() => setTf(t)}>{t}</button>
        ))}
      </div>
      <div className="indicator-row">
        {indicators.map(([label, items]) => (
          <div key={label} className={"dropdown "+(openMenu===label?"open":"")}
               onMouseLeave={() => setOpenMenu(null)}>
            <button className="dd-btn"
                    onClick={() => setOpenMenu(openMenu===label?null:label)}>
              {label}<span className="caret">▾</span>
            </button>
            {openMenu===label && (
              <div className="dd-menu">
                <div className="dd-h">{label}</div>
                {items.map(it => (
                  <div key={it} className="dd-item">
                    <span className="dd-check"></span>{it}
                  </div>
                ))}
                <div className="dd-foot">
                  <button className="pill sm outline" style={{ color: "var(--ink-2)", borderColor: "var(--line-2)" }}>+ Custom</button>
                </div>
              </div>
            )}
          </div>
        ))}
      </div>
      <button className="dot-btn" title="Studies">∿</button>
      <button className="dot-btn" title="Tools">✎</button>
      <button className="dot-btn" title="Alerts">◔</button>
      <div className="tape">
        <div className="tape-track">
          {[...TAPE_ITEMS, ...TAPE_ITEMS].map((it,i) => (
            <span key={i} className={"tape-item "+(it[3]>0?"up":"dn")}>
              <span className="sy">{it[0]}</span>
              <span className="px">{it[1]}</span>
              <span className="ch">{it[2]}</span>
            </span>
          ))}
        </div>
      </div>
      <button className="dot-btn orange" onClick={onTicket} title="New ticket (⌘K)">+</button>
    </div>
  );
}

function OrderTicket({ open, onClose }) {
  const [side, setSide] = useState("BUY");
  const [type, setType] = useState("LMT");
  const [tif, setTif]   = useState("DAY");
  const [qty, setQty]   = useState(100);
  const [px,  setPx]    = useState(1284.32);
  if (!open) return null;
  const notional = qty * px;
  const fmt = (n) => "$" + n.toLocaleString(undefined, { maximumFractionDigits: 0 });
  return (
    <div className="overlay" onClick={onClose}>
      <div className="ticket" onClick={(e) => e.stopPropagation()}>
        <div className="ticket-h">
          <span className="accent">●</span>
          <span className="ttl">Order ticket</span>
          <span className="sym">NVDA</span>
          <button className="x" onClick={onClose}>✕</button>
        </div>
        <div className="t-bidask">
          <div className="cell"><div className="lbl">Bid</div><div className="v">1,284.27</div></div>
          <div className="cell last"><div className="lbl">Last</div><div className="v">{px.toFixed(2)}</div></div>
          <div className="cell"><div className="lbl">Ask</div><div className="v">1,284.37</div></div>
        </div>
        <div className="t-side">
          <button className={"buy "+(side==="BUY"?"on":"")} onClick={() => setSide("BUY")}>Buy · long</button>
          <button className={"sell "+(side==="SELL"?"on":"")} onClick={() => setSide("SELL")}>Sell · short</button>
        </div>
        <div className="t-row">
          <span className="lbl">Type</span>
          <div className="t-seg">
            {["MKT","LMT","STP","STP·L"].map(t =>
              <button key={t} className={type===t?"on":""} onClick={() => setType(t)}>{t}</button>
            )}
          </div>
        </div>
        <div className="t-row">
          <span className="lbl">TIF</span>
          <div className="t-seg">
            {["DAY","GTC","IOC","FOK"].map(t =>
              <button key={t} className={tif===t?"on":""} onClick={() => setTif(t)}>{t}</button>
            )}
          </div>
        </div>
        <div className="t-input">
          <div className="cell">
            <div className="lbl">Quantity</div>
            <input type="number" value={qty} onChange={e => setQty(+e.target.value || 0)} />
            <div className="stepper">
              <button onClick={() => setQty(Math.max(0, qty-100))}>−</button>
              <button onClick={() => setQty(qty+100)}>+</button>
              <button className="wide" onClick={() => setQty(500)}>500</button>
              <button className="wide" onClick={() => setQty(1000)}>1k</button>
            </div>
          </div>
          <div className="cell">
            <div className="lbl">Limit price</div>
            <input type="number" step="0.01" value={px} onChange={e => setPx(+e.target.value || 0)} />
            <div className="stepper">
              <button onClick={() => setPx(+(px-0.05).toFixed(2))}>−</button>
              <button onClick={() => setPx(+(px+0.05).toFixed(2))}>+</button>
              <button className="wide" onClick={() => setPx(1284.32)}>Last</button>
            </div>
          </div>
        </div>
        <div className="t-meta"><span>Notional</span><b>{fmt(notional)}</b></div>
        <div className="t-meta"><span>Buying power</span><b>$1,482,930</b></div>
        <div className="t-meta"><span>Slippage est.</span><b>0.3 bp</b></div>
        <button className={"t-review "+(side==="SELL"?"sell":"")}>
          Review {side.toLowerCase()} · {qty} @ {px.toFixed(2)}
          <span className="arrow">→</span>
        </button>
      </div>
    </div>
  );
}

function App() {
  const [view, setView] = useState("trade");
  const [ticketOpen, setTicketOpen] = useState(false);
  const [tweaks, setTweak] = useTweaks(/*EDITMODE-BEGIN*/{
    "theme": "aperture",
    "showTape": true,
    "startView": "trade"
  }/*EDITMODE-END*/);

  useEffect(() => { setView(tweaks.startView || "trade"); /* eslint-disable-next-line */ }, []);
  useEffect(() => {
    document.documentElement.setAttribute("data-theme", tweaks.theme || "bauhaus");
  }, [tweaks.theme]);

  useEffect(() => {
    function onKey(e) {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault(); setTicketOpen(v => !v); return;
      }
      if (e.key === "Escape") { setTicketOpen(false); return; }
      const tag = document.activeElement?.tagName;
      if (e.metaKey || e.ctrlKey || e.altKey || tag === "INPUT" || tag === "TEXTAREA") return;
      const k = e.key.toLowerCase();
      if (k === "t") setView("trade");
      if (k === "w") setView("tiles");
      if (k === "d") setView("dashboard");
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  let body = null;
  if (view === "tiles") body = <TilesView setView={setView} setTicketOpen={setTicketOpen} />;
  else if (view === "dashboard") body = <DashboardView setView={setView} setTicketOpen={setTicketOpen} />;
  else body = <TradeView setView={setView} setTicketOpen={setTicketOpen} />;

  return (
    <div className="app" data-screen-label={view}>
      <Topbar view={view} setView={setView} />
      <Subnav onTicket={() => setTicketOpen(true)} />
      <div className="body-wrap">{body}</div>
      <RightPane setTicketOpen={setTicketOpen} />
      <OrderTicket open={ticketOpen} onClose={() => setTicketOpen(false)} />

      <TweaksPanel title="Tweaks">
        <TweakSection title="Theme">
          <TweakRadio label="Palette" value={tweaks.theme}
            options={[
              { value: "aperture",  label: "APERTURE" },
              { value: "bauhaus",   label: "BAUHAUS" },
              { value: "midnight",  label: "MIDNIGHT" },
              { value: "nord",      label: "NORD" },
              { value: "monokai",   label: "MONOKAI" },
              { value: "solarized", label: "SOLAR" },
              { value: "rosepine",  label: "ROSÉ" },
            ]}
            onChange={(v) => setTweak("theme", v)} />
        </TweakSection>
        <TweakSection title="Start view">
          <TweakRadio label="Open on" value={tweaks.startView}
            options={[
              { value: "trade", label: "TRADE" },
              { value: "tiles", label: "TILES" },
              { value: "dashboard", label: "DASH" },
            ]}
            onChange={(v) => setTweak("startView", v)} />
        </TweakSection>
        <TweakSection title="Quick">
          <TweakButton label="Open ticket (⌘K)" onClick={() => setTicketOpen(true)} />
        </TweakSection>
      </TweaksPanel>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")).render(<App />);
