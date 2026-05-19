// screen-terminal.jsx — Terminal: bento of news/heatmap/order/positions with NVDA hero

function HeroPrice(){
  const data = useMemo(() => makeArea(80, 11, 1180, 4), []);
  return (
    <div style={{ display:'grid', gridTemplateColumns:'1.4fr 1fr 0.9fr', gap:0, padding:'18px 20px',
                  borderBottom:'1px solid var(--line-3)', boxShadow:'0 1px 0 rgba(255,234,210,.025)', backgroundColor:'var(--bg-1)' }}>
      <div>
        <div style={{ display:'flex', alignItems:'baseline', gap:10 }}>
          <span style={{ fontWeight:700, fontSize:13, letterSpacing:'.04em' }}>NVDA</span>
          <span style={{ color:'var(--ink-3)', fontSize:11 }}>NVIDIA Corp · NASDAQ</span>
        </div>
        <div className="num" style={{ fontSize:78, lineHeight:1, color:'var(--ink-0)', letterSpacing:'-.03em', fontWeight:600, marginTop:8 }}>
          1,284<span style={{ color:'var(--ink-2)' }}>.32</span>
        </div>
        <div style={{ display:'flex', alignItems:'center', gap:6, marginTop:8 }}>
          <span className="num t-up" style={{ fontSize:13 }}>+28.40</span>
          <span style={{ color:'var(--ink-3)' }}>·</span>
          <span className="num t-up" style={{ fontSize:13 }}>+2.26%</span>
          <span className="pill" style={{ marginLeft:8, color:'var(--ink-3)', fontSize:9.5 }}>VS PREV CLOSE</span>
        </div>
      </div>
      <div style={{ display:'flex', flexDirection:'column', justifyContent:'space-between' }}>
        <div style={{ fontFamily:'var(--mono)', fontSize:9.5, color:'var(--ink-3)', letterSpacing:'.06em', textTransform:'uppercase' }}>1D · INTRADAY</div>
        <AreaChart data={data} width={400} height={170} color="var(--buy)" fillOpacity={.22}/>
      </div>
      <div style={{ display:'grid', gridTemplateColumns:'1fr 1fr', rowGap:14, columnGap:24, paddingLeft:30, alignContent:'center' }}>
        {[
          { l:'OPEN', v:'1,386.55' }, { l:'HIGH', v:'1,392.72' },
          { l:'LOW',  v:'1,276.40' }, { l:'VOL',  v:'48.2M' },
          { l:'52W H',v:'1,348.55' }, { l:'52W L',v:'718.10' },
          { l:'MKT CAP',v:'3.16T' }, { l:'P/E',  v:'68.2' },
        ].map((x,i) => (
          <div key={i}>
            <div style={{ fontSize:9, color:'var(--ink-3)', letterSpacing:'.08em' }}>{x.l}</div>
            <div className="num" style={{ fontSize:14, color:'var(--ink-0)', marginTop:2 }}>{x.v}</div>
          </div>
        ))}
      </div>
    </div>
  );
}

function HeatBox({ sym, ch, big, intensity = 1 }){
  const isUp = ch >= 0;
  const a = Math.min(.55, Math.abs(ch) / 4) * intensity;
  const bg = isUp
    ? `linear-gradient(180deg, rgba(111,191,115,${a + .05}), rgba(111,191,115,${a*.7}))`
    : `linear-gradient(180deg, rgba(226,93,93,${a + .05}), rgba(226,93,93,${a*.7}))`;
  return (
    <div style={{ background:bg,
                  border:'.5px solid rgba(0,0,0,.4)',
                  borderRadius:4,
                  padding:'8px 10px',
                  boxShadow:'inset 0 .5px 0 rgba(255,255,255,.08), inset 0 -.5px 0 rgba(0,0,0,.4)',
                  display:'flex', flexDirection:'column', justifyContent:'space-between',
                  height:'100%', minHeight: big ? 80 : 50 }}>
      <span style={{ fontFamily:'var(--mono)', fontWeight:700, fontSize: big ? 14 : 11, letterSpacing:'.04em', color:'var(--ink-0)' }}>{sym}</span>
      <span className={`num ${isUp ? 't-up' : 't-dn'}`} style={{ fontSize: big ? 12 : 10, fontWeight:600 }}>{fmtCh(ch)}</span>
    </div>
  );
}

function ScreenTerminal(){
  const [side, setSide] = useState('buy');
  const orderBook = useMemo(() => {
    const r = mulberry32(13);
    const rows = [];
    for (let i = 0; i < 9; i++){
      rows.push({
        px: +(1285.02 - i*0.05).toFixed(2),
        size: 40 + Math.floor(r()*200),
        total: 1300 + Math.floor(r()*1100)
      });
    }
    return rows;
  }, []);

  const candles = useMemo(() => makeCandles(110, 41, 1200, 7), []);
  const containerRef = useRef(null);
  const [w, setW] = useState(700);
  useEffect(() => {
    if (!containerRef.current) return;
    const ro = new ResizeObserver(es => setW(es[0].contentRect.width));
    ro.observe(containerRef.current);
    return () => ro.disconnect();
  }, []);

  const heatTech = [
    { sym:'NVDA', ch:+2.26, big:true },
    { sym:'AAPL', ch:-0.83, big:true },
    { sym:'MSFT', ch:+0.72, big:true },
    { sym:'AVGO', ch:+1.94 },
    { sym:'AMD',  ch:+2.62 },
    { sym:'CRM',  ch:+0.41 },
    { sym:'ORCL', ch:+1.10 },
    { sym:'ADBE', ch:-0.21 },
  ];
  const heatComms = [
    { sym:'META', ch:+1.85, big:true },
    { sym:'GOOGL',ch:-0.41, big:true },
    { sym:'NFLX', ch:+0.55 },
    { sym:'DIS',  ch:-0.82 },
  ];

  return (
    <div style={{ display:'flex', flexDirection:'column', height:'100%' }}>
      <TickerStrip/>
      <div className="scroll" style={{ flex:1, minHeight:0 }}>
        <HeroPrice/>

        {/* Bento grid */}
        <div style={{ display:'grid',
                      gridTemplateColumns:'minmax(200px, 240px) 1fr minmax(220px, 280px)',
                      gridTemplateRows:'minmax(280px, 1fr) minmax(220px, 1fr)',
                      gap:0,
                      borderBottom:'1px solid var(--line-3)' }}>

          {/* Watchlist — shared component */}
          <div className="split-v" style={{ display:'flex', flexDirection:'column', minHeight:0 }}>
            <WatchlistPanel
              num="01"
              title="Watchlist"
              group="Top 12"
              count={8}
              activeSym={'NVDA'}
              setActiveSym={() => {}}/>
          </div>

          {/* Chart */}
          <div className="split-v" style={{ display:'flex', flexDirection:'column', minHeight:0 }}>
            <div className="ph">
              <div className="ph-l">
                <span className="fr-label"><span className="num">02</span> NVDA · 1D</span>
                <span className="sub">CANDLES · 5M</span>
              </div>
              <div className="seg" style={{ padding:1 }}>
                {['1m','5m','15m','1H','1D','1W'].map((t,i) => (
                  <button key={t} className={`sg ${i===1?'on-acc':''}`} style={{ height:18, padding:'0 7px', fontSize:9.5 }}>{t}</button>
                ))}
              </div>
            </div>
            <div style={{ padding:'4px 12px', fontFamily:'var(--mono)', fontSize:10, color:'var(--ink-3)' }}>
              <span style={{ color:'var(--ink-2)' }}>O</span> 1386.55  <span style={{ color:'var(--ink-2)' }}>H</span> 1392.72  <span style={{ color:'var(--ink-2)' }}>L</span> 1276.40  <span style={{ color:'var(--ink-2)' }}>C</span> <span style={{ color:'var(--ink-0)' }}>1284.32</span>  <span style={{ color:'var(--accent)' }}>MA20</span> 1212.18  <span style={{ color:'var(--sell)' }}>MA50</span> 1184.62
            </div>
            <div ref={containerRef} style={{ flex:1, minHeight:0, position:'relative' }}>
              <div style={{ position:'absolute', inset:0 }}>
                <CandleChart candles={candles} width={w} height={260} lineOverlay showVolume showGrid/>
              </div>
            </div>
          </div>

          {/* Order book */}
          <div style={{ display:'flex', flexDirection:'column', minHeight:0 }}>
            <div className="ph">
              <div className="ph-l">
                <span className="fr-label"><span className="num">03</span> Order Book</span>
                <span className="sub">LVL 2 · 14 DEEP</span>
              </div>
            </div>
            <div style={{ display:'grid', gridTemplateColumns:'1fr 60px 60px', padding:'7px 12px',
                          fontFamily:'var(--mono)', fontSize:10, color:'var(--ink-3)', letterSpacing:'.08em', textTransform:'uppercase',
                          background:'rgba(0,0,0,.18)', borderBottom:'1px solid var(--line-3)' }}>
              <span>PRICE</span><span style={{ textAlign:'right' }}>SIZE</span><span style={{ textAlign:'right' }}>TOTAL</span>
            </div>
            <div style={{ flex:1, minHeight:0, overflow:'auto' }}>
              {orderBook.map((r, i) => {
                const isAsk = i < 5;
                const col = isAsk ? 'var(--sell)' : 'var(--buy)';
                const pct = (r.size / 240) * 100;
                return (
                  <div key={i} style={{ position:'relative', display:'grid', gridTemplateColumns:'1fr 60px 60px',
                                        padding:'4px 12px', alignItems:'center',
                                        fontFamily:'var(--mono)', fontSize:11 }}>
                    <div style={{ position:'absolute', inset:0,
                      background: `linear-gradient(90deg, transparent ${100-pct}%, ${isAsk?'rgba(226,93,93,.12)':'rgba(111,191,115,.12)'} 100%)` }}/>
                    <span style={{ position:'relative', color: col }}>{r.px.toFixed(2)}</span>
                    <span style={{ position:'relative', color:'var(--ink-1)', textAlign:'right' }}>{r.size}</span>
                    <span style={{ position:'relative', color:'var(--ink-2)', textAlign:'right' }}>{r.total}</span>
                  </div>
                );
              })}
            </div>
          </div>

          {/* News */}
          <div className="split-v split-hT" style={{ display:'flex', flexDirection:'column', minHeight:0 }}>
            <div className="ph">
              <div className="ph-l">
                <span className="fr-label"><span className="num">04</span> News</span>
                <span className="sub">LIVE</span>
              </div>
              <button className="btn-ghost" style={{ width:18, height:18, borderRadius:4, display:'inline-flex', alignItems:'center', justifyContent:'center' }}><Icon.Maximize size={11}/></button>
            </div>
            <div style={{ flex:1, minHeight:0, overflow:'auto' }}>
              {[
                { t:'09:42', src:'BBG', body:'Nvidia surges past $1.28T as Blackwell shipments accelerate; analysts raise PT to $1,400', sym:'NVDA' },
                { t:'09:38', src:'RTRS', body:'Tesla deliveries miss Q3 estimates; FSD revenue offsets vehicle weakness', sym:'TSLA' },
                { t:'09:31', src:'WSJ', body:'Apple iPhone 17 supercycle data points trending positive into holiday', sym:'AAPL' },
                { t:'09:24', src:'CNBC',body:'Meta announces Reality Labs profitability target moved up to 2027', sym:'META' },
                { t:'09:17', src:'BBG', body:'JPMorgan raises S&P 500 year-end target to 6,200 on AI capex strength', sym:'JPM' },
              ].map((n, i) => (
                <div key={i} style={{ padding:'9px 12px', borderBottom:'1px solid rgba(255,234,210,.04)',
                                      display:'grid', gridTemplateColumns:'40px 38px 1fr', gap:8 }}>
                  <span className="num" style={{ fontSize:10.5, color:'var(--ink-3)' }}>{n.t}</span>
                  <span style={{ fontFamily:'var(--mono)', fontSize:9.5, color:'var(--accent)', letterSpacing:'.04em' }}>{n.src}</span>
                  <div>
                    <div style={{ fontSize:12, color:'var(--ink-1)', lineHeight:1.45, textWrap:'pretty' }}>{n.body}</div>
                    <div style={{ marginTop:3, fontFamily:'var(--mono)', fontSize:9.5, color:'var(--ink-3)' }}>{n.sym}</div>
                  </div>
                </div>
              ))}
            </div>
          </div>

          {/* Sector heatmap */}
          <div className="split-v split-hT" style={{ display:'flex', flexDirection:'column', minHeight:0 }}>
            <div className="ph">
              <div className="ph-l">
                <span className="fr-label"><span className="num">05</span> Sector Heatmap</span>
                <span className="sub">S&P 500 · TODAY</span>
              </div>
            </div>
            <div style={{ padding:10, display:'flex', flexDirection:'column', gap:6, flex:1, minHeight:0 }}>
              <div style={{ display:'flex', justifyContent:'space-between', fontFamily:'var(--mono)', fontSize:9.5, color:'var(--ink-3)', letterSpacing:'.06em', textTransform:'uppercase' }}>
                <span>TECH · 32%</span>
                <span style={{ color:'var(--buy)' }}>+1.84%</span>
              </div>
              <div style={{ display:'grid', gridTemplateColumns:'1.4fr 1fr 1fr 1fr', gridAutoRows:'1fr', gap:3, height:90 }}>
                <div style={{ gridRow:'span 2' }}><HeatBox {...heatTech[0]}/></div>
                <div style={{ gridRow:'span 2' }}><HeatBox {...heatTech[1]}/></div>
                <div><HeatBox {...heatTech[2]}/></div>
                <div><HeatBox {...heatTech[3]}/></div>
                <div><HeatBox {...heatTech[4]}/></div>
                <div><HeatBox {...heatTech[5]}/></div>
              </div>
              <div style={{ display:'flex', justifyContent:'space-between', fontFamily:'var(--mono)', fontSize:9.5, color:'var(--ink-3)', letterSpacing:'.06em', textTransform:'uppercase', marginTop:4 }}>
                <span>COMMS · 16%</span>
                <span style={{ color:'var(--buy)' }}>+0.42%</span>
              </div>
              <div style={{ display:'grid', gridTemplateColumns:'1.4fr 1fr 1fr 1fr', gridAutoRows:'1fr', gap:3, height:50 }}>
                <div><HeatBox {...heatComms[0]}/></div>
                <div><HeatBox {...heatComms[1]}/></div>
                <div><HeatBox {...heatComms[2]}/></div>
                <div><HeatBox {...heatComms[3]}/></div>
              </div>
            </div>
          </div>

          {/* Time & Sales + Order ticket — split */}
          <div className="split-hT" style={{ display:'grid', gridTemplateRows:'auto 1fr auto', minHeight:0 }}>
            <div className="ph">
              <div className="ph-l">
                <span className="fr-label"><span className="num">06</span> Order Ticket</span>
                <span className="sub">NVDA</span>
              </div>
            </div>
            <div style={{ padding:10, display:'flex', flexDirection:'column', gap:8, minHeight:0 }}>
              <div className="seg">
                <button className={`sg ${side==='buy'?'on-buy':''}`} onClick={() => setSide('buy')}>BUY</button>
                <button className={`sg ${side==='sell'?'on-sell':''}`} onClick={() => setSide('sell')}>SELL</button>
              </div>
              <div>
                <div style={{ fontSize:9.5, color:'var(--ink-3)', letterSpacing:'.08em', marginBottom:4 }}>SYMBOL</div>
                <div className="well" style={{ height:24, display:'flex', alignItems:'center', padding:'0 8px', justifyContent:'space-between' }}>
                  <span className="num" style={{ fontSize:11, fontWeight:600 }}>NVDA</span>
                  <span style={{ fontSize:9.5, color:'var(--ink-3)', fontFamily:'var(--mono)' }}>NASDAQ · USD</span>
                </div>
              </div>
              <div style={{ display:'grid', gridTemplateColumns:'1fr 70px', gap:6 }}>
                <div>
                  <div style={{ fontSize:9.5, color:'var(--ink-3)', letterSpacing:'.08em', marginBottom:4 }}>TYPE</div>
                  <div className="well" style={{ height:24, display:'flex', alignItems:'center', padding:'0 8px', justifyContent:'space-between' }}>
                    <span className="num" style={{ fontSize:11 }}>LMT</span>
                    <Icon.Caret size={9}/>
                  </div>
                </div>
                <div>
                  <div style={{ fontSize:9.5, color:'var(--ink-3)', letterSpacing:'.08em', marginBottom:4 }}>QTY</div>
                  <div className="stp" style={{ padding:1 }}>
                    <button className="stp-b" style={{ width:18, height:18 }}>−</button>
                    <input className="stp-i" defaultValue={100} style={{ height:18, fontSize:11 }}/>
                    <button className="stp-b" style={{ width:18, height:18 }}>+</button>
                  </div>
                </div>
              </div>
              <div>
                <div style={{ fontSize:9.5, color:'var(--ink-3)', letterSpacing:'.08em', marginBottom:4 }}>LIMIT PRICE</div>
                <div className="well" style={{ height:24, display:'flex', alignItems:'center', padding:'0 8px' }}>
                  <span className="num" style={{ fontSize:11 }}>1,284.32</span>
                </div>
              </div>
              <button className={side==='buy' ? 'btn btn-buy' : 'btn btn-sell'} style={{ height:32, fontSize:11, letterSpacing:'.06em' }}>
                {side==='buy' ? 'BUY 100' : 'SELL 100'}
              </button>
            </div>
          </div>
        </div>

        {/* Positions strip */}
        <div style={{ padding:18, display:'flex', flexDirection:'column', gap:14 }}>
          <div className="ph" style={{ padding:0, border:0 }}>
            <div className="ph-l">
              <span className="fr-label"><span className="num">07</span> Positions</span>
              <span className="sub">7 OPEN · LONG-SHORT</span>
            </div>
            <button className="btn-ghost" style={{ width:18, height:18, borderRadius:4, display:'inline-flex', alignItems:'center', justifyContent:'center' }}><Icon.Maximize size={11}/></button>
          </div>

          <div style={{ display:'grid', gridTemplateColumns:'1fr 1fr 1fr 1fr', gap:14 }}>
            {[
              { l:'NET P&L (TODAY)', v:'+290.8K', col:'var(--buy)' },
              { l:'MARKET VALUE', v:'$3.25M' },
              { l:'CASH', v:'$2.42M' },
              { l:'BUYING POWER', v:'$5.83M' },
            ].map((x,i) => (
              <div key={i}>
                <div style={{ fontSize:9.5, color:'var(--ink-3)', letterSpacing:'.08em' }}>{x.l}</div>
                <div className="num" style={{ fontSize:30, color: x.col || 'var(--ink-0)', letterSpacing:'-.02em', marginTop:4 }}>{x.v}</div>
              </div>
            ))}
          </div>

          <div className="card-flat" style={{ padding:0, overflow:'hidden' }}>
            <div style={{ display:'grid', gridTemplateColumns:'80px 80px 80px 80px 1fr 100px', padding:'10px 14px',
                          fontFamily:'var(--mono)', fontSize:10, color:'var(--ink-3)', letterSpacing:'.08em', textTransform:'uppercase',
                          background:'rgba(0,0,0,.18)', borderBottom:'1px solid var(--line-3)' }}>
              <span>SYM</span><span>SIDE</span><span>QTY</span><span>AVG</span><span>MARKET VALUE</span><span style={{ textAlign:'right' }}>P&L</span>
            </div>
            {SYMS.filter(s => s.qty).map((p,i) => {
              const mv = Math.abs(p.qty) * p.px;
              const pl = p.qty * (p.px - p.avg);
              return (
                <div key={p.sym} style={{ display:'grid', gridTemplateColumns:'80px 80px 80px 80px 1fr 100px',
                                          padding:'10px 14px', alignItems:'center',
                                          fontFamily:'var(--mono)', fontSize:12,
                                          borderBottom:'1px solid rgba(255,234,210,.04)' }} className="cell-hover">
                  <span style={{ color:'var(--ink-0)', fontWeight:600 }}>{p.sym}</span>
                  <span style={{ color: p.qty < 0 ? 'var(--sell-ink)' : 'var(--buy-ink)', fontSize:10, letterSpacing:'.06em' }}>{p.qty < 0 ? 'SHORT' : 'LONG'}</span>
                  <span className="num">{Math.abs(p.qty).toLocaleString()}</span>
                  <span className="num" style={{ color:'var(--ink-2)' }}>{fmtNum(p.avg)}</span>
                  <span className="num" style={{ color:'var(--ink-1)' }}>${mv.toLocaleString('en-US',{maximumFractionDigits:0})}</span>
                  <span className={`num ${pl>=0?'t-up':'t-dn'}`} style={{ textAlign:'right' }}>
                    {pl>=0?'+':''}${Math.abs(pl).toLocaleString('en-US',{maximumFractionDigits:0})} · {fmtCh(p.ch)}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}

window.ScreenTerminal = ScreenTerminal;
