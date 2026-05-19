// ed-dashboard-alt.jsx — Dashboard · ALT
// Brings back the broadsheet masthead and the "Session at a glance"
// row of tickers / numbers from the very first newspaper-aesthetic
// design. A more ceremonial front page.

function GrandMasthead(){
  const today = new Date();
  const dateStr = today.toLocaleDateString('en-US', {
    weekday:'long', month:'long', day:'numeric', year:'numeric'
  });
  return (
    <div style={{
      borderBottom: '3px double var(--ink-0)',
      padding: '18px 0 10px',
      display: 'grid',
      gridTemplateColumns: '1fr auto 1fr',
      alignItems: 'end',
      gap: 24,
    }}>
      <div style={{
        fontFamily: 'var(--sans)', fontSize: 10, letterSpacing: '.08em',
        textTransform: 'uppercase', color: 'var(--ink-2)',
        display: 'flex', flexDirection: 'column', gap: 3,
      }}>
        <div>{dateStr}</div>
        <div>Vol. CLXIV · No. 4,827</div>
        <div>New York · 15.7°C · partly cloudy</div>
      </div>

      <div>
        <div style={{
          fontFamily: 'var(--serif)', fontSize: 56, fontWeight: 700,
          fontStyle: 'italic', letterSpacing: '-.02em', lineHeight: .95,
          color: 'var(--ink-0)', textAlign: 'center',
          fontFeatureSettings: '"kern", "liga", "swsh", "onum"',
        }}>
          <span>Alto</span>{' '}
          <span style={{ color:'var(--claret)', fontWeight: 400 }}>&amp;</span>{' '}
          <span>Co.</span>
        </div>
        <div style={{
          fontFamily: 'var(--sans)', fontSize: 9.5, letterSpacing: '.24em',
          textTransform: 'uppercase', color: 'var(--ink-2)',
          textAlign: 'center', marginTop: 2,
        }}>
          The Daily · Established 2026 · Trading Edition
        </div>
      </div>

      <div style={{
        fontFamily: 'var(--sans)', fontSize: 10, letterSpacing: '.08em',
        textTransform: 'uppercase', color: 'var(--ink-2)',
        display: 'flex', flexDirection: 'column', gap: 3,
        textAlign: 'right', alignItems: 'flex-end',
      }}>
        <div>Closing prices · 16:00 ET</div>
        <div style={{ display:'flex', gap: 12 }}>
          <span><span style={{ color:'var(--ink-0)', fontWeight:500 }}>S&amp;P</span> 5,847.22</span>
          <span style={{ color:'var(--buy)' }}>+0.71%</span>
        </div>
        <div style={{ display:'flex', gap: 12 }}>
          <span><span style={{ color:'var(--ink-0)', fontWeight:500 }}>10Y</span> 4.184%</span>
          <span style={{ color:'var(--sell)' }}>−2.6 bp</span>
        </div>
      </div>
    </div>
  );
}

// Ticker grid — 8 cells, each a mini "stock card"
function SessionGlance(){
  const cells = SYMS.slice(0, 8);
  return (
    <div style={{
      display: 'grid',
      gridTemplateColumns: 'repeat(8, 1fr)',
      borderTop: '2px solid var(--ink-0)',
      borderBottom: '2px solid var(--ink-0)',
      margin: '14px 0 0',
    }}>
      {cells.map((s, i) => (
        <div key={s.sym} style={{
          padding: '12px 12px',
          borderRight: i < cells.length - 1 ? '.5px solid var(--rule-2)' : 'none',
          background: i === 0 ? 'var(--paper-2)' : 'var(--paper)',
        }}>
          <div style={{
            fontFamily:'var(--sans)', fontSize:10, letterSpacing:'.14em',
            textTransform:'uppercase', color:'var(--ink-2)', fontWeight: 600,
          }}>{s.sym}</div>
          <div style={{
            fontFamily:'var(--serif)', fontStyle:'italic', fontSize: 10.5,
            color:'var(--ink-3)', marginBottom: 6, lineHeight: 1.2,
          }}>{s.name}</div>
          <div className="num" style={{ fontSize: 17, color:'var(--ink-0)', fontWeight: 500 }}>
            {fmtNum(s.px)}
          </div>
          <div style={{ display:'flex', justifyContent:'space-between', alignItems:'baseline', marginTop: 2 }}>
            <span className={`num ${s.ch>=0?'up':'dn'}`}
                  style={{ fontSize: 11.5, fontWeight: 600 }}>
              {fmtCh(s.ch)}
            </span>
            <span style={{ fontFamily:'var(--sans)', fontSize: 9, letterSpacing:'.08em',
                            color:'var(--ink-3)', textTransform:'uppercase' }}>
              vol {(20 + (s.sym.length * 4.3)).toFixed(1)}M
            </span>
          </div>
        </div>
      ))}
    </div>
  );
}

// Hero stat strip — the "session at a glance" row of large display figures
function HeroStrip(){
  const items = [
    { label:'Day P&L',    value:'+$42,118',  sub:'+1.32%',   tone:'up' },
    { label:'Net Liq.',   value:'$3.24',     unit:'M',       sub:'NAV · USD' },
    { label:'YTD',        value:'+18.4',     unit:'%',       sub:'vs. SPY +12.0%', tone:'up' },
    { label:'Sharpe',     value:'2.41',                       sub:'Sortino 3.02' },
    { label:'Drawdown',   value:'−1.8',      unit:'%',       sub:'MTD · vs −5% cap', tone:'dn' },
    { label:'Cash',       value:'$2.42',     unit:'M',       sub:'BP $5.83M' },
  ];
  return (
    <div style={{
      display:'grid',
      gridTemplateColumns: `repeat(${items.length}, 1fr)`,
      borderTop: '1px solid var(--rule-2)',
      borderBottom: '1px solid var(--rule-2)',
      margin: '0 0 18px',
    }}>
      {items.map((it, i) => (
        <div key={i} style={{
          padding: '16px 16px',
          borderRight: i < items.length - 1 ? '1px solid var(--rule-2)' : 'none',
        }}>
          <div style={{
            fontFamily:'var(--sans)', fontSize: 10, letterSpacing: '.14em',
            textTransform:'uppercase', color:'var(--ink-2)', fontWeight: 600,
          }}>{it.label}</div>
          <div style={{
            fontFamily:'var(--serif)', fontWeight: 700, fontSize: 34,
            letterSpacing:'-.025em', lineHeight: 1, marginTop: 4,
            color: it.tone === 'up' ? 'var(--buy-ink)' :
                   it.tone === 'dn' ? 'var(--sell-ink)' : 'var(--ink-0)',
            fontVariantNumeric:'tabular-nums',
          }}>
            {it.value}
            {it.unit && <span style={{ color:'var(--ink-2)', fontSize: 18, marginLeft: 2 }}>{it.unit}</span>}
          </div>
          {it.sub && (
            <div style={{
              fontFamily:'var(--serif)', fontStyle:'italic', fontSize: 12,
              color:'var(--ink-2)', marginTop: 4,
            }}>{it.sub}</div>
          )}
        </div>
      ))}
    </div>
  );
}

function ScreenDashboardAlt({ activeSym, setActiveSym }){
  const positions = SYMS.filter(s => s.qty);
  const navSeries = useMemo(() => makeArea(45, 17, 280, 6).map(v => 2.96 + v * 0.001), []);

  return (
    <div className="page-inner">
      <GrandMasthead/>

      <div style={{ marginTop: 16 }}>
        <div className="eyebrow muted" style={{ marginBottom: 4 }}>
          Session at a glance · 16:00 ET
        </div>
        <SessionGlance/>
      </div>

      <div style={{ marginTop: 18 }}>
        <HeroStrip/>
      </div>

      {/* Lead column + chart */}
      <div className="grid-12">
        <div className="col-7">
          <div className="eyebrow">Letter from the Desk · 17 May</div>
          <h1 className="headline h1">
            The book runs to fresh highs as the rally broadens.
          </h1>
          <p className="dek">
            A measured close to a noisy week. Net liquidation lifted
            $42,118 on the day; volatility sits comfortably below cap.
          </p>
          <p className="lede">
            The New York session closed firm on the back of fresh accumulation
            in semiconductors and broad participation across communication
            services. The desk's book added 1.32 per cent, lifting year-to-date
            performance to +18.4 — comfortably ahead of the S&amp;P, and within
            risk limits on every axis save sector concentration, which is
            unchanged from yesterday.
          </p>
          <div className="byline">
            <span className="dateline">New York, May 17 —</span> by
            <span className="name"> The Desk</span> · 16:14 ET
          </div>

          <div style={{ marginTop: 18, width:'100%' }}>
            <EditorialLine data={navSeries} width={640} height={170}
                            annotation={{ idx: 28, label:'EARNINGS' }}/>
          </div>
          <div className="figcap" style={{ borderTop:'.5px solid var(--rule-1)', paddingTop: 5 }}>
            Figure I — Account NAV across 45 sessions; claret marker indicates last print.
          </div>
        </div>

        <div className="col-5 vrule">
          <div className="sec-head">
            <span className="ttl">Watch list</span>
            <span className="sub">{SYMS.length} issues · live</span>
          </div>
          <DashWatchTable syms={SYMS}
                            activeSym={activeSym}
                            setActiveSym={setActiveSym}/>
        </div>
      </div>

      {/* Positions ledger — full width */}
      <div className="hrule-thick" style={{ marginTop: 22, paddingTop: 14 }}>
        <div className="sec-head">
          <span className="ttl">Open positions · the ledger</span>
          <span className="sub">{positions.length} open · long / short · 16:00 ET</span>
        </div>
        <table className="table">
          <thead>
            <tr>
              <th>Issue</th>
              <th className="r">Qty</th>
              <th className="r">Avg</th>
              <th className="r">Last</th>
              <th>30 sess.</th>
              <th className="r">Mkt value</th>
              <th className="r">P&amp;L</th>
              <th className="r">%</th>
            </tr>
          </thead>
          <tbody>
            {positions.map(p => {
              const mv = Math.abs(p.qty) * p.px;
              const pl = p.qty * (p.px - p.avg);
              const spark = makeArea(20, p.sym.length*23, p.px*0.95, p.px*0.01);
              return (
                <tr key={p.sym}>
                  <td>
                    <div className="watch-cell-sym">
                      <span className="ticker">{p.sym}
                        <span style={{
                          fontFamily:'var(--sans)', fontSize:9, letterSpacing:'.14em',
                          color: p.qty < 0 ? 'var(--sell-ink)' : 'var(--buy-ink)',
                          marginLeft: 8, fontWeight: 600,
                        }}>{p.qty < 0 ? 'SHORT' : 'LONG'}</span>
                      </span>
                      <span className="desc">{p.name}</span>
                    </div>
                  </td>
                  <td className="r">{p.qty < 0 ? `(${Math.abs(p.qty).toLocaleString()})` : p.qty.toLocaleString()}</td>
                  <td className="r">{fmtNum(p.avg)}</td>
                  <td className="r px">{fmtNum(p.px)}</td>
                  <td><EditorialSpark data={spark} w={80} h={18} fill/></td>
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

      {/* Pull quote */}
      <div className="pullquote" style={{ marginTop: 28 }}>
        "Edge is what survives a quiet tape."
        <span className="attrib">House view · weekly memo · 12 May</span>
      </div>
    </div>
  );
}

window.ScreenDashboardAlt = ScreenDashboardAlt;
