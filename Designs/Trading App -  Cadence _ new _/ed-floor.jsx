// ed-floor.jsx — Screen III: "Trading Floor"
// Active trading desk: chart + DOM ladder + order ticket + compact wire.

function CompactWatchlist({ syms, activeSym, setActiveSym }){
  return (
    <div>
      <div className="sec-head">
        <span className="ttl">Watch list</span>
        <span className="sub">{syms.length} issues</span>
      </div>
      <div className="search" style={{ marginBottom:8 }}>
        <input placeholder="Find a security…"/>
        <span className="kbd">⌘K</span>
      </div>
      <table className="table dense">
        <tbody>
          {syms.slice(0, 10).map((s, i) => {
            const spark = makeArea(20, s.sym.length*13 + i, s.px*0.95, s.px*0.008);
            const on = activeSym === s.sym;
            return (
              <tr key={s.sym} className={on ? 'on' : ''}
                  onClick={() => setActiveSym && setActiveSym(s.sym)}>
                <td className="sym">{s.sym}</td>
                <td><EditorialSpark data={spark} w={48} h={16} fill/></td>
                <td className="r px">{fmtNum(s.px)}</td>
                <td className={`r ${s.ch>=0?'up':'dn'}`}>{fmtCh(s.ch)}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function CompactWire(){
  const wires = [
    { kicker:'Earnings', src:'BBG',  city:'SAN JOSE', time:'15:42',
      hed:'Nvidia surges past $3.16tn as Blackwell shipments accelerate',
      body:'Morgan Stanley and Wedbush raised price targets to $1,400.' },
    { kicker:'Autos', src:'RTRS', city:'AUSTIN',   time:'15:38',
      hed:'Tesla Q3 deliveries miss estimates as China softens',
      body:'Missed by 8,200 units; FSD revenue partially offsets weaker vehicle margins.' },
    { kicker:'Macro', src:'BBG',  city:'NEW YORK', time:'15:17',
      hed:'JPMorgan lifts S&P 500 year-end target to 6,200 on AI capex',
      body:'Third upward revision this year. Kolanovic cites resilient consumer.' },
  ];

  return (
    <div>
      <div className="sec-head">
        <span className="ttl">Wire</span>
        <span className="sub">live · 3 of 14</span>
      </div>
      {wires.map((w, i) => (
        <article key={i} className="news-article" style={{ padding: '10px 0' }}>
          <div className="kicker" style={{ fontSize: 9 }}>{w.kicker}</div>
          <h3 className="hed" style={{ fontSize: 13 }}>{w.hed}</h3>
          <p className="summary" style={{ fontSize: 11.5, marginTop: 4 }}>
            <span className="dateline" style={{ fontSize:10 }}>{w.city} —</span>{w.body}
          </p>
          <div className="meta">
            <span className="src">{w.src}</span> · {w.time} ET
          </div>
        </article>
      ))}
    </div>
  );
}

function ChartHeader({ sym, refPrice, ch }){
  const [tf, setTf] = useState('5m');
  const tfs = ['1m','5m','15m','1h','1D','1W'];
  return (
    <div style={{ display:'flex', alignItems:'baseline', gap:12,
                  borderBottom:'2px solid var(--ink-0)', paddingBottom:6, flexWrap:'wrap' }}>
      <h2 className="headline h2" style={{ marginRight: 4 }}>{sym}</h2>
      <span className="num" style={{ fontSize: 18, color:'var(--ink-0)' }}>{fmtNum(refPrice)}</span>
      <span className={`num ${ch>=0?'up':'dn'}`} style={{ fontSize:14, fontWeight:600 }}>
        {fmtCh(ch)}
      </span>
      <span style={{ flex:1 }}/>
      <div style={{ display:'flex', gap:3, alignItems:'center' }}>
        {tfs.map(t => (
          <button key={t} className={`tk-chip ${tf===t?'on':''}`} onClick={() => setTf(t)}
                  style={{ padding:'3px 8px', fontSize:9 }}>
            {t}
          </button>
        ))}
      </div>
      <span style={{ fontFamily:'var(--sans)', fontSize:9, letterSpacing:'.14em',
                      textTransform:'uppercase', color:'var(--ink-2)' }}>
        live · 16:14 ET
      </span>
    </div>
  );
}

function TradesBlotter(){
  const trades = useMemo(() => {
    const r = mulberry32(31);
    const syms = ['NVDA','SPY','AAPL','TSLA','META','AMD','MSFT','GOOGL','AMZN','NVDA','SPY','AAPL'];
    return syms.map((sym, i) => {
      const ref = SYMS.find(s => s.sym === sym);
      const side = r() > 0.5 ? 'BUY' : 'SELL';
      const qty  = (Math.floor(r() * 6) + 1) * 50;
      const px   = ref ? ref.px + (r() - 0.5) * ref.px * 0.005 : 100;
      const t    = new Date(Date.now() - i * 90 * 1000);
      const tStr = `${String(t.getHours()).padStart(2,'0')}:${String(t.getMinutes()).padStart(2,'0')}:${String(t.getSeconds()).padStart(2,'0')}`;
      return { sym, side, qty, px, t: tStr };
    });
  }, []);

  return (
    <div>
      <div className="sec-head">
        <span className="ttl">Today's blotter</span>
        <span className="sub">{trades.length} fills · live</span>
      </div>
      <table className="table dense">
        <thead>
          <tr>
            <th>Time</th>
            <th>Issue</th>
            <th>Side</th>
            <th className="r">Qty</th>
            <th className="r">Fill</th>
            <th className="r">Notional</th>
          </tr>
        </thead>
        <tbody>
          {trades.slice(0, 8).map((t, i) => (
            <tr key={i}>
              <td style={{ color:'var(--ink-2)' }}>{t.t}</td>
              <td className="sym">{t.sym}</td>
              <td className={t.side === 'BUY' ? 'up-strong' : 'dn-strong'}
                  style={{ fontFamily:'var(--sans)', fontSize:10, letterSpacing:'.14em' }}>
                {t.side}
              </td>
              <td className="r">{t.qty}</td>
              <td className="r px">{fmtNum(t.px)}</td>
              <td className="r" style={{ color:'var(--ink-1)' }}>
                ${(t.qty * t.px).toLocaleString('en-US', { maximumFractionDigits: 0 })}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function ScreenFloor({ activeSym, setActiveSym }){
  const sym = SYMS.find(s => s.sym === activeSym) || SYMS[2];
  const candles = useMemo(() => makeCandles(72, sym.sym.length*9, sym.px*0.95, sym.px*0.007),
                          [sym.sym, sym.px]);
  const containerRef = useRef(null);
  const [w, setW] = useState(600);
  useEffect(() => {
    if (!containerRef.current) return;
    const ro = new ResizeObserver(es => setW(es[0].contentRect.width));
    ro.observe(containerRef.current);
    return () => ro.disconnect();
  }, []);

  return (
    <div style={{ display:'grid',
                  gridTemplateColumns: '220px 1fr 280px',
                  height: '100%',
                  gap: 0,
                  overflow: 'hidden' }}>

      {/* LEFT — Watchlist + compact wire */}
      <div style={{ borderRight:'1px solid var(--rule-2)',
                    overflow:'auto', padding:'14px 14px 24px' }}>
        <CompactWatchlist syms={SYMS}
                          activeSym={sym.sym}
                          setActiveSym={setActiveSym}/>
        <div style={{ marginTop:18 }}>
          <CompactWire/>
        </div>
      </div>

      {/* CENTER — Chart + blotter */}
      <div style={{ overflow:'auto', padding:'14px 16px 24px',
                    display:'flex', flexDirection:'column', gap:16 }}>
        <ChartHeader sym={sym.sym} refPrice={sym.px} ch={sym.ch}/>
        <div ref={containerRef} style={{ flex:'0 0 auto' }}>
          <EditorialCandle candles={candles} width={w} height={300}/>
        </div>
        <div className="figcap" style={{ borderTop:'.5px solid var(--rule-1)', paddingTop:4 }}>
          Figure III — Intraday candles, five-minute bars. Last price highlighted in claret.
        </div>

        {/* Positions strip */}
        <div>
          <div className="sec-head">
            <span className="ttl">Open positions</span>
            <span className="sub">{SYMS.filter(s=>s.qty).length} · long / short</span>
          </div>
          <div style={{ display:'grid',
                        gridTemplateColumns:'repeat(4, 1fr)', gap:0,
                        border:'1px solid var(--rule-3)', marginBottom: 8 }}>
            {[
              { l:'Day P&L',     v:'+$42,118', tone:'up' },
              { l:'NAV',         v:'$3.24M' },
              { l:'Cash',        v:'$2.42M' },
              { l:'Buying power',v:'$5.83M' },
            ].map((s, i) => (
              <div key={i} style={{
                padding:'10px 12px',
                borderRight: i<3 ? '.5px solid var(--rule-2)' : 'none'
              }}>
                <div style={{ fontFamily:'var(--sans)', fontSize:9,
                              letterSpacing:'.14em', textTransform:'uppercase',
                              color:'var(--ink-2)', fontWeight:600 }}>{s.l}</div>
                <div className="num" style={{ fontSize: 18, marginTop: 2,
                                                color: s.tone === 'up' ? 'var(--buy-ink)' : 'var(--ink-0)' }}>
                  {s.v}
                </div>
              </div>
            ))}
          </div>
        </div>

        <TradesBlotter/>
      </div>

      {/* RIGHT — DOM ladder + order ticket */}
      <div style={{ borderLeft:'1px solid var(--rule-2)',
                    overflow:'auto', padding:'14px 14px 24px',
                    display:'flex', flexDirection:'column', gap:14 }}>
        <div>
          <div className="sec-head">
            <span className="ttl">Depth · {sym.sym}</span>
            <span className="sub">L2 · 14 deep</span>
          </div>
          <EdDOMLadder centerPx={sym.px} rows={14}/>
          <div style={{ marginTop:6, padding:'6px 10px',
                        fontFamily:'var(--sans)', fontSize:9, letterSpacing:'.12em',
                        textTransform:'uppercase', color:'var(--ink-2)',
                        display:'flex', justifyContent:'space-between',
                        background:'var(--paper-2)', border:'.5px solid var(--rule-2)' }}>
            <span>Spread <span className="num" style={{ color:'var(--ink-0)' }}>0.14</span></span>
            <span>Imb. <span className="num up">+62%</span></span>
          </div>
        </div>

        <TicketStub sym={sym.sym} refPrice={sym.px}/>
      </div>
    </div>
  );
}

window.ScreenFloor = ScreenFloor;
window.TradesBlotter = TradesBlotter;
