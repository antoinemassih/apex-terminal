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
      <circle cx={size/2} cy={size/2} r={r} fill="none" stroke="var(--line-2)" strokeWidth="3"/>
      <circle cx={size/2} cy={size/2} r={r} fill="none" stroke={color} strokeWidth="3"
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
                    background:'linear-gradient(180deg, #211c17, #1a1611)' }}>
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

          {/* Net Liquidation hero — amber */}
          <TileFrame label="Net Liquidation" num="01" tone="amber">
            <div style={{ display:'flex', flexDirection:'column', justifyContent:'space-between', flex:1, marginTop:6 }}>
              <div>
                <div className="num" style={{ fontSize:64, lineHeight:1, color:'var(--ink-0)', letterSpacing:'-.02em', fontWeight:600 }}>$3.24M</div>
                <div style={{ display:'flex', gap:8, marginTop:10 }}>
                  <span className="pill pill-on">+ $42,118 today</span>
                  <span className="pill pill-on" style={{ color:'var(--buy-ink)' }}>+1.32%</span>
                  <span className="pill pill-on">YTD +18.4%</span>
                </div>
              </div>
              <div style={{ display:'flex', alignItems:'center', justifyContent:'space-between', fontFamily:'var(--mono)', fontSize:10, color:'var(--ink-3)' }}>
                <button className="knob" style={{ width:22, height:22 }}><Icon.ArrowL size={10}/></button>
                <span>February 2026</span>
                <button className="knob" style={{ width:22, height:22 }}><Icon.ArrowR size={10}/></button>
              </div>
            </div>
          </TileFrame>

          {/* P&L 30D */}
          <TileFrame label="P&L · 30D" num="02">
            <div style={{ display:'flex', flexDirection:'column', justifyContent:'space-between', flex:1, marginTop:6 }}>
              <div>
                <div className="num" style={{ fontSize:42, color:'var(--buy-ink)', letterSpacing:'-.02em', fontWeight:600 }}>+17.43%</div>
                <div style={{ marginTop:14 }}>
                  <div style={{ display:'flex', height:14, gap:2 }}>
                    {Array.from({ length: 30 }).map((_, i) => {
                      const v = 0.4 + Math.sin(i * 0.4) * 0.3 + (i / 60);
                      const isUp = v > 0.55;
                      return <div key={i} style={{ flex:1, height: `${Math.min(100, v*100)}%`, alignSelf:'flex-end', background: isUp ? 'var(--buy)' : 'var(--sell)', opacity: .7, borderRadius:1 }}/>;
                    })}
                  </div>
                  <div style={{ display:'flex', justifyContent:'space-between', marginTop:6, fontSize:10, color:'var(--ink-3)' }}>
                    <span>+18.56% real</span>
                    <span>Est +15.78%</span>
                  </div>
                </div>
              </div>
              <div style={{ fontSize:10, color:'var(--ink-3)', textAlign:'center', fontFamily:'var(--mono)' }}>February 2026</div>
            </div>
          </TileFrame>

          {/* Sharpe */}
          <TileFrame label="Sharpe · YTD" num="03">
            <div style={{ display:'flex', alignItems:'center', justifyContent:'center', flex:1, gap:14, marginTop:6 }}>
              <div style={{ position:'relative' }}>
                <Donut value={0.62} color="var(--accent)" size={108}/>
                <div style={{ position:'absolute', inset:0, display:'flex', flexDirection:'column', alignItems:'center', justifyContent:'center' }}>
                  <div className="num" style={{ fontSize:28, color:'var(--ink-0)' }}>2.41</div>
                  <div style={{ fontSize:9, color:'var(--ink-3)', fontFamily:'var(--mono)' }}>Sortino 3.02</div>
                </div>
              </div>
            </div>
          </TileFrame>

          {/* Watchlist (right column, spans 3 rows) */}
          <div className="card-flat" style={{ gridRow:'span 3', display:'flex', flexDirection:'column', minHeight:0, overflow:'hidden' }}>
            <div className="ph">
              <span className="fr-label"><span className="num">04</span> Watchlist</span>
              <div className="seg" style={{ padding:1 }}>
                {['Watch','Pos','Alert'].map((t,i) => (
                  <button key={t} className={`sg ${i===0 ? 'on-acc':''}`} style={{ height:18, padding:'0 7px', fontSize:9.5 }}>{t}</button>
                ))}
              </div>
            </div>
            <div style={{ padding:'8px 12px', borderBottom:'1px solid var(--line-3)', background:'rgba(0,0,0,.18)', display:'flex', justifyContent:'space-between', fontFamily:'var(--mono)', fontSize:10.5, color:'var(--ink-3)', letterSpacing:'.08em', textTransform:'uppercase' }}>
              <span>Megacaps</span>
              <span>12</span>
            </div>
            <div className="scroll" style={{ flex:1, minHeight:0 }}>
              {SYMS.slice(0, 12).map((s, i) => (
                <div key={s.sym} style={{ display:'grid', gridTemplateColumns:'48px 1fr 70px 56px', alignItems:'center',
                                          padding:'8px 12px', borderBottom:'1px solid rgba(255,234,210,.04)',
                                          background: i===0 ? 'rgba(217,152,88,.06)' : 'transparent' }}>
                  <span style={{ fontFamily:'var(--mono)', fontSize:12, fontWeight:600, color:'var(--ink-0)', letterSpacing:'.04em' }}>{s.sym}</span>
                  <span style={{ fontSize:11, color:'var(--ink-3)' }}>{s.name.slice(0, 8)}…</span>
                  <span className="num" style={{ fontSize:11.5, color:'var(--ink-1)', textAlign:'right' }}>{fmtNum(s.px)}</span>
                  <span className={`num ${s.ch>=0 ? 't-up':'t-dn'}`} style={{ fontSize:10.5, textAlign:'right' }}>{fmtCh(s.ch)}</span>
                </div>
              ))}
            </div>
            <div className="split-hT" style={{ padding:14 }}>
              <div className="fr-label" style={{ marginBottom:6 }}><span className="num">05</span> P&L · Today</div>
              <div className="num" style={{ fontSize:24, color:'var(--ink-0)', letterSpacing:'-.02em' }}>+$290,794</div>
              <div style={{ fontSize:10, color:'var(--ink-3)', fontFamily:'var(--mono)', marginTop:2 }}>+1.32% · NAV $3.24M</div>
              <div style={{ display:'grid', gridTemplateColumns:'1fr 1fr', gap:8, marginTop:10 }}>
                <div className="tile-buy" style={{ padding:'6px 8px' }}>
                  <div style={{ fontSize:9, color:'var(--buy-ink)', letterSpacing:'.08em' }}>REALIZED</div>
                  <div className="num" style={{ fontSize:14, color:'var(--ink-0)' }}>+$48k</div>
                </div>
                <div className="tile-buy" style={{ padding:'6px 8px' }}>
                  <div style={{ fontSize:9, color:'var(--buy-ink)', letterSpacing:'.08em' }}>UNREAL.</div>
                  <div className="num" style={{ fontSize:14, color:'var(--ink-0)' }}>+$242k</div>
                </div>
              </div>
            </div>
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

          {/* Top mover */}
          <TileFrame label="Top mover" num="07" tone="sell">
            <div style={{ flex:1, display:'flex', flexDirection:'column', justifyContent:'flex-end' }}>
              <div className="num" style={{ fontSize:56, color:'var(--ink-0)', letterSpacing:'-.04em', fontWeight:600 }}>TSLA</div>
              <div style={{ display:'flex', gap:8, marginTop:6, alignItems:'baseline' }}>
                <span className="pill pill-on" style={{ color:'var(--sell-ink)' }}>−3.88%</span>
                <span className="num" style={{ color:'var(--ink-1)', fontSize:11 }}>254.18</span>
                <span style={{ color:'var(--ink-3)', fontSize:10 }}>· vol 32.4M</span>
              </div>
            </div>
          </TileFrame>

          {/* Cycle time by agent / Trading hours */}
          <TileFrame label="Trading hours · YTD" num="08">
            <div style={{ flex:1, display:'flex', flexDirection:'column', justifyContent:'space-between', marginTop:6 }}>
              <div className="num" style={{ fontSize:40, color:'var(--ink-0)', letterSpacing:'-.02em' }}>554h</div>
              <div style={{ display:'flex', alignItems:'flex-end', gap:6, height:60 }}>
                {['J','F','M','A','M','J','J','A','S','O','N','D'].map((m, i) => {
                  const v = 20 + (i*7) % 36;
                  return (
                    <div key={i} style={{ flex:1, display:'flex', flexDirection:'column', alignItems:'center', gap:3 }}>
                      <div style={{ width:1, background:'var(--accent)', height:`${v}px`, opacity:.5 }}/>
                      <div style={{ width:6, height:6, borderRadius:99, background:'var(--accent)',
                                    boxShadow:'0 0 6px var(--accent-glow)' }}/>
                      <div style={{ fontSize:9, color:'var(--ink-3)', fontFamily:'var(--mono)' }}>{m}</div>
                    </div>
                  );
                })}
              </div>
            </div>
          </TileFrame>

        </div>

        {/* Action row */}
        <div style={{ display:'flex', gap:10, marginTop:14, alignItems:'center' }}>
          <button className="btn btn-accent" style={{ height:32, padding:'0 14px', fontSize:12 }}>
            <Icon.Plus size={12}/> New ticket
          </button>
          <button className="btn" style={{ height:32, padding:'0 14px' }}>Rebalance</button>
          <button className="btn" style={{ height:32, padding:'0 14px', color:'var(--buy-ink)' }}>Hedge book</button>
          <button className="btn" style={{ height:32, padding:'0 14px', color:'var(--accent-ink)' }}>Run scenario</button>
          <button className="btn" style={{ height:32, padding:'0 14px' }}>Export tax lots</button>
          <div style={{ flex:1 }}/>
          <button className="knob" style={{ width:32, height:32 }}><Icon.ArrowR/></button>
          <button className="btn" style={{ height:32, padding:'0 14px' }}>Download statements</button>
        </div>
      </div>
    </div>
  );
}

window.ScreenDashboard = ScreenDashboard;
