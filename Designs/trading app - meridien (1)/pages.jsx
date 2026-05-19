// Trade page (chart-focused with DOM left, Watchlist right, floating widgets)
// + Tiled dashboard page — A/B/C variants with refined infographic widgets

const { useState: tS } = React;

// =========================================================
// TRADE PAGE
// =========================================================
function TradePage({ activeRow, D, variant='A', showVol, showMA, showBB }) {
  return (
    <div style={{flex:1, minHeight:0, display:'grid', gridTemplateColumns:'260px 1fr 280px', overflow:'hidden'}}
         data-screen-label="03 Trade">
      {/* DOM TRADER LEFT */}
      <div style={{borderRight:'1px solid var(--rule)', display:'flex', flexDirection:'column', minHeight:0, overflow:'hidden'}}>
        <div className="panel-head">
          <div className="ttl">
            <span className="num">DOM</span>
            <span className="nm">{activeRow.sym} · Ladder</span>
          </div>
          <div className="actions"><button className="icon-btn">⋯</button></div>
        </div>
        <div style={{flex:1, minHeight:0, overflow:'auto'}}>
          <DomLadder sym={activeRow.sym} mid={activeRow.last}/>
        </div>
      </div>

      {/* CHART CENTER WITH FLOATING WIDGETS */}
      <div style={{position:'relative', minHeight:0, overflow:'hidden', display:'flex', flexDirection:'column'}}>
        <div className="chart-toolbar">
          <div className="grp">
            {['1m','5m','15m','30m','1H','4H','1D','1W'].map(tf => (
              <div key={tf} className={'tf '+(tf==='5m'?'on':'')}>{tf}</div>
            ))}
          </div>
          <div className="grp">
            <div className="tf">＋ Indicator</div>
            <div className="tf">⊞ Compare</div>
          </div>
        </div>
        <div className="chart-body" style={{position:'relative'}}>
          <div className="chart-tools">
            {['↖','│','╱','◯','▭','T','⌖','⊕','▾'].map((g,i)=>(
              <div key={i} className={'t '+(i===2?'on':'')}>{g}</div>
            ))}
          </div>
          <div className="chart-canvas">
            <Candles candles={D.candles} width={1100} height={520}
                     showVol={showVol} showMA={showMA} showBB={showBB}/>
          </div>

          {variant === 'A'
            ? <FloatingWidgetsA activeRow={activeRow} D={D}/>
            : <FloatingWidgetsB activeRow={activeRow} D={D}/>}
        </div>
      </div>

      {/* WATCHLIST RIGHT */}
      <div style={{borderLeft:'1px solid var(--rule)', display:'flex', flexDirection:'column', minHeight:0, overflow:'hidden'}}>
        <div className="panel-head">
          <div className="ttl">
            <span className="num">WL</span>
            <span className="nm">Watchlist</span>
          </div>
          <div className="actions"><button className="icon-btn">＋</button></div>
        </div>
        <div style={{flex:1, minHeight:0, overflow:'auto'}}>
          <Watchlist data={D.watchlist} active={activeRow.sym}/>
          <div style={{padding:'14px', borderTop:'1px solid var(--hair)'}}>
            <div className="label" style={{marginBottom:8}}>Quick orders</div>
            <div style={{display:'grid', gridTemplateColumns:'1fr 1fr', gap:8}}>
              <button style={{padding:'10px', background:'var(--up)', color:'var(--bg)', border:0, fontFamily:'JetBrains Mono', fontSize:11, letterSpacing:'.1em', cursor:'pointer'}}>BUY MKT</button>
              <button style={{padding:'10px', background:'var(--down)', color:'var(--bg)', border:0, fontFamily:'JetBrains Mono', fontSize:11, letterSpacing:'.1em', cursor:'pointer'}}>SELL MKT</button>
              <button style={{padding:'10px', background:'var(--paper)', color:'var(--ink)', border:'1px solid var(--rule)', fontFamily:'JetBrains Mono', fontSize:11, letterSpacing:'.1em', cursor:'pointer'}}>BUY LMT</button>
              <button style={{padding:'10px', background:'var(--paper)', color:'var(--ink)', border:'1px solid var(--rule)', fontFamily:'JetBrains Mono', fontSize:11, letterSpacing:'.1em', cursor:'pointer'}}>SELL LMT</button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function DomLadder({ sym, mid=1284.32 }){
  const tick=0.05;
  const rows=[];
  for(let i=12;i>=-12;i--){
    const px = +(mid + i*tick).toFixed(2);
    const isMid = i===0;
    const sz = Math.abs(i)===0 ? 0 : Math.round(40 + Math.sin(i*1.3+5)*40 + Math.cos(i*0.7)*30 + 80);
    rows.push({i, px, sz: Math.abs(sz), side: i>0?'ask':'bid', isMid});
  }
  return (
    <div style={{fontFamily:'JetBrains Mono', fontSize:11}}>
      <div style={{display:'grid', gridTemplateColumns:'1fr 1fr 1fr 1fr', padding:'6px 8px', borderBottom:'1px solid var(--hair)', fontSize:9, color:'var(--muted)', letterSpacing:'.12em', textTransform:'uppercase'}}>
        <div>BUY</div><div style={{textAlign:'right'}}>BID</div><div>PRICE</div><div>ASK</div>
      </div>
      {rows.map(r => (
        <div key={r.i} style={{
          display:'grid', gridTemplateColumns:'1fr 1fr 1fr 1fr',
          padding:'3px 8px',
          borderBottom: r.isMid ? '1px solid var(--rule)' : '1px solid var(--hair-2)',
          background: r.isMid ? 'var(--paper)' : 'transparent',
          alignItems:'center', position:'relative', minHeight:22,
        }}>
          {r.side==='bid' ? (
            <>
              <div style={{color:'var(--muted)'}}>{r.sz}</div>
              <div className="up" style={{textAlign:'right'}}>{r.sz}</div>
            </>
          ) : <><div></div><div></div></>}
          <div style={{fontWeight: r.isMid?700:500, fontSize: r.isMid?13:11}}>{r.px.toFixed(2)}</div>
          {r.side==='ask' ? (
            <div className="down">{r.sz}</div>
          ) : <div></div>}
        </div>
      ))}
    </div>
  );
}

// ----- Variant A: pill-action floating set
function FloatingWidgetsA({ activeRow, D }){
  return (
    <>
      <div style={{position:'absolute', top:18, left:54, display:'flex', gap:10}}>
        <div style={{
          background:'var(--accent)', color:'#0a0a08',
          borderRadius:18, padding:'14px 18px', minWidth:180,
          boxShadow:'0 8px 28px rgba(20,20,15,.18)',
        }}>
          <div style={{fontFamily:'JetBrains Mono', fontSize:9, letterSpacing:'.12em', opacity:.7}}>NVDA · INTRADAY</div>
          <div style={{fontFamily:'Inter Tight', fontSize:42, fontWeight:500, letterSpacing:'-0.04em', lineHeight:.95, marginTop:4}}>+2.26%</div>
          <div style={{color:'#0a0a08', marginTop:6}}>
            <WaveBars values={D.candles.slice(-30).map(c=>c.c-1100)} cols={20} rows={5} dotSize={5}/>
          </div>
        </div>
        <div style={{
          background:'#0a0a08', color:'#f1ede1',
          borderRadius:18, padding:'14px 18px', minWidth:130,
          boxShadow:'0 8px 28px rgba(20,20,15,.18)',
        }}>
          <div style={{fontFamily:'JetBrains Mono', fontSize:9, letterSpacing:'.12em', opacity:.55}}>SESSION</div>
          <div style={{display:'flex', alignItems:'baseline', gap:6, marginTop:4}}>
            <div style={{fontFamily:'JetBrains Mono', fontSize:9, opacity:.55, lineHeight:1.1}}>
              <div style={{opacity:.4}}>AM</div><div style={{color:'var(--accent)'}}>PM</div>
            </div>
            <div style={{fontFamily:'Inter Tight', fontSize:30, fontWeight:500, letterSpacing:'-0.03em'}}>02:14</div>
          </div>
          <div style={{fontFamily:'JetBrains Mono', fontSize:10, opacity:.55, marginTop:4}}>1h 46m to close</div>
        </div>
      </div>

      <div style={{position:'absolute', top:18, right:18,
                   background:'var(--bg)', border:'1px solid var(--rule)', borderRadius:16,
                   padding:'12px 14px', boxShadow:'0 8px 28px rgba(20,20,15,.10)', width:200}}>
        <div style={{display:'flex', justifyContent:'space-between', alignItems:'center', marginBottom:6}}>
          <div className="label">Sentiment · 24h</div>
          <div className="mono" style={{fontSize:10, color:'var(--muted)'}}>n=4,128</div>
        </div>
        <div style={{height:170}}>
          <FlowerSentiment data={[
            {label:'Buy', v:14, color:'var(--up)'},
            {label:'Strong', v:18, color:'var(--up)'},
            {label:'Neutral', v:6, color:'var(--muted)'},
            {label:'Hold', v:9, color:'var(--ink)'},
            {label:'Watch', v:5, color:'var(--ink)'},
            {label:'Cut', v:3, color:'var(--down)'},
            {label:'Sell', v:7, color:'var(--down)'},
            {label:'Avoid', v:4, color:'var(--down)'},
          ]}/>
        </div>
      </div>

      <div style={{position:'absolute', bottom:88, left:'50%', transform:'translateX(-50%)',
                   background:'rgba(247,243,234,.94)', backdropFilter:'blur(4px)',
                   border:'1px solid var(--rule)', borderRadius:99,
                   padding:'8px 10px', display:'flex', gap:6,
                   boxShadow:'0 8px 28px rgba(20,20,15,.12)'}}>
        {[
          {label:'Buy 100', fill:true, color:'var(--up)'},
          {label:'Sell 100', fill:true, color:'var(--down)'},
          {label:'Flatten', color:'var(--ink)'},
          {label:'Reverse', color:'var(--ink)'},
          {label:'Bracket', color:'var(--accent)'},
        ].map((it,i)=>(
          <div key={i} style={{
            padding:'8px 16px', borderRadius:99, fontFamily:'JetBrains Mono',
            fontSize:11, letterSpacing:'.06em', textTransform:'uppercase',
            background: it.fill ? it.color : 'transparent',
            border: it.fill ? 'none' : `1.5px solid ${it.color}`,
            color: it.fill ? '#fff' : it.color, cursor:'pointer',
          }}>{it.label}</div>
        ))}
      </div>

      <div style={{position:'absolute', bottom:18, right:18,
                   background:'#0a0a08', color:'#f1ede1',
                   borderRadius:18, padding:'14px 18px',
                   boxShadow:'0 8px 28px rgba(20,20,15,.22)', minWidth:200}}>
        <div style={{fontFamily:'JetBrains Mono', fontSize:9, letterSpacing:'.12em', opacity:.55}}>OPEN P&L · {activeRow.sym}</div>
        <div style={{fontFamily:'Inter Tight', fontSize:36, fontWeight:500, letterSpacing:'-0.04em', color:'var(--accent)', lineHeight:1, marginTop:4}}>+$169,824</div>
        <div style={{display:'flex', justifyContent:'space-between', marginTop:8, fontFamily:'JetBrains Mono', fontSize:10}}>
          <span style={{opacity:.55}}>1,200 @ 1142.80</span>
          <span style={{color:'var(--accent)'}}>+12.39%</span>
        </div>
      </div>

      <div style={{position:'absolute', left:54, bottom:18, display:'flex', gap:8}}>
        {[{l:'VWAP',v:'1,261.42'},{l:'σ 5m',v:'0.84%'},{l:'IV',v:'42.6'},{l:'RSI',v:'62.4'}].map((s,i)=>(
          <div key={i} style={{background:'var(--bg)', border:'1px solid var(--rule)', borderRadius:14, padding:'8px 12px', boxShadow:'0 4px 14px rgba(20,20,15,.06)'}}>
            <div className="label">{s.l}</div>
            <div className="mono" style={{fontSize:14, marginTop:2}}>{s.v}</div>
          </div>
        ))}
      </div>
    </>
  );
}

// ----- Variant B: burst + concentric + mosaic floating set
function FloatingWidgetsB({ activeRow, D }){
  const burstData = D.candles.slice(-40).map(c => c.c - 1140);
  return (
    <>
      {/* BURST top-left */}
      <div style={{position:'absolute', top:14, left:54,
                   background:'var(--paper)', border:'1px solid var(--rule)', borderRadius:18,
                   padding:'12px', boxShadow:'0 8px 28px rgba(20,20,15,.12)', width:240}}>
        <div style={{display:'flex', justifyContent:'space-between'}}>
          <div className="label">5-min burst · last 40</div>
          <div className="mono" style={{fontSize:10, color:'var(--muted)'}}>NVDA</div>
        </div>
        <div style={{height:200, marginTop:4}}>
          <BurstChart data={burstData} size={220}
                      label="+2.26%" sub="INTRADAY"
                      showYears={false} fillBars="var(--accent)"/>
        </div>
      </div>

      {/* CONCENTRIC top-right */}
      <div style={{position:'absolute', top:14, right:18,
                   background:'#0a0a08', color:'#f1ede1', borderRadius:18,
                   padding:'14px', boxShadow:'0 8px 28px rgba(20,20,15,.18)', width:240}}>
        <div style={{display:'flex', justifyContent:'space-between'}}>
          <div className="label" style={{color:'rgba(241,237,225,.6)'}}>Order flow · book depth</div>
          <div className="mono" style={{fontSize:10, color:'rgba(241,237,225,.6)'}}>L20</div>
        </div>
        <div style={{height:200, marginTop:4, '--paper':'#161410', '--rule':'rgba(241,237,225,.18)', '--hair':'rgba(241,237,225,.10)'}}>
          <ConcentricRings rings={[
            {label:'BIDS', values:[58,42], colors:['#5fd28a','rgba(95,210,138,.25)']},
            {label:'ASKS', values:[51,49], colors:['#ff7a52','rgba(255,122,82,.25)']},
            {label:'SWEEP',values:[34,66], colors:['var(--accent)','rgba(20,20,15,.4)']},
          ]} size={210}/>
        </div>
        <div style={{display:'flex', justifyContent:'space-between', fontFamily:'JetBrains Mono', fontSize:10, color:'rgba(241,237,225,.6)', marginTop:4}}>
          <span style={{color:'#5fd28a'}}>BUY 58%</span>
          <span style={{color:'#ff7a52'}}>SELL 42%</span>
        </div>
      </div>

      {/* IV TERM bottom-left strip */}
      <div style={{position:'absolute', left:54, bottom:14,
                   background:'var(--paper)', border:'1px solid var(--rule)', borderRadius:18,
                   padding:'12px 14px', width:340, boxShadow:'0 8px 28px rgba(20,20,15,.12)'}}>
        <div style={{display:'flex', justifyContent:'space-between'}}>
          <div className="label">IV term structure</div>
          <div className="mono" style={{fontSize:10, color:'var(--muted)'}}>30D · 45.2 IVR</div>
        </div>
        <MarketBands tenors={['1W','2W','1M','2M','3M','6M','1Y']}
                     values={[58.2, 52.1, 46.4, 44.8, 43.2, 41.0, 39.6]}
                     size={316} height={120}/>
      </div>

      {/* P&L bottom-right */}
      <div style={{position:'absolute', bottom:14, right:18,
                   background:'#0a0a08', color:'#f1ede1', borderRadius:18,
                   padding:'14px 18px', minWidth:230, boxShadow:'0 8px 28px rgba(20,20,15,.22)'}}>
        <div style={{fontFamily:'JetBrains Mono', fontSize:9, letterSpacing:'.12em', opacity:.55}}>OPEN P&L · {activeRow.sym}</div>
        <div style={{fontFamily:'Inter Tight', fontSize:36, fontWeight:500, letterSpacing:'-0.04em', color:'var(--accent)', lineHeight:1, marginTop:4}}>+$169,824</div>
        <div style={{display:'flex', justifyContent:'space-between', marginTop:8, fontFamily:'JetBrains Mono', fontSize:10}}>
          <span style={{opacity:.55}}>1,200 @ 1142.80</span>
          <span style={{color:'var(--accent)'}}>+12.39%</span>
        </div>
        <div style={{marginTop:10, color:'var(--accent)'}}>
          <WaveBars values={D.candles.slice(-30).map(c=>c.c-1100)} cols={28} rows={4} dotSize={4} off="rgba(241,237,225,.10)"/>
        </div>
      </div>

      {/* Mid-right small mosaic */}
      <div style={{position:'absolute', top:'52%', transform:'translateY(-50%)', right:18,
                   background:'var(--paper)', border:'1px solid var(--rule)', borderRadius:18,
                   padding:'12px', width:170, boxShadow:'0 8px 28px rgba(20,20,15,.12)'}}>
        <div className="label" style={{marginBottom:6}}>Liquidity grid</div>
        <div style={{height:140}}>
          <MosaicGrid rows={12} cols={12} density={0.42} accentEvery={11}/>
        </div>
        <div style={{display:'flex', justifyContent:'space-between', fontFamily:'JetBrains Mono', fontSize:9, color:'var(--muted)', marginTop:4}}>
          <span>L1 — L20</span><span>62%</span>
        </div>
      </div>
    </>
  );
}

// =========================================================
// TILES PAGE — three variants A/B/C
// =========================================================
function TilesPage({ D, variant='A' }){
  if (variant === 'B') return <TilesVariantB D={D}/>;
  if (variant === 'C') return <TilesVariantC D={D}/>;
  return <TilesVariantA D={D}/>;
}

// ----- Variant A: bold editorial — themed
function TilesVariantA({ D }){
  return (
    <div style={{flex:1, minHeight:0, overflow:'auto', padding: 22, background:'var(--tile-dark)'}}
         data-screen-label="04 Tiles · A">
      <div style={{
        display:'grid', gridTemplateColumns:'repeat(12, 1fr)',
        gridAutoRows:'minmax(110px, auto)', gap:14, color:'var(--tile-cream)',
      }}>
        {/* Hero P&L */}
        <div style={{gridColumn:'span 6', gridRow:'span 2',
          background:'var(--tile-cream)', color:'var(--tile-fg-on-warm)', borderRadius:24, padding:'24px 28px',
          display:'flex', flexDirection:'column', justifyContent:'space-between'}}>
          <div style={{display:'flex', justifyContent:'space-between', alignItems:'center'}}>
            <div style={{fontFamily:'Inter Tight', fontSize:14, fontWeight:600}}>Today's P&L</div>
            <div style={{display:'flex', gap:6}}>
              <span className="mono" style={{fontSize:10, padding:'4px 10px', borderRadius:99, background:'var(--tile-fg-on-warm)', color:'var(--tile-cream)'}}>1D</span>
              <span className="mono" style={{fontSize:10, padding:'4px 10px', borderRadius:99, border:'1px solid var(--tile-fg-on-warm)'}}>1W</span>
              <span className="mono" style={{fontSize:10, padding:'4px 10px', borderRadius:99, border:'1px solid var(--tile-fg-on-warm)'}}>1M</span>
            </div>
          </div>
          <div>
            <div style={{fontFamily:'Inter Tight', fontSize:120, fontWeight:500, letterSpacing:'-0.05em', lineHeight:.85}}>+$290,794</div>
            <div style={{display:'flex', gap:18, marginTop:14, fontFamily:'JetBrains Mono', fontSize:11}}>
              <span>REALIZED <span style={{color:'var(--tile-green)'}}>+$48,210</span></span>
              <span>UNREALIZED <span style={{color:'var(--tile-green)'}}>+$242,584</span></span>
              <span>WIN-RATE <strong>72%</strong></span>
            </div>
          </div>
        </div>

        {/* Top mover orange */}
        <div style={{gridColumn:'span 3', gridRow:'span 2', background:'var(--tile-warm)', color:'var(--tile-fg-on-warm)', borderRadius:24, padding:'20px 22px', display:'flex', flexDirection:'column', justifyContent:'space-between'}}>
          <div>
            <div style={{fontFamily:'Inter Tight', fontSize:12, fontWeight:600, opacity:.75}}>Top mover</div>
            <div style={{fontFamily:'Inter Tight', fontSize:36, fontWeight:600, letterSpacing:'-0.03em', lineHeight:1, marginTop:6}}>NVDA</div>
          </div>
          <div>
            <div style={{fontFamily:'Inter Tight', fontSize:64, fontWeight:500, letterSpacing:'-0.04em', lineHeight:.9}}>+2.26%</div>
            <div style={{fontFamily:'JetBrains Mono', fontSize:11, marginTop:4}}>$1,284.32 · +28.40</div>
          </div>
        </div>

        {/* AM/PM yellow */}
        <div style={{gridColumn:'span 3', background:'var(--tile-yellow)', color:'var(--tile-fg-on-warm)', borderRadius:20, padding:'14px 18px', display:'flex', alignItems:'center', gap:14}}>
          <div style={{fontFamily:'JetBrains Mono', fontSize:10, lineHeight:1.1, fontWeight:600}}>
            <div style={{opacity:.4}}>AM</div><div>PM</div>
          </div>
          <div>
            <div style={{fontFamily:'Inter Tight', fontSize:34, fontWeight:500, letterSpacing:'-0.04em', lineHeight:.9}}>02:14</div>
            <div style={{fontFamily:'JetBrains Mono', fontSize:10, opacity:.7, marginTop:2}}>NYSE · OPEN</div>
          </div>
        </div>

        {/* Volume tile w/ wave */}
        <div style={{gridColumn:'span 3', background:'var(--tile-bg-2)', color:'var(--tile-cream)', borderRadius:20, padding:'16px 18px', border:'1px solid var(--tile-stroke)'}}>
          <div className="label" style={{color:'color-mix(in srgb, var(--tile-cream) 55%, transparent)'}}>Volume · Today</div>
          <div style={{fontFamily:'Inter Tight', fontSize:38, fontWeight:500, letterSpacing:'-0.03em', marginTop:4}}>14.2B</div>
          <div style={{color:'var(--tile-warm)', marginTop:4}}>
            <WaveBars values={[3,5,4,8,6,9,7,10,8,12,9,14,10,8,11,9,7,12,10,13]} cols={20} rows={4} dotSize={4} off="color-mix(in srgb, var(--tile-cream) 8%, transparent)"/>
          </div>
        </div>

        {/* Index strip */}
        {[
          {l:'S&P 500', s:'5,824.10', v:'+0.38%', bg:'var(--tile-green)', fg:'var(--tile-fg-on-warm)'},
          {l:'NASDAQ',  s:'18,642.20',v:'+0.85%', bg:'var(--tile-cream)', fg:'var(--tile-fg-on-warm)'},
          {l:'VIX',     s:'14.82',    v:'−3.21%', bg:'var(--tile-red)',   fg:'var(--tile-cream)'},
          {l:'10Y · TNX',s:'4.218%',  v:'+4.1bp', bg:'var(--tile-bg-2)',  fg:'var(--tile-cream)', acc:true, b:true},
        ].map((it,i)=>(
          <div key={i} style={{gridColumn:'span 3', background:it.bg, color:it.fg, borderRadius:20, padding:'16px 18px', display:'flex', flexDirection:'column', justifyContent:'space-between', border: it.b? '1px solid var(--tile-stroke)':'none'}}>
            <div>
              <div style={{fontFamily:'Inter Tight', fontSize:12, fontWeight:600}}>{it.l}</div>
              <div style={{fontFamily:'Inter Tight', fontSize:11, opacity:.7}}>{it.s}</div>
            </div>
            <div style={{fontFamily:'Inter Tight', fontSize:48, fontWeight:500, letterSpacing:'-0.04em', color: it.acc?'var(--tile-warm)':it.fg}}>{it.v}</div>
          </div>
        ))}

        {/* Sentiment polar */}
        <div style={{gridColumn:'span 4', gridRow:'span 2', background:'var(--tile-bg-2)', color:'var(--tile-cream)', borderRadius:22, padding:'18px 20px', display:'flex', flexDirection:'column', border:'1px solid var(--tile-stroke)',
                     '--paper':'var(--tile-bg-2)', '--rule':'color-mix(in srgb, var(--tile-cream) 18%, transparent)', '--hair':'color-mix(in srgb, var(--tile-cream) 10%, transparent)', '--ink':'var(--tile-cream)'}}>
          <div style={{display:'flex', justifyContent:'space-between'}}>
            <div style={{fontFamily:'Inter Tight', fontSize:14, fontWeight:600}}>Market Sentiment</div>
            <div className="mono" style={{fontSize:10, color:'color-mix(in srgb, var(--tile-cream) 55%, transparent)'}}>· 24h · n=18.4k</div>
          </div>
          <div style={{flex:1, display:'grid', placeItems:'center'}}>
            <div style={{width:240, height:240}}>
              <FlowerSentiment data={[
                {label:'Greed',  v:14, color:'var(--tile-warm)',  dark:true},
                {label:'Mom',    v:18, color:'var(--tile-warm)',  dark:true},
                {label:'Trend',  v:12, color:'var(--tile-green)', dark:true},
                {label:'Hold',   v:7,  color:'var(--tile-green)', dark:true},
                {label:'Wait',   v:5,  color:'var(--tile-green)', dark:true},
                {label:'Fear',   v:9,  color:'var(--tile-red)',   dark:true},
                {label:'VIX↑',   v:16, color:'var(--tile-warm)',  dark:true},
                {label:'Risk',   v:20, color:'var(--tile-warm)',  dark:true},
              ]}/>
            </div>
          </div>
          <div style={{display:'flex', justifyContent:'space-between', fontFamily:'JetBrains Mono', fontSize:10, color:'color-mix(in srgb, var(--tile-cream) 55%, transparent)'}}>
            <span>FEAR</span><span style={{color:'var(--tile-warm)'}}>GREEDY · 68</span><span>EUPHORIC</span>
          </div>
        </div>

        {/* Watchlist */}
        <div style={{gridColumn:'span 4', gridRow:'span 2', background:'var(--tile-cream)', color:'var(--tile-fg-on-warm)', borderRadius:22, padding:'18px 20px'}}>
          <div style={{display:'flex', justifyContent:'space-between', marginBottom:10}}>
            <div style={{fontFamily:'Inter Tight', fontSize:14, fontWeight:600}}>Watchlist</div>
            <div className="mono" style={{fontSize:10, opacity:.5}}>TOP 6</div>
          </div>
          {D.watchlist.slice(0,6).map(w => (
            <div key={w.sym} style={{display:'grid', gridTemplateColumns:'60px 1fr 80px 70px', padding:'8px 0', borderBottom:'1px solid color-mix(in srgb, var(--tile-fg-on-warm) 12%, transparent)', fontFamily:'JetBrains Mono', fontSize:12, alignItems:'center'}}>
              <span style={{fontWeight:700}}>{w.sym}</span>
              <span style={{opacity:.6, fontSize:11}}>{w.name}</span>
              <span style={{textAlign:'right'}}>{w.last.toLocaleString('en-US',{minimumFractionDigits:2})}</span>
              <span style={{textAlign:'right', color: w.pct>=0?'var(--tile-green)':'var(--tile-red)'}}>{w.pct>=0?'+':''}{w.pct.toFixed(2)}%</span>
            </div>
          ))}
        </div>

        {/* Risk gauge */}
        <div style={{gridColumn:'span 4', gridRow:'span 2', background:'var(--tile-warm)', color:'var(--tile-fg-on-warm)', borderRadius:22, padding:'20px 22px', display:'flex', flexDirection:'column', justifyContent:'space-between'}}>
          <div style={{display:'flex', justifyContent:'space-between'}}>
            <div style={{fontFamily:'Inter Tight', fontSize:14, fontWeight:600}}>Portfolio Risk</div>
            <div className="mono" style={{fontSize:10, opacity:.7}}>VAR · 95%</div>
          </div>
          <div>
            <div style={{fontFamily:'Inter Tight', fontSize:84, fontWeight:500, letterSpacing:'-0.05em', lineHeight:.85}}>$48.2K</div>
            <div style={{fontFamily:'JetBrains Mono', fontSize:11, marginTop:6}}>1-DAY · 1.7% OF NAV</div>
          </div>
          <div style={{display:'grid', gridTemplateColumns:'1fr 1fr 1fr', gap:8, marginTop:10}}>
            <div><div className="label">β</div><div style={{fontFamily:'Inter Tight', fontSize:22, fontWeight:500}}>1.18</div></div>
            <div><div className="label">σ</div><div style={{fontFamily:'Inter Tight', fontSize:22, fontWeight:500}}>14.2</div></div>
            <div><div className="label">SHARPE</div><div style={{fontFamily:'Inter Tight', fontSize:22, fontWeight:500}}>2.41</div></div>
          </div>
        </div>

        {/* Allocation */}
        <div style={{gridColumn:'span 8', background:'var(--tile-cream)', color:'var(--tile-fg-on-warm)', borderRadius:22, padding:'20px 24px'}}>
          <div style={{display:'flex', justifyContent:'space-between', alignItems:'center', marginBottom:14}}>
            <div style={{fontFamily:'Inter Tight', fontSize:14, fontWeight:600}}>Allocation Targets</div>
            <div style={{padding:'4px 12px', borderRadius:99, border:'1px solid var(--tile-fg-on-warm)', fontFamily:'JetBrains Mono', fontSize:10}}>REBALANCE</div>
          </div>
          <div style={{display:'grid', gridTemplateColumns:'1fr 1fr', gap:'4px 30px'}}>
            <BulletRow label="Tech" pct={42} color="var(--tile-fg-on-warm)"/>
            <BulletRow label="Communications" pct={18} color="var(--tile-fg-on-warm)"/>
            <BulletRow label="Discretionary" pct={14} color="var(--tile-warm)"/>
            <BulletRow label="Financials" pct={12} color="var(--tile-fg-on-warm)"/>
            <BulletRow label="Healthcare" pct={8}  color="var(--tile-fg-on-warm)"/>
            <BulletRow label="Cash" pct={6}  color="var(--tile-green)"/>
          </div>
        </div>

        {/* News */}
        <div style={{gridColumn:'span 4', background:'var(--tile-bg-2)', color:'var(--tile-cream)', borderRadius:22, padding:'18px 20px', border:'1px solid var(--tile-stroke)'}}>
          <div style={{display:'flex', justifyContent:'space-between', marginBottom:10}}>
            <div style={{fontFamily:'Inter Tight', fontSize:14, fontWeight:600}}>News</div>
            <div className="mono" style={{fontSize:10, color:'color-mix(in srgb, var(--tile-cream) 55%, transparent)'}}>LIVE</div>
          </div>
          {D.news.slice(0,3).map((n,i)=>(
            <div key={i} style={{paddingBlock:8, borderBottom:'1px solid color-mix(in srgb, var(--tile-cream) 8%, transparent)'}}>
              <div style={{display:'flex', justifyContent:'space-between', fontFamily:'JetBrains Mono', fontSize:9, color:'color-mix(in srgb, var(--tile-cream) 45%, transparent)', letterSpacing:'.1em'}}>
                <span>{n.t} · {n.src}</span><span style={{color: n.tone==='pos'?'var(--tile-green)':n.tone==='neg'?'var(--tile-red)':'color-mix(in srgb, var(--tile-cream) 45%, transparent)'}}>{n.tag}</span>
              </div>
              <div style={{fontFamily:'Inter Tight', fontSize:12, marginTop:4, lineHeight:1.35}}>{n.head}</div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

// ----- Variant B: editorial dark with BURST as hero — themed
function TilesVariantB({ D }){
  const burstData = D.candles.map(c => c.c - 1140);
  return (
    <div style={{flex:1, minHeight:0, overflow:'auto', padding: 22, background:'var(--tile-bg)', color:'var(--tile-cream)'}}
         data-screen-label="04 Tiles · B">
      <div style={{display:'grid', gridTemplateColumns:'repeat(12, 1fr)', gridAutoRows:'minmax(120px, auto)', gap:14}}>
        {/* HERO BURST */}
        <div style={{gridColumn:'span 7', gridRow:'span 3', background:'var(--tile-bg-2)', borderRadius:24, padding:'24px 28px', position:'relative', overflow:'hidden', border:'1px solid var(--tile-stroke)',
                     '--paper':'var(--tile-bg-2)','--rule':'color-mix(in srgb, var(--tile-cream) 18%, transparent)','--hair':'color-mix(in srgb, var(--tile-cream) 8%, transparent)','--ink':'var(--tile-cream)','--muted':'color-mix(in srgb, var(--tile-cream) 60%, transparent)'}}>
          <div style={{display:'flex', justifyContent:'space-between'}}>
            <div>
              <div style={{fontFamily:'JetBrains Mono', fontSize:10, letterSpacing:'.18em', opacity:.6}}>112 SESSIONS · 5-MIN BURST · S&P 500</div>
              <div style={{fontFamily:'Inter Tight', fontSize:32, fontWeight:500, letterSpacing:'-0.03em', marginTop:8, maxWidth:520, lineHeight:1.1}}>
                One year of intraday volatility, drawn as a sprint chart.
              </div>
            </div>
            <div style={{textAlign:'right'}}>
              <div style={{fontFamily:'Inter Tight', fontSize:64, fontWeight:500, letterSpacing:'-0.04em', color:'var(--tile-yellow)', lineHeight:.9}}>+18.4%</div>
              <div style={{fontFamily:'JetBrains Mono', fontSize:10, opacity:.6, marginTop:6}}>YTD · n=23,184</div>
            </div>
          </div>
          <div style={{height:380, marginTop:8}}>
            <BurstChart data={burstData} size={380}
              fillBars="var(--tile-yellow)" fillTrack="color-mix(in srgb, var(--tile-cream) 8%, transparent)"
              showYears={true} startYear={2024}
              label="$5,824" sub="LAST · S&P 500"/>
          </div>
          <div style={{position:'absolute', bottom:18, left:28, fontFamily:'Inter Tight', fontSize:11, color:'color-mix(in srgb, var(--tile-cream) 50%, transparent)', maxWidth:260, lineHeight:1.4}}>
            Each ray is the 5-minute true range, scaled. The longest ray marks Aug 5 ‘24 — the gap-down session. The amber notch at 12 o'clock is today.
          </div>
          <div style={{position:'absolute', bottom:18, right:28, fontFamily:'JetBrains Mono', fontSize:9, color:'color-mix(in srgb, var(--tile-cream) 40%, transparent)'}}>
            <div>MERIDIAN.</div><div>RESEARCH 04/26</div>
          </div>
        </div>

        {/* Stat: Sharpe */}
        <div style={{gridColumn:'span 5', background:'var(--tile-yellow)', color:'var(--tile-fg-on-warm)', borderRadius:24, padding:'24px 28px',
                     display:'flex', flexDirection:'column', justifyContent:'space-between'}}>
          <div>
            <div style={{fontFamily:'Inter Tight', fontSize:13, fontWeight:600}}>Risk-adjusted return</div>
            <div style={{fontFamily:'JetBrains Mono', fontSize:10, opacity:.6, marginTop:2}}>SHARPE · TRAILING 12M</div>
          </div>
          <div style={{fontFamily:'Inter Tight', fontSize:128, fontWeight:500, letterSpacing:'-0.05em', lineHeight:.85}}>2.41</div>
          <div style={{fontFamily:'JetBrains Mono', fontSize:10, display:'flex', gap:14, opacity:.7}}>
            <span>vs S&P 1.18</span><span>vs HF idx 0.92</span>
          </div>
        </div>

        {/* Concentric rings: sector */}
        <div style={{gridColumn:'span 5', gridRow:'span 2', background:'var(--tile-bg-2)', borderRadius:24, padding:'20px 24px', border:'1px solid var(--tile-stroke)',
                     '--paper':'var(--tile-bg-2)','--rule':'color-mix(in srgb, var(--tile-cream) 18%, transparent)','--hair':'color-mix(in srgb, var(--tile-cream) 8%, transparent)','--ink':'var(--tile-cream)'}}>
          <div style={{display:'flex', justifyContent:'space-between'}}>
            <div style={{fontFamily:'Inter Tight', fontSize:14, fontWeight:600}}>Sector exposure</div>
            <div style={{fontFamily:'JetBrains Mono', fontSize:10, opacity:.6}}>3 LAYERS</div>
          </div>
          <div style={{height:260, marginTop:8}}>
            <ConcentricRings rings={[
              {label:'PORTFOLIO', values:[42,18,14,12,8,6], colors:['var(--tile-yellow)','var(--tile-warm)','var(--tile-green)','var(--tile-blue)','var(--tile-red)','var(--tile-bg)']},
              {label:'BENCHMARK', values:[31,9,11,15,12,22], colors:['color-mix(in srgb, var(--tile-yellow) 55%, transparent)','color-mix(in srgb, var(--tile-warm) 55%, transparent)','color-mix(in srgb, var(--tile-green) 55%, transparent)','color-mix(in srgb, var(--tile-blue) 55%, transparent)','color-mix(in srgb, var(--tile-red) 55%, transparent)','color-mix(in srgb, var(--tile-cream) 25%, transparent)']},
              {label:'12M FLOW',  values:[18,7,12,16,4,3], colors:['var(--tile-yellow)','var(--tile-warm)','var(--tile-green)','var(--tile-blue)','color-mix(in srgb, var(--tile-cream) 40%, transparent)','color-mix(in srgb, var(--tile-cream) 25%, transparent)']},
            ]} size={240}/>
          </div>
          <div style={{display:'grid', gridTemplateColumns:'repeat(3, 1fr)', gap:8, fontFamily:'JetBrains Mono', fontSize:10, color:'color-mix(in srgb, var(--tile-cream) 70%, transparent)', marginTop:6}}>
            <div><span style={{display:'inline-block', width:8,height:8,background:'var(--tile-yellow)',marginRight:6}}/>Tech 42</div>
            <div><span style={{display:'inline-block', width:8,height:8,background:'var(--tile-warm)',marginRight:6}}/>Comm 18</div>
            <div><span style={{display:'inline-block', width:8,height:8,background:'var(--tile-green)',marginRight:6}}/>Disc 14</div>
            <div><span style={{display:'inline-block', width:8,height:8,background:'var(--tile-blue)',marginRight:6}}/>Fin 12</div>
            <div><span style={{display:'inline-block', width:8,height:8,background:'var(--tile-red)',marginRight:6}}/>HC 8</div>
            <div><span style={{display:'inline-block', width:8,height:8,background:'color-mix(in srgb, var(--tile-cream) 40%, transparent)',marginRight:6}}/>Cash 6</div>
          </div>
        </div>

        {/* Mosaic */}
        <div style={{gridColumn:'span 4', gridRow:'span 2', background:'var(--tile-cream)', color:'var(--tile-fg-on-warm)', borderRadius:24, padding:'20px 24px',
                     '--paper':'var(--tile-cream)','--rule':'var(--tile-fg-on-warm)','--hair':'color-mix(in srgb, var(--tile-fg-on-warm) 15%, transparent)','--ink':'var(--tile-fg-on-warm)','--accent':'var(--tile-warm)'}}>
          <div style={{display:'flex', justifyContent:'space-between'}}>
            <div style={{fontFamily:'Inter Tight', fontSize:14, fontWeight:600}}>Liquidity matrix</div>
            <div style={{fontFamily:'JetBrains Mono', fontSize:10, opacity:.6}}>14×14 · TOP 196</div>
          </div>
          <div style={{height:240, marginTop:6}}>
            <MosaicGrid rows={14} cols={14} density={0.48} accentEvery={11} fg="var(--tile-fg-on-warm)" accent="var(--tile-warm)"/>
          </div>
          <div style={{fontFamily:'JetBrains Mono', fontSize:10, opacity:.6, marginTop:6, lineHeight:1.4}}>
            Each cell = a name's median spread. Filled = inside 4bps. Orange = today's outliers.
          </div>
        </div>

        {/* Radial bars */}
        <div style={{gridColumn:'span 4', gridRow:'span 2', background:'var(--tile-bg-2)', borderRadius:24, padding:'20px 24px', border:'1px solid var(--tile-stroke)',
                     '--paper':'var(--tile-bg-2)','--rule':'color-mix(in srgb, var(--tile-cream) 18%, transparent)','--hair':'color-mix(in srgb, var(--tile-cream) 8%, transparent)','--ink':'var(--tile-cream)','--muted':'color-mix(in srgb, var(--tile-cream) 60%, transparent)'}}>
          <div style={{display:'flex', justifyContent:'space-between'}}>
            <div style={{fontFamily:'Inter Tight', fontSize:14, fontWeight:600}}>Factor scorecard</div>
            <div style={{fontFamily:'JetBrains Mono', fontSize:10, opacity:.6}}>NORM 0–100</div>
          </div>
          <div style={{height:220, marginTop:6}}>
            <RadialBars items={[
              {label:'MOMENTUM', v:0.82, color:'var(--tile-yellow)'},
              {label:'VALUE',    v:0.41, color:'var(--tile-warm)'},
              {label:'QUALITY',  v:0.74, color:'var(--tile-green)'},
              {label:'GROWTH',   v:0.67, color:'var(--tile-blue)'},
              {label:'VOL',      v:0.52, color:'var(--tile-red)'},
              {label:'YIELD',    v:0.28, color:'color-mix(in srgb, var(--tile-cream) 60%, transparent)'},
            ]} size={260}/>
          </div>
        </div>

        {/* Stacked perf */}
        <div style={{gridColumn:'span 8', background:'var(--tile-bg-2)', borderRadius:24, padding:'18px 22px', border:'1px solid var(--tile-stroke)',
                     '--paper':'var(--tile-bg-2)','--rule':'color-mix(in srgb, var(--tile-cream) 18%, transparent)','--hair':'color-mix(in srgb, var(--tile-cream) 8%, transparent)','--ink':'var(--tile-cream)'}}>
          <div style={{display:'flex', justifyContent:'space-between', marginBottom:10}}>
            <div style={{fontFamily:'Inter Tight', fontSize:14, fontWeight:600}}>Attribution · YTD</div>
            <div style={{fontFamily:'JetBrains Mono', fontSize:10, opacity:.6}}>%</div>
          </div>
          <StackedBars data={[
            {label:'TECH', v:42, color:'var(--tile-yellow)'},
            {label:'COMM', v:18, color:'var(--tile-warm)'},
            {label:'DISC', v:14, color:'var(--tile-green)'},
            {label:'FIN',  v:12, color:'var(--tile-blue)'},
            {label:'HC',   v:8,  color:'var(--tile-red)', fg:'#fff'},
            {label:'CASH', v:6,  color:'color-mix(in srgb, var(--tile-cream) 40%, transparent)'},
          ]} height={64}/>
        </div>
      </div>
    </div>
  );
}

// ----- Variant C: light editorial w/ MOSAIC + petal hero — themed
function TilesVariantC({ D }){
  return (
    <div style={{flex:1, minHeight:0, overflow:'auto', padding: 22, background:'var(--bg)'}}
         data-screen-label="04 Tiles · C">
      <div style={{display:'grid', gridTemplateColumns:'repeat(12, 1fr)', gridAutoRows:'minmax(120px, auto)', gap:14}}>
        {/* HERO Mosaic — retention-style poster */}
        <div style={{gridColumn:'span 7', gridRow:'span 3', background:'var(--paper)', border:'1px solid var(--hair)', borderRadius:20, padding:'24px 28px', position:'relative', overflow:'hidden', color:'var(--ink)'}}>
          <div style={{display:'flex', justifyContent:'space-between'}}>
            <div>
              <div style={{fontFamily:'JetBrains Mono', fontSize:10, letterSpacing:'.18em', color:'var(--muted)'}}>RETENTION · LIQUIDITY · 196 NAMES</div>
              <div style={{fontFamily:'Inter Tight', fontSize:34, fontWeight:500, letterSpacing:'-0.03em', marginTop:8, maxWidth:520, lineHeight:1.05}}>
                Where the book actually fills.
              </div>
            </div>
            <div style={{display:'flex', gap:6, alignItems:'flex-start'}}>
              {['◀◀◀','▶▶▶'].map((g,i)=>(
                <div key={i} style={{fontFamily:'JetBrains Mono', fontSize:14, color:'var(--accent)', letterSpacing:'.06em'}}>{g}</div>
              ))}
            </div>
          </div>
          <div style={{display:'grid', gridTemplateColumns:'1fr 1fr', gap:30, marginTop:18, alignItems:'center'}}>
            <div style={{height:300}}>
              <MosaicGrid rows={14} cols={14} density={0.42} accentEvery={9} fg="var(--ink)" accent="var(--accent)"/>
            </div>
            <div style={{height:300}}>
              <ConcentricRings rings={[
                {values:[52,32,16], colors:['var(--ink)','var(--accent)','color-mix(in srgb, var(--ink) 20%, transparent)']},
                {values:[18,42,40], colors:['var(--ink)','var(--accent)','color-mix(in srgb, var(--ink) 15%, transparent)']},
                {values:[34,12,54], colors:['var(--ink)','var(--accent)','color-mix(in srgb, var(--ink) 10%, transparent)']},
                {values:[60,40], colors:['var(--ink)','var(--accent)']},
              ]} size={300}/>
            </div>
          </div>
          <div style={{position:'absolute', bottom:18, left:28, fontFamily:'JetBrains Mono', fontSize:9, color:'var(--muted)', letterSpacing:'.1em'}}>
            FIG. 04 · n.{'{'}lik-wid-i-tee{'}'} · the rate at which your fills come back to you.
          </div>
          <div style={{position:'absolute', top:24, right:28, display:'flex', flexDirection:'column', gap:6}}>
            {[1,2,3,4].map(i => <div key={i} style={{width:8, height:8, borderRadius:99, background:'var(--accent)', opacity:1-i*0.22}}/>)}
          </div>
        </div>

        {/* Petal sentiment */}
        <div style={{gridColumn:'span 5', gridRow:'span 2', background:'var(--tile-dark)', color:'var(--tile-cream)', borderRadius:20, padding:'20px 24px',
                     '--paper':'var(--tile-dark)','--rule':'color-mix(in srgb, var(--tile-cream) 18%, transparent)','--ink':'var(--tile-cream)','--muted':'color-mix(in srgb, var(--tile-cream) 55%, transparent)'}}>
          <div style={{display:'flex', justifyContent:'space-between'}}>
            <div style={{fontFamily:'Inter Tight', fontSize:14, fontWeight:600}}>Sentiment, by signal</div>
            <div style={{fontFamily:'JetBrains Mono', fontSize:10, opacity:.55}}>n=4,128</div>
          </div>
          <div style={{height:280, marginTop:8}}>
            <FlowerSentiment data={[
              {label:'Anticipation', v:5,  color:'var(--tile-cream)', dark:true},
              {label:'Happiness',    v:12, color:'var(--tile-cream)', dark:true},
              {label:'Awe',          v:10, color:'var(--tile-cream)', dark:true},
              {label:'Admiration',   v:5,  color:'var(--tile-cream)', dark:true},
              {label:'Surprise',     v:12, color:'var(--tile-cream)', dark:true},
              {label:'Sadness',      v:6,  color:'var(--tile-cream)', dark:true},
              {label:'Fear',         v:4,  color:'var(--tile-cream)', dark:true},
              {label:'Anger',        v:2,  color:'var(--tile-cream)', dark:true},
            ]}/>
          </div>
        </div>

        {/* Stat strip */}
        <div style={{gridColumn:'span 4', background:'var(--tile-yellow)', color:'var(--tile-fg-on-warm)', borderRadius:20, padding:'18px 22px',
                     border:'1px solid var(--hair)', display:'flex', flexDirection:'column', justifyContent:'space-between'}}>
          <div>
            <div style={{fontFamily:'Inter Tight', fontSize:13, fontWeight:600}}>Stream & Download</div>
            <div style={{fontFamily:'Inter Tight', fontSize:11, opacity:.55, marginTop:2}}>Realtime feeds active</div>
          </div>
          <div style={{fontFamily:'Inter Tight', fontSize:84, fontWeight:500, letterSpacing:'-0.05em', lineHeight:.85}}>8%</div>
        </div>
        <div style={{gridColumn:'span 4', background:'var(--tile-warm)', color:'var(--tile-fg-on-warm)', borderRadius:20, padding:'18px 22px',
                     display:'flex', flexDirection:'column', justifyContent:'space-between'}}>
          <div>
            <div style={{fontFamily:'Inter Tight', fontSize:13, fontWeight:600}}>Open Risk</div>
            <div style={{fontFamily:'Inter Tight', fontSize:11, opacity:.7, marginTop:2}}>VAR 95% · 1-day</div>
          </div>
          <div style={{fontFamily:'Inter Tight', fontSize:84, fontWeight:500, letterSpacing:'-0.05em', lineHeight:.85}}>17%</div>
        </div>
        <div style={{gridColumn:'span 4', background:'var(--tile-green)', color:'var(--tile-fg-on-warm)', borderRadius:20, padding:'18px 22px',
                     display:'flex', flexDirection:'column', justifyContent:'space-between'}}>
          <div>
            <div style={{fontFamily:'Inter Tight', fontSize:13, fontWeight:600}}>Cash · Margin</div>
            <div style={{fontFamily:'Inter Tight', fontSize:11, opacity:.7, marginTop:2}}>Available buying power</div>
          </div>
          <div style={{fontFamily:'Inter Tight', fontSize:84, fontWeight:500, letterSpacing:'-0.05em', lineHeight:.85}}>10%</div>
        </div>

        {/* Yield curve */}
        <div style={{gridColumn:'span 7', background:'var(--paper)', border:'1px solid var(--hair)', borderRadius:20, padding:'18px 22px', color:'var(--ink)'}}>
          <div style={{display:'flex', justifyContent:'space-between', marginBottom:6}}>
            <div style={{fontFamily:'Inter Tight', fontSize:14, fontWeight:600}}>US Treasury · curve</div>
            <div style={{fontFamily:'JetBrains Mono', fontSize:10, color:'var(--muted)'}}>10:42 ET</div>
          </div>
          <MarketBands tenors={['1M','3M','6M','1Y','2Y','5Y','10Y','30Y']}
                       values={[5.32, 5.28, 5.18, 4.92, 4.62, 4.32, 4.21, 4.42]}
                       size={520} height={140}/>
        </div>

        {/* Indices micro w/ sparkline */}
        <div style={{gridColumn:'span 5', background:'var(--paper)', border:'1px solid var(--hair)', borderRadius:20, padding:'18px 22px', color:'var(--ink)'}}>
          <div style={{display:'flex', justifyContent:'space-between', marginBottom:8}}>
            <div style={{fontFamily:'Inter Tight', fontSize:14, fontWeight:600}}>Indices</div>
            <div style={{fontFamily:'JetBrains Mono', fontSize:10, color:'var(--muted)'}}>4D · OPEN</div>
          </div>
          {[
            {l:'S&P 500', v:'5,824.10', p:'+0.38%', up:true,  d:[10,11,9,12,11,13,12,14,13,15,14,16,15,17]},
            {l:'NASDAQ',  v:'18,642.20',p:'+0.85%', up:true,  d:[8,10,9,11,10,12,11,13,14,12,15,14,16,17]},
            {l:'DOW 30',  v:'42,118.40',p:'−0.12%', up:false, d:[14,13,15,14,12,13,11,12,10,11,9,10,8,9]},
            {l:'VIX',     v:'14.82',    p:'−3.21%', up:false, d:[18,17,16,15,16,14,15,13,12,11,10,11,9,10]},
          ].map((r,i)=>(
            <div key={i} style={{display:'grid', gridTemplateColumns:'70px 1fr 100px 70px', alignItems:'center', padding:'8px 0', borderBottom:'1px solid var(--hair)', fontFamily:'JetBrains Mono', fontSize:11}}>
              <span style={{fontWeight:700, color:'var(--ink)'}}>{r.l}</span>
              <div style={{paddingInline:8}}>
                <Sparkline values={r.d} width={120} height={28} color={r.up?'var(--up)':'var(--down)'} fill={r.up?'var(--up)':'var(--down)'}/>
              </div>
              <span style={{textAlign:'right'}}>{r.v}</span>
              <span style={{textAlign:'right', color: r.up?'var(--up)':'var(--down)'}}>{r.p}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

Object.assign(window, { TradePage, TilesPage });
