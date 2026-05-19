// Trade Alt + Apex Alt — Lucid-native versions that MIRROR original pages
// Trade Alt = TradePage (DOM | Chart+floating widgets | Watchlist)
// Apex Alt  = ApexLayout (toolbar + [ladder | main | rail] + sub charts)
// Only the surface treatment changes (cards, segmented dividers, fonts).

const { useState: lS, useEffect: lE, useRef: lR, useMemo: lM } = React;

// ============================================================
// TRADE ALT — mirrors TradePage layout: DOM | Chart + floating | Watchlist
// Single rounded container per column, internal dividers instead of separate cards.
// ============================================================
function TradeAltPage({ activeRow, setActiveSym, D, showVol, showMA, showBB, variant='A' }){
  return (
    <div className="lc-trade-alt" data-screen-label="03 Trade · Alt">
      <div className="lc-trade-alt-grid">
        {/* LEFT: DOM ladder card */}
        <div className="lc-card">
          <div className="lc-section-head">
            <span className="lc-section-ttl">DOM · {activeRow.sym}</span>
            <span className="lc-section-meta">Ladder</span>
          </div>
          <div className="lc-section-body lc-dom-body">
            <LcDomLadder mid={activeRow.last}/>
          </div>
        </div>

        {/* CENTER: chart card with internal toolbar + floating widgets */}
        <div className="lc-card lc-chart-card">
          <div className="lc-chart-toolbar">
            <div className="grp">
              {['1m','5m','15m','30m','1H','4H','1D','1W'].map(tf => (
                <span key={tf} className={'tf '+(tf==='5m'?'on':'')}>{tf}</span>
              ))}
            </div>
            <div className="grp">
              <span className="tf">＋ Indicator</span>
              <span className="tf">⊞ Compare</span>
            </div>
          </div>
          <div className="lc-chart-body" style={{position:'relative'}}>
            <div className="lc-chart-tools">
              {['↖','│','╱','◯','▭','T','⌖','⊕','▾'].map((g,i)=>(
                <div key={i} className={'t '+(i===2?'on':'')}>{g}</div>
              ))}
            </div>
            <div className="lc-chart-canvas">
              <Candles candles={D.candles} width={1100} height={520}
                       showVol={showVol} showMA={showMA} showBB={showBB}/>
            </div>
            {variant === 'A'
              ? <LcFloatingA activeRow={activeRow} D={D}/>
              : <LcFloatingB activeRow={activeRow} D={D}/>}
          </div>
        </div>

        {/* RIGHT: watchlist card with quick orders strip */}
        <div className="lc-card">
          <div className="lc-section-head">
            <span className="lc-section-ttl">Watchlist</span>
            <span className="lc-section-meta">{D.watchlist.length} · Top movers</span>
          </div>
          <div className="lc-section-body" style={{padding:0, overflow:'auto', flex:1}}>
            <Watchlist data={D.watchlist} active={activeRow.sym} onSelect={setActiveSym}/>
          </div>
          <div className="lc-section-divider"/>
          <div className="lc-quick-orders">
            <div className="lc-section-meta" style={{marginBottom:8}}>Quick orders</div>
            <div className="lc-qo-grid">
              <button className="lc-qo buy">Buy MKT</button>
              <button className="lc-qo sell">Sell MKT</button>
              <button className="lc-qo">Buy LMT</button>
              <button className="lc-qo">Sell LMT</button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function LcDomLadder({ mid=1284.32 }){
  const tick = 0.05;
  const rows = [];
  for (let i=14; i>=-14; i--){
    const p = +(mid + i*tick).toFixed(2);
    const isMid = i===0;
    const sz = Math.abs(i)===0 ? 0 : Math.round(40 + Math.sin(i*1.3+5)*40 + Math.cos(i*0.7)*30 + 80);
    rows.push({i, p, isMid, sz: Math.abs(sz), side: i>0 ? 'ask' : 'bid'});
  }
  return (
    <div className="lc-dom">
      <div className="lc-dom-head">
        <span>Buy</span>
        <span>Bid</span>
        <span>Price</span>
        <span>Ask</span>
      </div>
      <div className="lc-dom-rows">
        {rows.map(r => (
          <div key={r.i} className={'lc-dom-row '+(r.isMid?'mid ':'')+r.side}>
            <span className="buy">{r.side==='bid' && <span className="mono">{r.sz}</span>}</span>
            <span className="bid">{r.side==='bid' && <span className="mono">{r.sz}</span>}</span>
            <span className="price mono">{r.p.toFixed(2)}</span>
            <span className="ask">{r.side==='ask' && <span className="mono">{r.sz}</span>}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

// Lucid-flavored floating widgets — same set as TradePage, restyled
function LcFloatingA({ activeRow, D }){
  return (
    <>
      <div className="lc-float-grp top-left">
        <div className="lc-float-card accent">
          <div className="lc-float-lab">{activeRow.sym} · Intraday</div>
          <div className="lc-float-val">{activeRow.pct>=0?'+':''}{activeRow.pct.toFixed(2)}%</div>
        </div>
        <div className="lc-float-card dark">
          <div className="lc-float-lab">Session</div>
          <div className="lc-float-val">02:14<span className="suffix">PM</span></div>
          <div className="lc-float-sub">1h 46m to close</div>
        </div>
      </div>
      <div className="lc-float-grp top-right">
        <div className="lc-float-card">
          <div className="lc-float-lab">Sentiment · 24h <span className="muted">n=4,128</span></div>
          <div style={{height:140, marginTop:6}}>
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
      </div>
      <div className="lc-float-grp bottom-center">
        <div className="lc-float-pill-bar">
          <button className="pill buy">Buy 100</button>
          <button className="pill sell">Sell 100</button>
          <button className="pill">Flatten</button>
          <button className="pill">Reverse</button>
          <button className="pill accent">Bracket</button>
        </div>
      </div>
      <div className="lc-float-grp bottom-right">
        <div className="lc-float-card dark">
          <div className="lc-float-lab">Open P&L · {activeRow.sym}</div>
          <div className="lc-float-val accent">+$169,824</div>
          <div className="lc-float-row">
            <span>1,200 @ 1142.80</span>
            <span className="accent">+12.39%</span>
          </div>
        </div>
      </div>
      <div className="lc-float-grp bottom-left">
        {[{l:'VWAP',v:'1,261.42'},{l:'σ 5m',v:'0.84%'},{l:'IV',v:'42.6'},{l:'RSI',v:'62.4'}].map((s,i)=>(
          <div key={i} className="lc-float-mini">
            <div className="lc-float-lab">{s.l}</div>
            <div className="mono v">{s.v}</div>
          </div>
        ))}
      </div>
    </>
  );
}

function LcFloatingB({ activeRow, D }){
  const burstData = D.candles.slice(-40).map(c => c.c - 1140);
  return (
    <>
      <div className="lc-float-grp top-left">
        <div className="lc-float-card" style={{width:240}}>
          <div className="lc-float-lab">5-min burst · last 40 <span className="muted">NVDA</span></div>
          <div style={{height:180, marginTop:4}}>
            <BurstChart data={burstData} size={220}
                        label={(activeRow.pct>=0?'+':'')+activeRow.pct.toFixed(2)+'%'} sub="Intraday"
                        showYears={false} fillBars="var(--accent)"/>
          </div>
        </div>
      </div>
      <div className="lc-float-grp top-right">
        <div className="lc-float-card dark" style={{width:240,
          '--paper':'var(--ink)','--rule':'color-mix(in srgb, var(--bg) 18%, transparent)','--hair':'color-mix(in srgb, var(--bg) 10%, transparent)','--ink':'var(--bg)'}}>
          <div className="lc-float-lab" style={{color:'color-mix(in srgb, var(--bg) 70%, transparent)'}}>Order flow · book depth</div>
          <div style={{height:180, marginTop:4}}>
            <ConcentricRings rings={[
              {label:'BIDS', values:[58,42], colors:['var(--up)','color-mix(in srgb, var(--up) 25%, transparent)']},
              {label:'ASKS', values:[51,49], colors:['var(--down)','color-mix(in srgb, var(--down) 25%, transparent)']},
              {label:'SWEEP',values:[34,66], colors:['var(--accent)','color-mix(in srgb, var(--bg) 30%, transparent)']},
            ]} size={200}/>
          </div>
          <div className="lc-float-row mono">
            <span style={{color:'var(--up)'}}>BUY 58%</span>
            <span style={{color:'var(--down)'}}>SELL 42%</span>
          </div>
        </div>
      </div>
      <div className="lc-float-grp bottom-left" style={{width:340}}>
        <div className="lc-float-card" style={{width:'100%'}}>
          <div className="lc-float-lab">IV term structure <span className="muted">30D · 45.2 IVR</span></div>
          <MarketBands tenors={['1W','2W','1M','2M','3M','6M','1Y']}
                       values={[58.2, 52.1, 46.4, 44.8, 43.2, 41.0, 39.6]}
                       size={316} height={120}/>
        </div>
      </div>
      <div className="lc-float-grp bottom-right">
        <div className="lc-float-card dark">
          <div className="lc-float-lab">Open P&L · {activeRow.sym}</div>
          <div className="lc-float-val accent">+$169,824</div>
          <div className="lc-float-row mono">
            <span>1,200 @ 1142.80</span>
            <span className="accent">+12.39%</span>
          </div>
        </div>
      </div>
    </>
  );
}


// ============================================================
// APEX ALT — mirrors ApexLayout: toolbar + [ladder | main | rail] + sub charts
// One outer card per region; toolbar is its own slim card; main + ladder + rail
// live inside ONE rounded container split by internal hairlines.
// ============================================================
function ApexAltPage({ activeRow, setActiveSym, D, showVol, showMA, showBB }){
  const [tf, setTf] = lS('5m');
  const [dd, setDd] = lS(null);  // {k, x, y}
  const [ctx, setCtx] = lS(null); // {x, y, price}

  function openDD(k, e){
    const r = e.currentTarget.getBoundingClientRect();
    setDd(d => d?.k === k ? null : { k, x: r.left, y: r.bottom + 6 });
    setCtx(null);
  }
  function onChartCtx(e){
    e.preventDefault();
    setCtx({ x: e.clientX, y: Math.min(e.clientY, window.innerHeight - 460), price: activeRow.last.toFixed(2) });
    setDd(null);
  }

  lE(()=>{
    function k(e){ if (e.key==='Escape'){ setDd(null); setCtx(null); } }
    function c(e){
      if (dd && !e.target.closest?.('.lc-pop') && !e.target.closest?.('.lc-btn.dd')) setDd(null);
      if (ctx && !e.target.closest?.('.lc-pop')) setCtx(null);
    }
    window.addEventListener('keydown', k);
    document.addEventListener('click', c);
    return ()=>{ window.removeEventListener('keydown', k); document.removeEventListener('click', c); };
  }, [dd, ctx]);

  return (
    <div className="lc-apex-alt" data-screen-label="0X Apex · Alt">
      {/* TOOLBAR — single thin card */}
      <div className="lc-apex-bar">
        <div className="grp tic">
          <span className="acc-dot"/>
          <span className="sym">{activeRow.sym}</span>
          <span className="name">· {activeRow.name||'NVIDIA Corp'}</span>
        </div>
        <div className="grp">
          <span className="lab">TF</span>
          <div className="lc-seg-tf">
            {['1m','5m','15m','1h','3h','1d','1w','1mo'].map(t=>(
              <button key={t} onClick={()=>setTf(t)} className={tf===t?'on':''}>{t}</button>
            ))}
          </div>
        </div>
        <div className="grp">
          {['Vol','Overlay','Tools','Studies','Widgets','Default'].map(s=>(
            <button key={s} onClick={(e)=>openDD(s, e)}
                    className={'lc-btn ghost dd '+(dd?.k===s?'on':'')}>
              {s}<span className="caret">▾</span>
            </button>
          ))}
        </div>
        <div className="grp" style={{marginLeft:'auto'}}>
          <button className="lc-btn ghost"><span className="ic">⊞</span>Window</button>
          <button className="lc-btn ghost accent-text"><span className="ic">◐</span>Signals</button>
          <button className="lc-btn ghost"><span className="ic">≡</span>Analysis</button>
          <button className="lc-btn accent"><span className="ic">$</span>Ticket</button>
          <button className="lc-btn ghost"><span className="ic">▷</span>Playback</button>
          <button className="lc-btn ghost"><span className="ic">⛁</span>Feed</button>
        </div>
      </div>

      {/* MAIN ROW — DOM ladder | chart | rail, all inside ONE rounded card */}
      <div className="lc-card lc-apex-main-card">
        <div className="lc-apex-main-grid">
          <div className="cell ladder">
            <div className="lc-section-head">
              <span className="lc-section-ttl">Depth</span>
              <span className="lc-section-meta">Lvl 2</span>
            </div>
            <div className="lc-section-body" style={{padding:0, flex:1, overflow:'auto'}}>
              <LcApexLadder mid={activeRow.last}/>
            </div>
            <div className="lc-section-divider"/>
            <div className="lc-apex-ladder-foot">
              <button className="lc-qo buy">Buy MKT</button>
              <button className="lc-qo sell">Sell MKT</button>
            </div>
          </div>

          <div className="cell chart" onContextMenu={onChartCtx}>
            <LcApexChartHead sym={activeRow.sym} candles={D.candles} timeframe={tf} row={activeRow}/>
            <div className="lc-chart-body" style={{flex:1, minHeight:0}}>
              <Candles candles={D.candles} width={1200} height={520}
                       showVol={showVol} showMA={showMA} showBB={showBB}/>
            </div>
          </div>

          <div className="cell rail">
            <div className="lc-section-head">
              <span className="lc-section-ttl">Watchlist · Chain</span>
              <span className="lc-section-meta">A</span>
            </div>
            <div className="lc-section-body" style={{padding:0, flex:1, overflow:'auto'}}>
              <Watchlist data={D.watchlist} active={activeRow.sym} onSelect={setActiveSym}/>
            </div>
            <div className="lc-section-divider"/>
            <div className="lc-rail-opts">
              <div className="lc-section-meta" style={{marginBottom:6, padding:'0 12px'}}>Options · {activeRow.sym}</div>
              {[
                {k:'C', tag:'SPY 716 0DTE', a:'2.97', b:'2.79'},
                {k:'C', tag:'SPY 715 0DTE', a:'3.44', b:'3.59'},
                {k:'P', tag:'SPY 711 0DTE', a:'3.10', b:'3.18'},
                {k:'P', tag:'SPY 709 0DTE', a:'3.32', b:'3.40'},
              ].map((o,i)=>(
                <div key={i} className="lc-opt-row">
                  <span className={'k '+(o.k==='C'?'up':'down')}>{o.k}</span>
                  <span className="t">{o.tag}</span>
                  <span className="px mono">{o.a} <span className="muted">×</span> {o.b}</span>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>

      {/* SUB ROW — two mini-chart cards sharing one container */}
      <div className="lc-card lc-apex-sub-card">
        <div className="lc-apex-sub-grid">
          <div className="cell">
            <LcApexChartHead sym="AAPL" candles={shiftCandlesLC(D.candles, 178, 0.62)}
                             timeframe="15m" mini
                             row={D.watchlist.find(w=>w.sym==='AAPL')||{sym:'AAPL',name:'Apple',last:178.42,chg:1.18,pct:0.66}}/>
            <div className="lc-chart-body lc-sub-body">
              <Candles candles={shiftCandlesLC(D.candles, 178, 0.62)} width={520} height={200}
                       showVol={false} showMA={showMA}/>
            </div>
            <div className="lc-section-divider"/>
            <LcMiniTicket sym="AAPL" last={178.42}/>
          </div>
          <div className="cell">
            <LcApexChartHead sym="SPY" candles={shiftCandlesLC(D.candles, 528, 0.30)}
                             timeframe="30m" mini
                             row={D.watchlist.find(w=>w.sym==='SPY')||{sym:'SPY',name:'S&P 500',last:528.61,chg:-1.94,pct:-0.36}}/>
            <div className="lc-chart-body lc-sub-body">
              <Candles candles={shiftCandlesLC(D.candles, 528, 0.30)} width={520} height={200}
                       showVol={false} showMA={showMA}/>
            </div>
            <div className="lc-section-divider"/>
            <LcMiniTicket sym="SPY" last={528.61}/>
          </div>
        </div>
      </div>
      {dd && <LcToolbarDrop dropKey={dd.k} x={dd.x} y={dd.y}/>}
      {ctx && <LcChartCtxMenu x={ctx.x} y={ctx.y} price={ctx.price}/>}
    </div>
  );
}

function LcToolbarDrop({ dropKey:k, x, y }){
  const sets = {
    'Vol':     { ttl:'Volume', rows:[
      {ic:'▮', l:'Volume bars', on:true},
      {ic:'⌇', l:'Volume profile'},
      {ic:'≈', l:'VWAP', acc:true},
      {ic:'⊕', l:'Cumulative delta'},
      {ic:'◐', l:'Buy/Sell ratio'},
    ]},
    'Overlay': { ttl:'Overlay', rows:[
      {ic:'≈', l:'Technical', arrow:true},
      {ic:'≡', l:'Structure', arrow:true},
      {ic:'⌇', l:'Regime',    arrow:true},
      {ic:'▮', l:'Data',      arrow:true},
      {div:true},
      {ic:'⊞', l:'Add symbol overlay', acc:true},
    ]},
    'Tools':   { ttl:'Drawing tools', rows:[
      {ic:'⟍', l:'Trend line', on:true},
      {ic:'╱', l:'Ray'},
      {ic:'▭', l:'Rectangle'},
      {ic:'◯', l:'Ellipse'},
      {ic:'⟚', l:'Measure'},
      {div:true},
      {ic:'≡', l:'Fibonacci', arrow:true},
      {ic:'⌗', l:'Gann/Pitchfork', arrow:true},
      {ic:'◍', l:'Harmonic', arrow:true},
    ]},
    'Studies': { ttl:'Indicators', rows:[
      {ic:'∿', l:'RSI 14',  on:true},
      {ic:'∿', l:'MACD'},
      {ic:'≈', l:'Bollinger bands'},
      {ic:'≡', l:'Stochastic'},
      {ic:'⌇', l:'ATR'},
      {ic:'◯', l:'Ichimoku'},
      {div:true},
      {ic:'⊕', l:'Add custom…', acc:true},
    ]},
    'Widgets': { ttl:'Chart widgets', rows:[
      {ic:'▭', l:'Mini chart', on:true},
      {ic:'☐', l:'Heatmap'},
      {ic:'◌', l:'Order flow donut'},
      {ic:'⌗', l:'Liquidity grid'},
      {ic:'≡', l:'Term structure'},
      {ic:'$', l:'P&L card'},
    ]},
    'Default': { ttl:'Templates', rows:[
      {ic:'★', l:'Day scalper', on:true},
      {ic:'★', l:'Swing · 4H'},
      {ic:'★', l:'Options vol surface'},
      {ic:'★', l:'Macro rates'},
      {div:true},
      {ic:'⊕', l:'Save as template…', acc:true},
    ]},
  };
  const s = sets[k];
  if (!s) return null;
  return (
    <div className="lc-pop" style={{left:x, top:y}} onClick={e=>e.stopPropagation()}>
      <div className="lc-pop-head">{s.ttl}</div>
      {s.rows.map((r,i)=> r.div ? <div key={i} className="lc-pop-div"/> : (
        <div key={i} className={'lc-pop-row '+(r.on?'on ':'')+(r.acc?'acc ':'')}>
          <span className="ic">{r.ic}</span>
          <span className="l">{r.l}</span>
          {r.on && <span className="ck">✓</span>}
          {r.arrow && <span className="ar">›</span>}
        </div>
      ))}
    </div>
  );
}

function LcChartCtxMenu({ x, y, price }){
  const groups = [
    {sec:null, rows:[
      {ic:'⟲', l:'Reset view'},
      {ic:'⊕', l:'Drag zoom'},
      {ic:'⟚', l:'Measure', kbd:'⇧+Drag'},
    ]},
    {sec:`Orders @ ${price}`, rows:[
      {ic:'↑', l:'Buy order',  c:'up'},
      {ic:'↓', l:'Sell order', c:'down'},
      {ic:'⛨', l:'Stop loss',  c:'down'},
      {ic:'☐', l:'OCO bracket', c:'acc'},
      {ic:'☐', l:'Bracket presets', c:'acc', arrow:true},
      {ic:'⟲', l:'Trigger order',   c:'acc'},
    ]},
    {sec:`Alerts @ ${price}`, rows:[
      {ic:'↑', l:`Alert above ${price}`},
      {ic:'↓', l:`Alert below ${price}`},
    ]},
    {sec:'Drawing tools', rows:[
      {ic:'⟍', l:'Lines', arrow:true},
      {ic:'│', l:'Channels', arrow:true},
      {ic:'≡', l:'Fibonacci', arrow:true},
      {ic:'▭', l:'Ranges', arrow:true},
      {ic:'⌗', l:'Patterns', arrow:true},
    ]},
    {sec:null, rows:[
      {ic:'★', l:'Save as template'},
      {ic:'◍', l:'Hide all'},
      {ic:'◐', l:'Hide / Show', arrow:true},
    ]},
    {sec:null, rows:[
      {ic:'⌫', l:'Delete all drawings', c:'down'},
      {ic:'⌫', l:'Delete temp (4)', c:'down'},
    ]},
  ];
  return (
    <div className="lc-pop lc-ctx" style={{left:x, top:y}} onClick={e=>e.stopPropagation()}>
      {groups.map((g,gi)=>(
        <React.Fragment key={gi}>
          {gi>0 && <div className="lc-pop-div"/>}
          {g.sec && <div className="lc-pop-sec">{g.sec}</div>}
          {g.rows.map((r,ri)=>(
            <div key={ri} className={'lc-pop-row '+(r.c||'')}>
              <span className="ic">{r.ic}</span>
              <span className="l">{r.l}</span>
              {r.kbd && <span className="kbd">{r.kbd}</span>}
              {r.arrow && <span className="ar">›</span>}
            </div>
          ))}
        </React.Fragment>
      ))}
    </div>
  );
}

function LcApexChartHead({ sym, candles, timeframe, row, mini }){
  const last = candles[candles.length-1];
  const chg = last.c - candles[0].c;
  const pct = chg / candles[0].c * 100;
  return (
    <div className={'lc-apex-chart-head '+(mini?'mini':'')}>
      <div className="lhs">
        <span className="acc-dot"/>
        <span className="sym">{sym}</span>
        <span className="tf">· {timeframe}</span>
        <span className="ohlc mono">
          O <b>{last.o.toFixed(2)}</b>
          &nbsp;H <b>{last.h.toFixed(2)}</b>
          &nbsp;L <b>{last.l.toFixed(2)}</b>
          &nbsp;C <b>{last.c.toFixed(2)}</b>
        </span>
        <span className={'chg mono '+(chg>=0?'up':'down')}>
          {chg>=0?'+':''}{chg.toFixed(2)} ({chg>=0?'+':''}{pct.toFixed(2)}%)
        </span>
      </div>
      <div className="rhs">
        <span className="lab mono">VOL <b>{(last.v/1e6).toFixed(2)}M</b></span>
      </div>
    </div>
  );
}

function LcApexLadder({ mid }){
  const tick = 0.05;
  const N = 26;
  const rows = [];
  for (let i=N; i>=-N; i--){
    const p = +(mid + i*tick).toFixed(2);
    const isLast = i===0;
    const dist = Math.abs(i);
    const seed = Math.sin(p*13.7)+Math.cos(p*7.2);
    const sz = Math.max(0, Math.round((1.4 - dist*0.04) * (220 + seed*120)));
    rows.push({i, p, isLast, sz, side: i>0?'ask':'bid'});
  }
  const maxSz = Math.max(...rows.map(r=>r.sz));
  return (
    <div className="lc-apex-ladder">
      <div className="lc-apex-ladder-head">
        <span>Bid</span><span>Price</span><span>Ask</span>
      </div>
      <div className="lc-apex-ladder-body">
        {rows.map((r,i)=>{
          const w = r.sz/maxSz*100;
          return (
            <div key={i} className={'lc-apex-ladder-row '+(r.isLast?'last ':'')+r.side}>
              <div className="bid">
                {r.side==='bid' && <>
                  <div className="bar" style={{width:w+'%'}}/>
                  <span className="n mono">{r.sz}</span>
                </>}
              </div>
              <div className="price mono">{r.p.toFixed(2)}</div>
              <div className="ask">
                {r.side==='ask' && <>
                  <div className="bar" style={{width:w+'%'}}/>
                  <span className="n mono">{r.sz}</span>
                </>}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function LcMiniTicket({ sym, last }){
  const notional = (100*last/1000).toFixed(1);
  return (
    <div className="lc-mini-ticket">
      <div className="grp"><span className="lab">Qty</span><span className="mono v">100</span></div>
      <div className="grp"><span className="lab">@</span><span className="mono v">{last.toFixed(2)}</span></div>
      <button className="lc-qo sm buy">Buy {notional}K</button>
      <button className="lc-qo sm sell">Sell {notional}K</button>
    </div>
  );
}

function shiftCandlesLC(src, base, vol){
  const last = src[src.length-1].c;
  const ratio = base/last;
  return src.map((c,i)=>({
    t: c.t,
    o: c.o*ratio + (Math.sin(i*0.7)*vol),
    h: c.h*ratio + (Math.cos(i*0.4)*vol*1.2),
    l: c.l*ratio - (Math.sin(i*0.5)*vol*1.2),
    c: c.c*ratio + (Math.cos(i*0.6)*vol),
    v: c.v*0.8,
  }));
}

Object.assign(window, { TradeAltPage, ApexAltPage });
