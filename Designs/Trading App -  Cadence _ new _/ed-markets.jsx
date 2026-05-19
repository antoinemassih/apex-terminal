// ed-markets.jsx — Screen II: "Markets" — instrument deep dive

function MarketsWatchTable({ syms, activeSym, setActiveSym }){
  const groups = [
    { label:'Megacaps · Tech', range:[0, 5] },
    { label:'Cyclical · Auto', range:[5, 9] },
    { label:'Index · ETF',     range:[9, 12] },
  ];

  return (
    <div>
      <div className="sec-head">
        <span className="ttl">Selector</span>
        <span className="sub">By weight</span>
      </div>

      <div className="search" style={{ marginBottom:10 }}>
        <input placeholder="Find a security…"/>
        <span className="kbd">⌘K</span>
      </div>

      <table className="table dense">
        <thead>
          <tr>
            <th>Issue</th>
            <th>30d</th>
            <th className="r">Last</th>
            <th className="r">Day</th>
          </tr>
        </thead>
        <tbody>
        {groups.map(g => (
          <React.Fragment key={g.label}>
            <tr className="subhead"><td colSpan={4}>{g.label}</td></tr>
            {syms.slice(g.range[0], g.range[1]).map((s, i) => {
              const spark = makeArea(20, s.sym.length*17 + i, s.px*0.95, s.px*0.008);
              const on = activeSym === s.sym;
              return (
                <tr key={s.sym} className={on ? 'on' : ''}
                    onClick={() => setActiveSym && setActiveSym(s.sym)}>
                  <td>
                    <div className="watch-cell-sym">
                      <span className="ticker">{s.sym}</span>
                      <span className="desc">{s.name.slice(0, 12)}</span>
                    </div>
                  </td>
                  <td><EditorialSpark data={spark} w={56} h={18} fill/></td>
                  <td className="r px">{fmtNum(s.px)}</td>
                  <td className={`r ${s.ch>=0?'up':'dn'}`}>{fmtCh(s.ch)}</td>
                </tr>
              );
            })}
          </React.Fragment>
        ))}
        </tbody>
      </table>
    </div>
  );
}

function TicketStub({ sym, refPrice }){
  const [side, setSide]   = useState('BUY');
  const [qty,  setQty]    = useState(100);
  const [type, setType]   = useState('LMT');
  const [tif,  setTif]    = useState('DAY');
  const [px,   setPx]     = useState(refPrice);
  useEffect(() => setPx(refPrice), [refPrice]);

  const notional = qty * px;
  const sideClass = side === 'BUY' ? 'buy' : 'sell';

  return (
    <div className="ticket-stub">
      <div className="ticket-header">
        <div>
          <div className="label">Order Ticket</div>
          <div className="ttl">{sym}</div>
        </div>
        <div style={{ textAlign:'right' }}>
          <div className="no">№ TX-2026.05.17-04827</div>
          <div className="label" style={{ marginTop:2 }}>16:14 ET</div>
        </div>
      </div>

      <div className="ticket-body">
        <div className="side-toggle">
          <button className={`${side==='BUY'?'on buy':''}`} onClick={() => setSide('BUY')}>Buy / Long</button>
          <button className={`${side==='SELL'?'on sell':''}`} onClick={() => setSide('SELL')}>Sell / Short</button>
        </div>

        <div className="ticket-field">
          <label>Symbol</label>
          <span className="val lg">{sym}</span>
        </div>

        <div className="ticket-field">
          <label>Order type</label>
          <div className="tk-row" style={{ flexWrap:'wrap' }}>
            {['MKT','LMT','STP','STL'].map(t => (
              <button key={t} className={`tk-chip ${type===t?'on':''}`} onClick={() => setType(t)}>{t}</button>
            ))}
          </div>
        </div>

        <div className="ticket-field">
          <label>Quantity</label>
          <input className="tk-input" type="number" value={qty}
                 onChange={e => setQty(+e.target.value || 0)} style={{ maxWidth: 140 }}/>
        </div>

        {type !== 'MKT' && (
          <div className="ticket-field">
            <label>Limit price</label>
            <input className="tk-input" type="number" value={px.toFixed(2)} step="0.01"
                   onChange={e => setPx(+e.target.value || 0)} style={{ maxWidth: 140 }}/>
          </div>
        )}

        <div className="ticket-field">
          <label>Time in force</label>
          <div className="tk-row">
            {['DAY','GTC','IOC','FOK'].map(t => (
              <button key={t} className={`tk-chip ${tif===t?'on':''}`} onClick={() => setTif(t)}>{t}</button>
            ))}
          </div>
        </div>

        <div style={{ marginTop:12, padding:'10px 0',
                      borderTop:'2px solid var(--ink-0)', borderBottom:'.5px solid var(--rule-2)' }}>
          <div style={{ display:'flex', justifyContent:'space-between', alignItems:'baseline' }}>
            <span style={{ fontFamily:'var(--sans)', fontSize:10, letterSpacing:'.14em',
                            textTransform:'uppercase', color:'var(--ink-2)', fontWeight:600 }}>
              Notional
            </span>
            <span className="num" style={{ fontSize: 22, fontWeight: 600, color:'var(--ink-0)' }}>
              ${notional.toLocaleString('en-US', { maximumFractionDigits: 0 })}
            </span>
          </div>
          <div style={{ display:'flex', justifyContent:'space-between', marginTop:4 }}>
            <span style={{ fontFamily:'var(--serif)', fontStyle:'italic', fontSize:12, color:'var(--ink-2)' }}>
              Estimated commission $1.00 · BP $5.83M
            </span>
          </div>
        </div>

        <button className={`tk-submit ${sideClass}`}>
          Submit · {side} {qty} {sym}
        </button>
      </div>
    </div>
  );
}

function BidAskRegister({ centerPx }){
  const rows = useMemo(() => makeLadder(centerPx, 16, Math.floor(centerPx*100)), [centerPx]);
  const bids = rows.filter(r => r.side === 'bid' || r.side === 'mid').slice(0, 8);
  const asks = rows.filter(r => r.side === 'ask').slice(0, 8);
  const topBid = bids[0];
  const topAsk = asks[asks.length-1];

  return (
    <div>
      <div className="sec-head">
        <span className="ttl">Bid / Ask Register</span>
        <span className="sub">L2 · Top of book</span>
      </div>

      <div className="register">
        <div className="register-side">
          <h4>Bid · 8 levels</h4>
          <div className="top bid">{topBid?.px.toFixed(2)}</div>
          {bids.map((r, i) => (
            <div key={i} className="register-row">
              <span className="px">{r.px.toFixed(2)}</span>
              <span className="sz">{r.vol.toLocaleString()}</span>
              <span className="total">{(r.depth*100).toFixed(0)}%</span>
            </div>
          ))}
        </div>
        <div className="register-side">
          <h4 style={{ textAlign:'right' }}>Ask · 8 levels</h4>
          <div className="top ask" style={{ textAlign:'right' }}>{topAsk?.px.toFixed(2)}</div>
          {asks.map((r, i) => (
            <div key={i} className="register-row" style={{ gridTemplateColumns:'auto auto 1fr' }}>
              <span className="total">{(r.depth*100).toFixed(0)}%</span>
              <span className="sz">{r.vol.toLocaleString()}</span>
              <span className="px" style={{ textAlign:'right' }}>{r.px.toFixed(2)}</span>
            </div>
          ))}
        </div>
      </div>

      <div style={{ marginTop:8, padding:'6px 12px',
                    fontFamily:'var(--sans)', fontSize:10, letterSpacing:'.14em',
                    textTransform:'uppercase', color:'var(--ink-2)',
                    display:'flex', justifyContent:'space-between',
                    background:'var(--paper-2)', border:'.5px solid var(--rule-2)' }}>
        <span>Spread <span className="num" style={{ color:'var(--ink-0)' }}>0.14</span></span>
        <span>Depth <span className="num" style={{ color:'var(--ink-0)' }}>$842K</span></span>
        <span>Imbalance <span className="num up">+62%</span></span>
      </div>
    </div>
  );
}

function StatsTable({ rows }){
  return (
    <table className="table dense">
      <tbody>
        {rows.map((r, i) => (
          <tr key={i}>
            <td className="name" style={{ width:'55%', fontFamily:'var(--sans)', fontStyle:'normal',
                                          fontSize:10, letterSpacing:'.14em',
                                          textTransform:'uppercase', color:'var(--ink-2)', fontWeight:600 }}>
              {r.label}
            </td>
            <td className="r px">{r.value}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function ScreenMarkets({ activeSym, setActiveSym }){
  const sym = SYMS.find(s => s.sym === activeSym) || SYMS[2]; // default NVDA
  const candles = useMemo(() => makeCandles(96, sym.sym.length*7, sym.px*0.94, sym.px*0.008),
                          [sym.sym, sym.px]);
  const containerRef = useRef(null);
  const [w, setW] = useState(700);
  useEffect(() => {
    if (!containerRef.current) return;
    const ro = new ResizeObserver(es => setW(es[0].contentRect.width));
    ro.observe(containerRef.current);
    return () => ro.disconnect();
  }, []);

  return (
    <div className="page-inner">
      <div className="grid-12" style={{ paddingTop: 20 }}>

        {/* LEFT — Watchlist sidebar (col 3) */}
        <div className="col-3">
          <MarketsWatchTable syms={SYMS}
                      activeSym={sym.sym}
                      setActiveSym={setActiveSym}/>
        </div>

        {/* CENTER — Quote + chart (col 6) */}
        <div className="col-6 vrule">
          <div className="eyebrow">{sym.name} · Common · {sym.sym === 'AAPL' ? 'NASDAQ' : 'Listed US'}</div>

          <div style={{ display:'flex', alignItems:'baseline', gap:14, marginBottom: 4 }}>
            <h1 className="headline h1" style={{ marginRight: 8 }}>{sym.sym}</h1>
            <span style={{ fontFamily:'var(--serif)', fontSize:18, color:'var(--ink-2)', fontStyle:'italic' }}>
              {sym.name}
            </span>
          </div>

          <div className="hero-quote">
            <span className="px">
              {Math.floor(sym.px).toLocaleString()}<span className="frac">.{(sym.px % 1).toFixed(2).slice(2)}</span>
            </span>
            <span className={`ch ${sym.ch>=0?'up':'dn'}`}>{sym.ch>=0?'+':''}{(sym.px*sym.ch/100).toFixed(2)}</span>
            <span className={`ch ${sym.ch>=0?'up':'dn'}`}>{fmtCh(sym.ch)}</span>
            <span className="pct">vs. previous close</span>
          </div>

          <div ref={containerRef} style={{ marginTop: 18 }}>
            <EditorialCandle candles={candles} width={w} height={320}/>
          </div>
          <div className="figcap" style={{ borderTop:'.5px solid var(--rule-1)', paddingTop:6 }}>
            Figure II — Intraday, five-minute bars; volume shown in grey beneath the price register.
            Annotation marks the last printed value. Source: Alto market data.
          </div>

          {/* By the Numbers */}
          <div className="hrule" style={{ marginTop:18 }}>
            <div className="eyebrow muted">By the Numbers</div>
            <dl className="dl-stats">
              <StatPair label="Open" value={fmtNum(sym.px*0.989)}/>
              <StatPair label="High" value={fmtNum(sym.px*1.012)}/>
              <StatPair label="Low" value={fmtNum(sym.px*0.978)}/>
              <StatPair label="Volume" value="48.2M"/>
              <StatPair label="52w high" value={fmtNum(sym.px*1.08)}/>
              <StatPair label="52w low" value={fmtNum(sym.px*0.54)}/>
            </dl>
          </div>

          {/* Trader's Notebook */}
          <div className="hrule" style={{ marginTop: 22 }}>
            <div className="eyebrow">Trader's Notebook · 17 May</div>
            <h3 className="headline h3">Reading the tape into the close.</h3>
            <div className="col-text" style={{ marginTop: 12 }}>
              <p>
                {sym.sym} traded through its three-session resistance on the late-morning print
                and held the level into Europe's close. Volume profile showed accumulation
                between 11:40 and 13:00 ET, with the desk's L2 feed registering a 62 per cent
                bid imbalance at the top of book.
              </p>
              <p>
                Options open interest moved meaningfully on the {fmtNum(sym.px*1.005)} strike,
                with the largest dealer adding roughly 12,000 lots in the front-week call.
                Short-dated volatility eased a quarter point; longer tenors held firm.
              </p>
              <p>
                The desk's working position is sized to one-quarter limit; we will scale if the
                tape closes above {fmtNum(sym.px*1.01)} on a daily basis.
              </p>
            </div>
            <div className="byline" style={{ marginTop: 10 }}>
              <span className="dateline">New York —</span> by
              <span className="name"> The Equities Desk</span>
            </div>
          </div>
        </div>

        {/* RIGHT — Order ticket + Bid/Ask register (col 3) */}
        <div className="col-3 vrule">
          <TicketStub sym={sym.sym} refPrice={sym.px}/>

          <div className="hrule" style={{ marginTop: 24 }}>
            <BidAskRegister centerPx={sym.px}/>
          </div>

          <div className="hrule" style={{ marginTop: 20 }}>
            <div className="eyebrow muted">Listing Details</div>
            <StatsTable rows={[
              { label:'Ticker',        value: sym.sym },
              { label:'Exchange',      value: sym.sym === 'AAPL' || sym.sym === 'NVDA' ? 'NASDAQ' : 'NYSE' },
              { label:'Sector',        value: 'Technology' },
              { label:'Mkt cap',       value: '3.16T' },
              { label:'P / E (ttm)',   value: '68.2' },
              { label:'Dividend',      value: '0.04 / qtr' },
              { label:'Beta · 5y',     value: '1.92' },
              { label:'Float',         value: '24.5B' },
            ]}/>
          </div>
        </div>
      </div>
    </div>
  );
}

window.ScreenMarkets = ScreenMarkets;
window.TicketStub = TicketStub;
window.BidAskRegister = BidAskRegister;
