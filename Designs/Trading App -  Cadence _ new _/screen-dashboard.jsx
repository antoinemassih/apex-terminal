// screen-dashboard.jsx — Live Tiles dashboard (warm-amber reskin of image 11)

function TileFrame({ label, num, tone = 'plain', children, action }){
  const cls = tone === 'amber' ? 'tile tile-amber' : tone === 'buy' ? 'tile tile-buy' : tone === 'sell' ? 'tile tile-sell' : 'tile';
  return (
    <div className={cls} style={{ padding:14, position:'relative', display:'flex', flexDirection:'column', minHeight:0 }}>
      <div style={{ display:'flex', alignItems:'center', justifyContent:'space-between', marginBottom:6 }}>
        <span className="fr-label">{num && <span className="num">{num}</span>}{label}</span>
        {action || <button className="btn-ghost" style={{ width:20, height:20, borderRadius:4, display:'inline-flex', alignItems:'center', justifyContent:'center' }}>···</button>}
      </div>
      {children}
    </div>
  );
}

function Donut({ value = 0.62, color = 'var(--accent)', size = 84 }){
  const r = (size - 10) / 2;
  const c = 2 * Math.PI * r;
  return (
    <svg width={size} height={size}>
      <circle cx={size/2} cy={size/2} r={r} fill="none" stroke="var(--line-2)" strokeWidth="10"/>
      <circle cx={size/2} cy={size/2} r={r} fill="none" stroke={color} strokeWidth="10"
              strokeDasharray={`${c*value} ${c}`} strokeLinecap="round"
              transform={`rotate(-90 ${size/2} ${size/2})`}/>
    </svg>
  );
}

function ScreenDashboard(){
  const positions = SYMS.filter(s => s.qty);
  const cycleData = useMemo(() => {
    return ['NVDA','AAPL','MSFT','TSLA','META','AMD','GOOGL','SPY','AMZN','AVGO','NFLX','JPM'].map((sym, i) => {
      const r = mulberry32(i + 5);
      return Array.from({ length: 30 }, () => 0.2 + r() * 0.8);
    });
  }, []);

  return (
    <div style={{ display:'flex', flexDirection:'column', height:'100%' }}>
      {/* Sub-toolbar (timeframe + symbols ribbon) */}
      <div className="split-h" style={{ display:'flex', alignItems:'center', gap:14, padding:'8px 14px',
                    backgroundColor:'var(--bg-2)' }}>
        <div className="seg" style={{ padding:1 }}>
          {['1m','5m','15m','1H','4H','1D','1W'].map((t,i) => (
            <button key={t} className={`sg ${i===2 ? 'on-acc' : ''}`}
                    style={{ height:20, padding:'0 8px', fontSize:10.5 }}>{t}</button>
          ))}
        </div>
        <div style={{ width:1, height:18, background:'var(--line-3)', boxShadow:'1px 0 0 var(--hi-1)' }}/>
        <div style={{ display:'flex', gap:2 }}>
          {['MA','Oscillators','Trends','Volatility','Volume'].map(t => (
            <button key={t} className="pill" style={{ height:20 }}>
              {t} <Icon.Caret size={9}/>
            </button>
          ))}
        </div>
        <div style={{ flex:1 }}/>
        <div style={{ display:'flex', gap:14, fontFamily:'var(--mono)', fontSize:10.5 }}>
          {['AAPL','MSFT','TSLA','META','AMZN','GOOGL','AMD','AVGO','NFLX','JPM'].map(sym => {
            const s = SYMS.find(x => x.sym === sym);
            if (!s) return null;
            return (
              <div key={sym} style={{ display:'flex', gap:5 }}>
                <span style={{ color:'var(--ink-2)', fontWeight:600 }}>{sym}</span>
                <span style={{ color:'var(--ink-1)' }}>{fmtNum(s.px)}</span>
                <span className={s.ch>=0?'t-up':'t-dn'}>{fmtCh(s.ch)}</span>
              </div>
            );
          })}
        </div>
      </div>

      {/* Tile grid */}
      <div className="scroll" style={{ flex:1, padding:14, minHeight:0 }}>
        <div style={{ display:'grid', gridTemplateColumns:'1.4fr 1fr 1fr 280px', gridAutoRows:'minmax(180px, auto)', gap:12 }}>

          {/* Net Liquidation hero — amber, dot grid, vertical-line chart */}
          <TileFrame label="Net Liquidation" num="01">
            <div className="dotgrid" style={{ position:'absolute', inset:0, opacity:.45, pointerEvents:'none' }}/>
            <div style={{ position:'relative', display:'flex', flexDirection:'column', justifyContent:'space-between', flex:1, marginTop:6 }}>
              <div style={{ display:'flex', alignItems:'flex-start', justifyContent:'space-between', gap:14 }}>
                <div>
                  <div className="bignum" style={{ fontSize:84 }}>+1.32<span style={{ color:'var(--ink-2)', fontSize:50 }}>%</span></div>
                  <div style={{ marginTop:8, fontFamily:'var(--mono)', fontSize:11, color:'var(--ink-2)', letterSpacing:'.04em' }}>
                    NAV <span style={{ color:'var(--ink-0)' }}>$3.24M</span> · +$42,118 today · YTD +18.4%
                  </div>
                </div>
              </div>
              <div className="vlines" style={{ height:78, color:'var(--ink-0)', marginTop:14 }}>
                {Array.from({ length: 64 }).map((_, i) => {
                  const seed = (Math.sin(i * 0.55) + Math.sin(i * 0.21 + 1.4) + Math.cos(i * 0.13)) / 3;
                  const v = 0.20 + Math.abs(seed) * 0.85 + (i / 200);
                  return <i key={i} style={{ height: `${Math.min(100, v*100)}%` }}/>;
                })}
              </div>
              <div style={{ display:'flex', alignItems:'center', justifyContent:'space-between', marginTop:14, gap:10 }}>
                <button className="pill-btn" style={{ height:34, padding:'0 18px', fontSize:10.5 }}>Compare</button>
                <div style={{ display:'flex', alignItems:'center', gap:10, fontFamily:'var(--mono)', fontSize:11, color:'var(--ink-2)' }}>
                  <button className="knob" style={{ width:24, height:24 }}><Icon.ArrowL size={10}/></button>
                  <span style={{ letterSpacing:'.04em' }}>February 2026</span>
                  <button className="knob" style={{ width:24, height:24 }}><Icon.ArrowR size={10}/></button>
                </div>
                <button className="pill-btn" style={{ height:34, padding:'0 18px', fontSize:10.5 }}>Statements</button>
              </div>
            </div>
          </TileFrame>

          {/* P&L 30D — vertical-line chart */}
          <TileFrame label="P&L · 30D" num="02">
            <div style={{ display:'flex', flexDirection:'column', justifyContent:'space-between', flex:1, marginTop:6 }}>
              <div className="bignum" style={{ fontSize:48, color:'var(--buy-ink)' }}>+17.43<span style={{ fontSize:24, color:'var(--ink-3)' }}>%</span></div>
              <div className="vlines" style={{ height:70, color:'var(--ink-0)', marginTop:8 }}>
                {Array.from({ length: 30 }).map((_, i) => {
                  const v = 0.4 + Math.sin(i * 0.4) * 0.3 + (i / 60);
                  const isUp = v > 0.55;
                  return <i key={i} style={{ height:`${Math.min(100, v*100)}%`, background: isUp ? 'var(--buy)' : 'var(--sell)', opacity:.8 }}/>;
                })}
              </div>
              <div style={{ display:'flex', justifyContent:'space-between', marginTop:6, fontFamily:'var(--mono)', fontSize:10, color:'var(--ink-3)', letterSpacing:'.06em' }}>
                <span>+18.56% real</span>
                <span>est +15.78%</span>
              </div>
            </div>
          </TileFrame>

          {/* Sharpe — dot-pattern halo + bold central number */}
          <TileFrame label="Sharpe · YTD" num="03">
            <div className="dotgrid" style={{ position:'absolute', inset:0, opacity:.5, pointerEvents:'none' }}/>
            <div style={{ position:'relative', display:'flex', alignItems:'center', justifyContent:'center', flex:1, marginTop:6 }}>
              <div style={{ position:'relative', display:'flex', alignItems:'center', justifyContent:'center' }}>
                <Donut value={0.62} color="var(--accent)" size={132}/>
                <div style={{ position:'absolute', inset:0, display:'flex', flexDirection:'column', alignItems:'center', justifyContent:'center' }}>
                  <div className="bignum" style={{ fontSize:42 }}>2.41</div>
                  <div style={{ fontSize:9.5, color:'var(--ink-3)', fontFamily:'var(--mono)', letterSpacing:'.10em', marginTop:4 }}>SORTINO 3.02</div>
                </div>
              </div>
            </div>
          </TileFrame>

          {/* Watchlist (right column, spans 3 rows) — shared component */}
          <div className="card-flat" style={{ gridRow:'span 3', display:'flex', flexDirection:'column', minHeight:0, overflow:'hidden' }}>
            <WatchlistPanel
              num="04"
              title="Watchlist"
              group="Megacaps"
              count={12}
              showFooter={true}/>
          </div>

          {/* Positions table — spans 2 cols */}
          <TileFrame label="Positions · 2 pending" num="06">
            <div style={{ display:'grid', gridTemplateColumns:'40px 1fr 1fr 1fr 60px',
                          padding:'8px 0', fontFamily:'var(--mono)', fontSize:10.5, color:'var(--ink-3)',
                          letterSpacing:'.08em', textTransform:'uppercase',
                          borderBottom:'1px solid var(--line-3)' }}>
              <span>SYM</span><span>QTY</span><span style={{ textAlign:'right' }}>AVG</span>
              <span style={{ textAlign:'right' }}>LAST</span><span style={{ textAlign:'right' }}>Δ</span>
            </div>
            <div style={{ flex:1, minHeight:0, overflow:'auto' }}>
              {positions.map(p => (
                <div key={p.sym} style={{ display:'grid', gridTemplateColumns:'40px 1fr 1fr 1fr 60px',
                                          padding:'7px 0', alignItems:'center',
                                          fontFamily:'var(--mono)', fontSize:12,
                                          borderBottom:'1px solid rgba(255,234,210,.04)' }} className="cell-hover">
                  <span style={{ color:'var(--ink-0)', fontWeight:600 }}>{p.sym}</span>
                  <span className="num" style={{ color: p.qty < 0 ? 'var(--sell-ink)':'var(--ink-1)' }}>{p.qty}</span>
                  <span className="num" style={{ color:'var(--ink-2)', textAlign:'right' }}>{fmtNum(p.avg)}</span>
                  <span className="num" style={{ color:'var(--ink-0)', textAlign:'right' }}>{fmtNum(p.px)}</span>
                  <span className={`num ${p.ch>=0?'t-up':'t-dn'}`} style={{ textAlign:'right', fontSize:11 }}>{fmtCh(p.ch)}</span>
                </div>
              ))}
            </div>
          </TileFrame>

          {/* Top mover — huge ticker treatment */}
          <TileFrame label="Top mover" num="07">
            <div className="dotgrid" style={{ position:'absolute', inset:0, opacity:.35, pointerEvents:'none' }}/>
            <div style={{ position:'relative', flex:1, display:'flex', flexDirection:'column', justifyContent:'flex-end' }}>
              <div className="bignum" style={{ fontSize:72, color:'var(--ink-0)' }}>TSLA</div>
              <div className="vlines" style={{ height:30, color:'var(--sell)', marginTop:10, opacity:.7 }}>
                {Array.from({ length:36 }).map((_,i) => {
                  const v = 0.3 + Math.abs(Math.sin(i*0.7) * 0.6) - i/72;
                  return <i key={i} style={{ height:`${Math.max(8, v*100)}%` }}/>;
                })}
              </div>
              <div style={{ display:'flex', gap:8, marginTop:8, alignItems:'baseline' }}>
                <span className="pill pill-on" style={{ color:'var(--sell-ink)' }}>−3.88%</span>
                <span className="num" style={{ color:'var(--ink-1)', fontSize:11 }}>254.18</span>
                <span style={{ color:'var(--ink-3)', fontSize:10 }}>· vol 32.4M</span>
              </div>
            </div>
          </TileFrame>

          {/* Cycle time by agent / Trading hours */}
          <TileFrame label="Trading hours · YTD" num="08">
            <div style={{ flex:1, display:'flex', flexDirection:'column', justifyContent:'space-between', marginTop:6 }}>
              <div className="bignum" style={{ fontSize:46 }}>554<span style={{ fontSize:24, color:'var(--ink-3)' }}>h</span></div>
              <div className="vlines" style={{ height:60, color:'var(--accent)', marginTop:10 }}>
                {['J','F','M','A','M','J','J','A','S','O','N','D'].flatMap((m, mi) =>
                  Array.from({ length: 6 }).map((_, j) => {
                    const v = 0.30 + Math.abs(Math.sin(mi*0.7 + j*0.4)) * 0.7;
                    return <i key={`${mi}-${j}`} style={{ height:`${Math.min(100, v*100)}%` }}/>;
                  })
                )}
              </div>
              <div style={{ display:'flex', justifyContent:'space-between', marginTop:4, fontFamily:'var(--mono)', fontSize:9.5, color:'var(--ink-3)', letterSpacing:'.06em' }}>
                {['J','F','M','A','M','J','J','A','S','O','N','D'].map((m,i) => <span key={i}>{m}</span>)}
              </div>
            </div>
          </TileFrame>

        </div>

        {/* Action row */}
        <div style={{ display:'flex', gap:10, marginTop:18, alignItems:'center', flexWrap:'wrap' }}>
          <button className="pill-btn" style={{ background:'linear-gradient(180deg, color-mix(in oklab, var(--accent) 88%, white 12%), var(--accent))', color:'#1a0f06', borderColor:'rgba(0,0,0,.4)' }}>
            <Icon.Plus size={12}/> New ticket
          </button>
          <button className="pill-btn">Rebalance</button>
          <button className="pill-btn" style={{ color:'var(--buy-ink)' }}>Hedge book</button>
          <button className="pill-btn" style={{ color:'var(--accent-ink)' }}>Run scenario</button>
          <button className="pill-btn">Export tax lots</button>
          <div style={{ flex:1 }}/>
          <button className="pill-btn">Download statements <Icon.ArrowR size={12}/></button>
        </div>
      </div>
    </div>
  );
}

window.ScreenDashboard = ScreenDashboard;
