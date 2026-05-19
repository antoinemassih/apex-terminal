// ed-dashboard-og.jsx — Dashboard · OG
// Recreates the original first-pass newspaper front page design:
// lead story w/ drop-cap, hero NAV chart, watchlist, positions ledger,
// sectors league. Streamlined, no later additions (no Day Pulse, no
// attribution, no risk register).

function ScreenDashboardOG({ activeSym, setActiveSym }){
  const positions = SYMS.filter(s => s.qty);
  const longCount  = positions.filter(p => p.qty > 0).length;
  const shortCount = positions.filter(p => p.qty < 0).length;
  const navSeries  = useMemo(() => makeArea(45, 17, 280, 6).map(v => 2.96 + v * 0.001), []);
  const topGain    = useMemo(() => [...SYMS].sort((a,b)=>b.ch-a.ch)[0], []);
  const topLose    = useMemo(() => [...SYMS].sort((a,b)=>a.ch-b.ch)[0], []);

  return (
    <div className="page-inner">

      {/* Front page top rail */}
      <div className="grid-12" style={{ paddingTop: 24 }}>

        {/* LEAD STORY — col 4 */}
        <div className="col-4">
          <div className="eyebrow">Editor's Note · 17 May</div>
          <h1 className="headline h1">
            Book runs to a fresh high as rally widens beyond the megacaps.
          </h1>
          <p className="dek">
            Net liquidation crests $3.24 million; risk holds in check despite a softer Treasury bid.
          </p>
          <p className="lede">
            The book closed the New York session up 1.32 per cent on the day, with realised gains
            of $48,200 and a further $242,000 in unrealised mark-to-market across thirteen open
            positions. Volatility eased into the close — a calm tone that has so far rewarded
            disciplined sizing.
          </p>
          <p style={{ fontFamily:'var(--serif)', fontSize:14, lineHeight:1.55, color:'var(--ink-1)', marginTop:10 }}>
            Trader's notes are anchored to {topGain.sym}, which led the desk's gainers at
            <span className="num up"> {fmtCh(topGain.ch)}</span>, offsetting a soft showing from
            {' '}{topLose.sym} at <span className="num dn">{fmtCh(topLose.ch)}</span>.
          </p>
          <div className="byline">
            <span className="dateline">New York, May 17 —</span> by
            <span className="name"> The Desk</span> · 16:14 ET
          </div>

          <div className="pullquote" style={{ marginTop: 24 }}>
            "Edge is what survives a quiet tape."
            <span className="attrib">House view · Weekly memo</span>
          </div>

          <p style={{ fontFamily:'var(--serif)', fontSize:13.5, lineHeight:1.55, color:'var(--ink-1)', marginTop:8 }}>
            Cash on hand stands at $2.42m, leaving buying power of $5.83m at standard margin —
            a wider-than-usual cushion ahead of Thursday's CPI print.
          </p>
        </div>

        {/* HERO CHART — col 5 */}
        <div className="col-5 vrule">
          <div className="eyebrow muted">Figure I · Account NAV · 45 sessions</div>

          <div className="hero-quote">
            <span className="px">3,240<span className="frac">,194</span></span>
            <span className="ch up">+$42,118</span>
            <span className="ch up">+1.32%</span>
            <span className="pct">Net Liquidation · USD</span>
          </div>

          <div style={{ marginTop: 12, width:'100%' }}>
            <EditorialLine data={navSeries} width={560} height={210}
              annotation={{ idx: 28, label:'EARNINGS' }}/>
          </div>
          <div className="figcap">
            Account NAV (USD m). Annotation marks the 16 April Nvidia print; +18.4% YTD.
          </div>

          <dl className="dl-stats" style={{ marginTop: 18 }}>
            <div className="pair"><dt>Realised P&amp;L</dt>
              <dd style={{ color:'var(--buy)' }}>+$48,200<span className="pct">+1.78%</span></dd></div>
            <div className="pair"><dt>Unrealised</dt>
              <dd style={{ color:'var(--buy)' }}>+$242,594<span className="pct">+8.16%</span></dd></div>
            <div className="pair"><dt>Sharpe · YTD</dt>
              <dd>2.41<span className="pct">Sortino 3.02</span></dd></div>
            <div className="pair"><dt>Cash</dt><dd>$2.42M</dd></div>
            <div className="pair"><dt>Buying Power</dt><dd>$5.83M</dd></div>
          </dl>

          <div className="hrule" style={{ marginTop: 18 }}>
            <div className="eyebrow">Top mover · Underperformer</div>
            <div style={{ display:'flex', gap:14, alignItems:'baseline', marginTop:4 }}>
              <span className="headline h3">Tesla</span>
              <span className="num dn" style={{ fontSize:18, fontWeight:600 }}>−3.88%</span>
              <span style={{ color:'var(--ink-2)', fontFamily:'var(--mono)' }}>$254.18</span>
            </div>
            <p style={{ fontFamily:'var(--serif)', fontSize:13, lineHeight:1.5, color:'var(--ink-1)', marginTop: 6 }}>
              Tesla led decliners after the company missed Q3 vehicle delivery estimates by
              roughly 8,200 units. The desk holds no current exposure to the name.
            </p>
          </div>
        </div>

        {/* WATCH LIST — col 3 */}
        <div className="col-3 vrule">
          <div className="sec-head">
            <span className="ttl">Watch list</span>
            <span className="sub">{SYMS.length} · live</span>
          </div>
          <DashWatchTable syms={SYMS} activeSym={activeSym} setActiveSym={setActiveSym}/>
        </div>
      </div>

      {/* Below the fold */}
      <div className="grid-12 hrule-thick" style={{ marginTop: 32 }}>

        {/* POSITIONS LEDGER — col 8 */}
        <div className="col-8">
          <div className="sec-head">
            <span className="ttl">Open Positions · The Ledger</span>
            <span className="sub">{longCount} long · {shortCount} short · 16:00 ET</span>
          </div>

          <table className="table">
            <thead>
              <tr>
                <th>Issue</th>
                <th className="r">Quantity</th>
                <th className="r">Avg cost</th>
                <th className="r">Last</th>
                <th>30 sess.</th>
                <th className="r">Mkt value</th>
                <th className="r">P&amp;L</th>
                <th className="r">%</th>
              </tr>
            </thead>
            <tbody>
              <tr className="subhead"><td colSpan={8}>Long positions · {longCount}</td></tr>
              {positions.filter(p => p.qty > 0).map(p => {
                const mv = Math.abs(p.qty) * p.px;
                const pl = p.qty * (p.px - p.avg);
                const spark = makeArea(20, p.sym.length*23, p.px*0.95, p.px*0.01);
                return (
                  <tr key={p.sym}>
                    <td>
                      <div className="watch-cell-sym">
                        <span className="ticker">{p.sym}</span>
                        <span className="desc">{p.name}</span>
                      </div>
                    </td>
                    <td className="r">{Math.abs(p.qty).toLocaleString()}</td>
                    <td className="r">{fmtNum(p.avg)}</td>
                    <td className="r px">{fmtNum(p.px)}</td>
                    <td><EditorialSpark data={spark} w={70} h={18} fill/></td>
                    <td className="r">${mv.toLocaleString('en-US',{maximumFractionDigits:0})}</td>
                    <td className={`r ${pl>=0?'up':'dn'}`}>
                      {pl>=0?'+':'−'}${Math.abs(pl).toLocaleString('en-US',{maximumFractionDigits:0})}
                    </td>
                    <td className={`r ${p.ch>=0?'up':'dn'}`}>{fmtCh(p.ch)}</td>
                  </tr>
                );
              })}
              <tr className="subhead"><td colSpan={8}>Short positions · {shortCount}</td></tr>
              {positions.filter(p => p.qty < 0).map(p => {
                const mv = Math.abs(p.qty) * p.px;
                const pl = p.qty * (p.px - p.avg);
                const spark = makeArea(20, p.sym.length*29, p.px*0.95, p.px*0.012);
                return (
                  <tr key={p.sym}>
                    <td>
                      <div className="watch-cell-sym">
                        <span className="ticker">{p.sym}</span>
                        <span className="desc">{p.name}</span>
                      </div>
                    </td>
                    <td className="r">({Math.abs(p.qty).toLocaleString()})</td>
                    <td className="r">{fmtNum(p.avg)}</td>
                    <td className="r px">{fmtNum(p.px)}</td>
                    <td><EditorialSpark data={spark} w={70} h={18} fill/></td>
                    <td className="r">${mv.toLocaleString('en-US',{maximumFractionDigits:0})}</td>
                    <td className={`r ${pl>=0?'up':'dn'}`}>
                      {pl>=0?'+':'−'}${Math.abs(pl).toLocaleString('en-US',{maximumFractionDigits:0})}
                    </td>
                    <td className={`r ${p.ch>=0?'up':'dn'}`}>{fmtCh(p.ch)}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>

        {/* SECTOR LEAGUE TABLE — col 4 */}
        <div className="col-4 vrule">
          <div className="sec-head">
            <span className="ttl">Sectors · League Table</span>
            <span className="sub">S&amp;P 500 · today</span>
          </div>

          <table className="league">
            <thead>
              <tr>
                <th>#</th>
                <th>Sector</th>
                <th>Move</th>
                <th className="r">Day</th>
              </tr>
            </thead>
            <tbody>
              {[
                { name:'Information Technology', ch:+1.84 },
                { name:'Communication Services', ch:+0.42 },
                { name:'Consumer Discretionary', ch:+0.31 },
                { name:'Financials',             ch:+0.12 },
                { name:'Industrials',            ch:-0.08 },
                { name:'Healthcare',             ch:-0.21 },
                { name:'Materials',              ch:-0.34 },
                { name:'Real Estate',            ch:-0.42 },
                { name:'Utilities',              ch:-0.58 },
                { name:'Energy',                 ch:-1.12 },
                { name:'Consumer Staples',       ch:-1.31 },
              ].map((s,i) => {
                const w = Math.min(100, Math.abs(s.ch) * 40);
                return (
                  <tr key={s.name}>
                    <td className="rk">{String(i+1).padStart(2,'0')}</td>
                    <td className="sector">{s.name}</td>
                    <td><span className={`bar ${s.ch>=0?'up':'dn'}`} style={{ width: `${w}px` }}/></td>
                    <td className={`r ${s.ch>=0?'up':'dn'}`}>{fmtCh(s.ch)}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>

          <div className="hrule" style={{ marginTop: 20 }}>
            <div className="eyebrow">From the Editor</div>
            <h3 className="headline h4">A note on positioning into CPI.</h3>
            <p style={{ fontFamily:'var(--serif)', fontSize:13, lineHeight:1.55, color:'var(--ink-1)', marginTop:6 }}>
              We retain a modest long bias into Thursday's print, hedged via the SPY put-spread
              held since Monday. The desk's view: a 0.2pp surprise either way is already priced;
              tape risk sits in second-order positioning.
            </p>
            <div className="byline" style={{ marginTop:8 }}>
              <span className="name">Editorial Desk</span> · 16:08 ET
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

window.ScreenDashboardOG = ScreenDashboardOG;
