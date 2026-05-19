// Apex layout — multi-pane chart trader
// Styled with the editorial cream/dark system from the rest of the app.
// Layout: [DOM ladder | main chart | watchlist+options]
//         [mini chart + ticket | mini chart + ticket]
const { useState: aS, useEffect: aE, useMemo: aM } = React;

function ApexLayout({ activeRow, setActiveSym, D, dotmatrix, showVol, showMA, showBB }){
  const [tkt, setTkt] = aS(true);
  const [ctx, setCtx] = aS(null);
  const [tools, setTools] = aS(null);
  const [overlay, setOverlay] = aS(null);
  const [analysis, setAnalysis] = aS(false);

  function onChartCtx(e){
    e.preventDefault();
    setCtx({x: e.clientX, y: Math.min(e.clientY, window.innerHeight - 600)});
  }

  return (
    <div className="apex" data-screen-label="0X Apex Layout">
      <ApexToolbar activeRow={activeRow}
        onTools={(e)=>setTools({x:e.clientX-120, y:e.clientY+8})}
        onOverlay={(e)=>setOverlay({x:e.clientX-60, y:e.clientY+8})}
        onAnalysis={()=>setAnalysis(a=>!a)}
        onTicket={()=>setTkt(t=>!t)}/>
      <div className="apex-grid">
        <div className="apex-cell ladder">
          <ApexLadder ob={D.ob} last={activeRow.last}/>
        </div>
        <div className="apex-cell main-chart" onContextMenu={onChartCtx}>
          <ApexChart sym={activeRow.sym} candles={D.candles} row={activeRow}
                     showVol={showVol} showMA={showMA} showBB={showBB} dotmatrix={dotmatrix}
                     big timeframe="5m"/>
        </div>
        <div className="apex-cell rail">
          <ApexRail watchlist={D.watchlist} active={activeRow.sym} onSelect={setActiveSym}/>
        </div>
        <div className="apex-cell sub" style={{gridColumn:'1 / 3'}}>
          <ApexChart sym="AAPL" candles={shiftCandles(D.candles, 178, 0.62)}
                     row={D.watchlist.find(w=>w.sym==='AAPL')||{sym:'AAPL',name:'Apple',last:178.42,chg:1.18,pct:0.66}}
                     showVol={showVol} showMA={showMA} timeframe="15m"/>
        </div>
        <div className="apex-cell sub" style={{gridColumn:'3 / 4'}}>
          <ApexChart sym="SPY" candles={shiftCandles(D.candles, 528, 0.30)}
                     row={D.watchlist.find(w=>w.sym==='SPY')||{sym:'SPY',name:'S&P 500',last:528.61,chg:-1.94,pct:-0.36}}
                     showVol={showVol} showMA={showMA} timeframe="30m"/>
        </div>
      </div>
      {tkt && <ApexFloatingTicket sym={activeRow.sym} last={activeRow.last} onClose={()=>setTkt(false)}/>}
      {ctx && <ApexContextMenu x={ctx.x} y={ctx.y} price={activeRow.last.toFixed(2)} onClose={()=>setCtx(null)}/>}
      {tools && <ApexToolsPopover x={tools.x} y={tools.y} onClose={()=>setTools(null)}/>}
      {overlay && <ApexOverlayDrop x={overlay.x} y={overlay.y} onClose={()=>setOverlay(null)}/>}
      {analysis && <ApexAnalysisSidebar onClose={()=>setAnalysis(false)}/>}
    </div>
  );
}

function shiftCandles(src, base, vol){
  // Re-base existing candles to a different price level so the sub-charts feel distinct.
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

// ────────────────────────────────────────────────────────────────────
// TOOLBAR — chart-trader density tools row
// ────────────────────────────────────────────────────────────────────
function ApexToolbar({ activeRow, onTools, onOverlay, onAnalysis, onTicket }){
  const tfs = ['1m','5m','15m','1h','3h','1d','1w','1mo','Range'];
  const [tf, setTf] = aS('5m');
  const [drop, setDrop] = aS(null); // {key, x, y}
  function openDrop(key, e, fn){
    if (fn) { fn(e); return; }
    const r = e.currentTarget.getBoundingClientRect();
    setDrop(d => d?.key === key ? null : { key, x: r.left, y: r.bottom + 6 });
  }
  const studies = [
    {l:'Vol',     k:'vol'},
    {l:'Overlay', k:'overlay', fn: onOverlay},
    {l:'Tools',   k:'tools',   fn: onTools},
    {l:'Studies', k:'studies'},
    {l:'Widgets', k:'widgets'},
    {l:'Default', k:'default'},
  ];
  const right = [
    {l:'Window',  d:'⊞', k:'window'},
    {l:'Signals', d:'◐', k:'signals', acc:true},
    {l:'Analysis',d:'≡', k:'analysis', fn: onAnalysis},
    {l:'Ticket',  d:'$', k:'ticket', on:true, fn: onTicket},
    {l:'Playback',d:'▷', k:'playback'},
    {l:'Feed',    d:'⛁', k:'feed'},
  ];
  return (
    <div className="apex-toolbar">
      <div className="grp tic">
        <span className="acc-dot"/>
        <span className="mono sym">{activeRow.sym}</span>
        <span className="mono name">· {activeRow.name||'NVIDIA Corp'}</span>
      </div>
      <div className="grp">
        <span className="lab">TF</span>
        <div className="seg-tf">
          {tfs.map(t=>(
            <span key={t} onClick={()=>setTf(t)} className={'seg-btn '+(t===tf?'on':'')}>{t}</span>
          ))}
        </div>
      </div>
      <div className="grp">
        {studies.map(s=>(
          <span key={s.l}
                className={'btn dd '+(drop?.key===s.k?'open':'')}
                onClick={(e)=>openDrop(s.k, e, s.fn)}>
            {s.l}<span className="caret">▾</span>
          </span>
        ))}
      </div>
      <div className="grp" style={{marginLeft:'auto'}}>
        {right.map(r=>(
          <span key={r.l} onClick={(e)=>r.fn ? r.fn(e) : null}
                className={'btn '+(r.on?'on ':'')+(r.acc?'acc ':'')}>
            <span style={{opacity:.6, marginRight:6, fontFamily:'JetBrains Mono', fontSize:10}}>{r.d}</span>{r.l}
          </span>
        ))}
      </div>
      {drop && <ApexToolbarDrop dropKey={drop.key} x={drop.x} y={drop.y} onClose={()=>setDrop(null)}/>}
    </div>
  );
}

// Generic dropdown for the toolbar buttons (Vol/Studies/Widgets/Default)
function ApexToolbarDrop({ dropKey:k, x, y, onClose }){
  aE(()=>{
    const cl = (e)=>{ if (!e.target.closest?.('.tb-drop') && !e.target.closest?.('.btn.dd')) onClose(); };
    setTimeout(()=>document.addEventListener('click', cl), 50);
    return ()=>document.removeEventListener('click', cl);
  }, []);
  const sets = {
    vol:    { ttl:'VOLUME · STUDIES', rows:[
      {ic:'▮', l:'Volume Bars', on:true},
      {ic:'⌇', l:'Volume Profile'},
      {ic:'≈', l:'VWAP', acc:true},
      {ic:'⊕', l:'Cumulative Delta'},
      {ic:'◐', l:'Buy/Sell Ratio'},
    ]},
    studies:{ ttl:'INDICATORS', rows:[
      {ic:'∿', l:'RSI 14',   on:true},
      {ic:'∿', l:'MACD'},
      {ic:'≈', l:'Bollinger Bands'},
      {ic:'≡', l:'Stochastic'},
      {ic:'⌇', l:'ATR'},
      {ic:'◯', l:'Ichimoku'},
      {ic:'⊕', l:'Add Custom…',  acc:true},
    ]},
    widgets:{ ttl:'CHART WIDGETS', rows:[
      {ic:'▭', l:'Mini Chart', on:true},
      {ic:'☐', l:'Heatmap'},
      {ic:'◌', l:'Order Flow Donut'},
      {ic:'⌗', l:'Liquidity Grid'},
      {ic:'≡', l:'Term Structure'},
      {ic:'$', l:'P&L Card'},
    ]},
    default:{ ttl:'TEMPLATES', rows:[
      {ic:'★', l:'Day Scalper',   on:true},
      {ic:'★', l:'Swing · 4H'},
      {ic:'★', l:'Options Vol Surface'},
      {ic:'★', l:'Macro Rates'},
      {ic:'⊕', l:'Save as Template…', acc:true},
    ]},
    window: { ttl:'WINDOW LAYOUT', rows:[
      {ic:'▦', l:'2×2 Grid',  on:true},
      {ic:'▤', l:'1×3 Stack'},
      {ic:'▥', l:'3×1 Row'},
      {ic:'▣', l:'Single'},
    ]},
    signals:{ ttl:'SIGNAL FEEDS', rows:[
      {ic:'◐', l:'Price Cross', on:true, acc:true},
      {ic:'◑', l:'Volume Surge'},
      {ic:'◒', l:'News Sentiment'},
      {ic:'◓', l:'Options Flow'},
    ]},
    playback:{ ttl:'BAR REPLAY', rows:[
      {ic:'▷', l:'Play'},
      {ic:'▷▷',l:'Step Forward'},
      {ic:'⇆', l:'Set Speed'},
      {ic:'⌫', l:'Reset to Live'},
    ]},
    feed:   { ttl:'DATA FEED', rows:[
      {ic:'⛁', l:'NASDAQ TotalView', on:true, acc:true},
      {ic:'⛁', l:'NYSE OpenBook'},
      {ic:'⛁', l:'Cboe One'},
      {ic:'⊞', l:'Add Provider…', acc:true},
    ]},
  };
  const s = sets[k];
  if (!s) return null;
  return (
    <div className="tb-drop" style={{left:x, top:y}} onClick={e=>e.stopPropagation()}>
      <div className="tb-drop-head mono">{s.ttl}</div>
      {s.rows.map((r,i)=>(
        <div key={i} className={'tb-drop-row '+(r.on?'on ':'')+(r.acc?'acc ':'')}>
          <span className="ic mono">{r.ic}</span>
          <span className="l">{r.l}</span>
          {r.on && <span className="check mono">✓</span>}
        </div>
      ))}
    </div>
  );
}

// ────────────────────────────────────────────────────────────────────
// DOM LADDER — bid/ask price ladder, left rail
// ────────────────────────────────────────────────────────────────────
function ApexLadder({ ob, last }){
  // Build a centered ladder of prices around `last`.
  const tick = 0.05;
  const N = 32; // rows above + rows below
  const rows = [];
  for (let i=N; i>=-N; i--){
    const p = +(last + i*tick).toFixed(2);
    const isAsk = i > 0;
    const isBid = i < 0;
    const isLast = i === 0;
    // synthesize sizes from order book by distance
    const dist = Math.abs(i);
    const seed = Math.sin(p*13.7)+Math.cos(p*7.2);
    const sz = Math.max(0, Math.round((1.4 - dist*0.04) * (220 + seed*120)));
    rows.push({ p, isAsk, isBid, isLast, sz, dist });
  }
  // Heat for inside spread
  const maxSz = Math.max(...rows.map(r=>r.sz));
  return (
    <div className="ladder-wrap">
      <div className="ladder-head">
        <span>BID</span><span>PRICE</span><span>ASK</span>
      </div>
      <div className="ladder-body">
        {rows.map((r,i)=>{
          const w = r.sz / maxSz * 100;
          return (
            <div key={i} className={'ladder-row '+(r.isLast?'last ':'')+(r.isBid?'bid ':'')+(r.isAsk?'ask ':'')}>
              <div className="bid-cell">
                {r.isBid && <>
                  <div className="bar" style={{width:`${w}%`}}/>
                  <span className="mono num">{r.sz}</span>
                </>}
              </div>
              <div className={'price-cell mono '+(r.isLast?'tag':'')}>
                {r.p.toFixed(2)}
              </div>
              <div className="ask-cell">
                {r.isAsk && <>
                  <div className="bar" style={{width:`${w}%`}}/>
                  <span className="mono num">{r.sz}</span>
                </>}
              </div>
            </div>
          );
        })}
      </div>
      <div className="ladder-foot">
        <span className="btn buy mono">BUY · MKT</span>
        <span className="btn sell mono">SELL · MKT</span>
      </div>
    </div>
  );
}

// ────────────────────────────────────────────────────────────────────
// MAIN CHART CELL (also used for sub-charts)
// ────────────────────────────────────────────────────────────────────
function ApexChart({ sym, candles, row, showVol, showMA, showBB, big, timeframe='5m', dotmatrix }){
  const last = candles[candles.length-1];
  const chg = last.c - candles[0].c;
  const pct = chg / candles[0].c * 100;
  return (
    <div className="apex-chart">
      <div className="apex-chart-head">
        <div className="lhs">
          <span className="acc-dot"/>
          <span className="mono sym">{sym}</span>
          <span className="mono tf">· {timeframe}</span>
          <span className="mono ohlc">
            O <b>{last.o.toFixed(2)}</b>
            &nbsp;H <b>{last.h.toFixed(2)}</b>
            &nbsp;L <b>{last.l.toFixed(2)}</b>
            &nbsp;C <b>{last.c.toFixed(2)}</b>
          </span>
          <span className={'mono chg '+(chg>=0?'up':'down')}>
            {chg>=0?'+':''}{chg.toFixed(2)}  ({chg>=0?'+':''}{pct.toFixed(2)}%)
          </span>
        </div>
        <div className="rhs">
          <span className="mono lab">VOL <b>{(last.v/1e6).toFixed(2)}M</b></span>
        </div>
      </div>
      <div className="apex-chart-body">
        <ApexCandles candles={candles} showVol={showVol} showMA={showMA} showBB={showBB} big={big}/>
      </div>
      {!big && <ApexMiniTicket sym={sym} last={last.c}/>}
    </div>
  );
}

// Lightweight candles rendered for the apex layout — uses the existing
// editorial palette but sized to fill its parent and includes trendlines
// and a mouse-tracking crosshair tooltip.
function ApexCandles({ candles, showVol=true, showMA=true, showBB=false, big }){
  const W = 1200, H = big ? 520 : 360;
  const padL = 8, padR = 64, padT = 18, padB = showVol ? 96 : 28;
  const innerW = W - padL - padR;
  const innerH = H - padT - padB;

  const wrapRef = React.useRef(null);
  const [hover, setHover] = aS(null); // {i, c, mx, my, sx, sy, price}

  const highs = candles.map(c=>c.h);
  const lows  = candles.map(c=>c.l);
  const max = Math.max(...highs);
  const min = Math.min(...lows);
  const span = max - min || 1;
  const x = i => padL + (i / (candles.length-1)) * innerW;
  const y = v => padT + (1 - (v - min) / span) * innerH;
  const cw = Math.max(2, innerW / candles.length * 0.62);

  // moving averages
  const ma = (n)=> candles.map((_,i)=>{
    const s = Math.max(0, i-n+1);
    const slice = candles.slice(s, i+1);
    return slice.reduce((a,b)=>a+b.c,0)/slice.length;
  });
  const ma20 = aM(()=>ma(20),[candles]);
  const ma50 = aM(()=>ma(50),[candles]);

  // volume zone
  const volMax = Math.max(...candles.map(c=>c.v));
  const volTop = H - padB + 18;
  const volH = padB - 24;

  // last candle for "callout" overlay
  const lastIdx = candles.length-1;
  const last = candles[lastIdx];

  // Trendlines — fit a falling and rising line through visual highs/lows
  // similar to the reference screenshot.
  const tlHi = { x1: x(2), y1: y(max - span*0.05), x2: x(Math.floor(candles.length*0.55)), y2: y(min + span*0.45) };
  const tlLo = { x1: x(Math.floor(candles.length*0.18)), y1: y(min + span*0.10), x2: x(lastIdx-1), y2: y(min + span*0.62) };

  // Mouse tracking — convert client coords to candle index + price
  function onMove(e){
    const rect = wrapRef.current.getBoundingClientRect();
    const px = (e.clientX - rect.left) / rect.width * W;
    const py = (e.clientY - rect.top) / rect.height * H;
    const i = Math.max(0, Math.min(candles.length-1, Math.round(((px - padL) / innerW) * (candles.length-1))));
    const price = max - ((py - padT) / innerH) * span;
    setHover({ i, c: candles[i], mx: e.clientX - rect.left, my: e.clientY - rect.top, sx: x(i), sy: py, price });
  }
  function onLeave(){ setHover(null); }

  return (
    <div ref={wrapRef} style={{position:'relative', width:'100%', height:'100%'}}
         onMouseMove={onMove} onMouseLeave={onLeave}>
    <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" style={{width:'100%', height:'100%', display:'block'}}>
      {/* horizontal grid — dashed hairlines */}
      {[0.2,0.4,0.6,0.8].map(p=>(
        <line key={p} x1={padL} x2={W-padR} y1={padT+innerH*p} y2={padT+innerH*p}
              stroke="var(--hair)" strokeDasharray="2 4"/>
      ))}
      {/* y-axis labels */}
      {[0,0.25,0.5,0.75,1].map(p=>{
        const v = max - span*p;
        return <text key={p} x={W-padR+6} y={padT+innerH*p+4} fontSize="10" fontFamily="JetBrains Mono" fill="var(--muted)">{v.toFixed(2)}</text>;
      })}

      {/* trendlines */}
      <line {...tlHi} stroke="var(--accent)" strokeWidth="1" strokeDasharray="0" opacity="0.85"/>
      <line {...tlLo} stroke="var(--accent)" strokeWidth="1" opacity="0.85"/>

      {/* candles */}
      {candles.map((c,i)=>{
        const cx = x(i);
        const isUp = c.c >= c.o;
        const yo = y(c.o), yc = y(c.c);
        const yh = y(c.h), yl = y(c.l);
        const top = Math.min(yo,yc);
        const bh = Math.max(1.2, Math.abs(yc-yo));
        return (
          <g key={i}>
            <line x1={cx} x2={cx} y1={yh} y2={yl} stroke={isUp?'var(--up)':'var(--down)'} strokeWidth="1"/>
            <rect x={cx-cw/2} y={top} width={cw} height={bh}
                  fill={isUp?'var(--up)':'var(--down)'}/>
          </g>
        );
      })}

      {/* moving avg */}
      {showMA && (
        <>
          <path d={ma20.map((v,i)=> (i?'L':'M')+x(i).toFixed(1)+' '+y(v).toFixed(1)).join(' ')}
                fill="none" stroke="var(--accent)" strokeWidth="1.4" opacity="0.9"/>
          <path d={ma50.map((v,i)=> (i?'L':'M')+x(i).toFixed(1)+' '+y(v).toFixed(1)).join(' ')}
                fill="none" stroke="var(--ink)" strokeWidth="1" opacity="0.55" strokeDasharray="3 3"/>
        </>
      )}

      {/* volume */}
      {showVol && candles.map((c,i)=>{
        const h = (c.v / volMax) * volH;
        const isUp = c.c >= c.o;
        return <rect key={'v'+i} x={x(i)-cw/2} y={volTop+volH-h} width={cw} height={h}
                     fill={isUp?'var(--up)':'var(--down)'} opacity="0.55"/>;
      })}

      {/* last price tag */}
      <g>
        <line x1={padL} x2={W-padR} y1={y(last.c)} y2={y(last.c)} stroke="var(--accent)" strokeWidth="0.8" strokeDasharray="2 3" opacity="0.7"/>
        <rect x={W-padR+2} y={y(last.c)-9} width={56} height={18} fill="var(--accent)"/>
        <text x={W-padR+30} y={y(last.c)+4} fontSize="11" fontFamily="JetBrains Mono" fontWeight="600" fill="#fff" textAnchor="middle">{last.c.toFixed(2)}</text>
      </g>

      {/* editorial callout removed in favor of mouse-following tooltip */}

      {/* crosshair */}
      {hover && (
        <g pointerEvents="none">
          <line x1={hover.sx} x2={hover.sx} y1={padT} y2={padT+innerH}
                stroke="var(--ink)" strokeWidth="0.6" strokeDasharray="2 3" opacity="0.45"/>
          <line x1={padL} x2={W-padR} y1={hover.sy} y2={hover.sy}
                stroke="var(--ink)" strokeWidth="0.6" strokeDasharray="2 3" opacity="0.45"/>
          <rect x={W-padR+2} y={hover.sy-9} width={56} height={18} fill="var(--ink)"/>
          <text x={W-padR+30} y={hover.sy+4} fontSize="11" fontFamily="JetBrains Mono" fontWeight="600" fill="var(--bg)" textAnchor="middle">
            {hover.price.toFixed(2)}
          </text>
        </g>
      )}
    </svg>

    {/* mouse-following data tooltip */}
    {hover && (
      <div className="apex-tt" style={{
        left: Math.min(hover.mx + 14, (wrapRef.current?.getBoundingClientRect().width || 9999) - 220),
        top:  Math.max(8, hover.my - 92),
      }}>
        <div className="tt-row tt-head">
          <span className="tt-lab">{(()=>{ const d = new Date(hover.c.t); return isFinite(d) ? d.toISOString().slice(11,16) : '--:--'; })()} · UTC</span>
          <span className={'tt-ch '+(hover.c.c >= hover.c.o ? 'up':'down')}>
            {hover.c.c >= hover.c.o ? '+' : ''}{(hover.c.c - hover.c.o).toFixed(2)}
          </span>
        </div>
        <div className="tt-grid">
          <div><span className="tt-lab">O</span><span className="tt-v">{hover.c.o.toFixed(2)}</span></div>
          <div><span className="tt-lab">H</span><span className="tt-v up">{hover.c.h.toFixed(2)}</span></div>
          <div><span className="tt-lab">L</span><span className="tt-v down">{hover.c.l.toFixed(2)}</span></div>
          <div><span className="tt-lab">C</span><span className="tt-v">{hover.c.c.toFixed(2)}</span></div>
        </div>
        <div className="tt-row">
          <span className="tt-lab">VOL</span>
          <span className="tt-v">{(hover.c.v/1e6).toFixed(2)}M</span>
          <span className="tt-lab" style={{marginLeft:10}}>VWAP</span>
          <span className="tt-v">{((hover.c.h+hover.c.l+hover.c.c)/3).toFixed(2)}</span>
        </div>
      </div>
    )}
    </div>
  );
}

// Tiny ticket attached to sub-charts
function ApexMiniTicket({ sym, last }){
  return (
    <div className="apex-mini-ticket">
      <div className="grp">
        <span className="lab">QTY</span>
        <span className="mono v">100</span>
      </div>
      <div className="grp">
        <span className="lab">@</span>
        <span className="mono v">{last.toFixed(2)}</span>
      </div>
      <div className="btn buy mono">BUY {Math.round(100*last/1000*10)/10}K</div>
      <div className="btn sell mono">SELL {Math.round(100*last/1000*10)/10}K</div>
    </div>
  );
}

// ────────────────────────────────────────────────────────────────────
// RIGHT RAIL — watchlist heat + options strip
// ────────────────────────────────────────────────────────────────────
function ApexRail({ watchlist, active, onSelect }){
  return (
    <div className="apex-rail">
      <div className="rail-head">
        <div className="ttl">
          <span className="num mono">A</span>
          <span className="nm">List · Chain · Heat</span>
        </div>
        <span className="mono add">+ ADD</span>
      </div>
      <div className="rail-search">
        <span className="mono">▢</span>
        <span className="mono p">Add symbol…</span>
      </div>
      <div className="rail-list">
        {watchlist.slice(0,11).map(w=>(
          <div key={w.sym} className={'rail-row '+(w.sym===active?'on':'')} onClick={()=>onSelect(w.sym)}>
            <span className="mono sym">{w.sym}</span>
            <span className={'mono pct '+(w.pct>=0?'up':'down')}>
              {w.pct>=0?'+':''}{w.pct.toFixed(2)}%
            </span>
            <span className="mono px">{w.last.toFixed(2)}</span>
          </div>
        ))}
      </div>
      <div className="rail-section">
        <div className="rail-shead">
          <span className="num mono">B</span>
          <span className="nm">Options · {active}</span>
        </div>
        <div className="rail-opt">
          {[
            {k:'C', tag:'SPY 716 00TE', a:'2.97', b:'2.79'},
            {k:'C', tag:'SPY 715 00TE', a:'3.44', b:'3.59'},
            {k:'P', tag:'SPY 711 00TE', a:'3.10', b:'3.18'},
            {k:'P', tag:'SPY 709 00TE', a:'3.32', b:'3.40'},
          ].map((o,i)=>(
            <div key={i} className="opt-row">
              <span className={'k mono '+(o.k==='C'?'up':'down')}>{o.k}</span>
              <span className="mono name">{o.tag}</span>
              <span className="mono v">{o.a} <span className="muted">×</span> {o.b}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

// ────────────────────────────────────────────────────────────────────
// ORDER TICKET — strip beneath the main chart
// ────────────────────────────────────────────────────────────────────
function ApexOrderTicket({ sym, last }){
  const [side, setSide] = aS(null);
  const [type, setType] = aS('LMT');
  const [qty, setQty]   = aS(100);
  const [px, setPx]     = aS(last.toFixed(2));
  const bid = (last - 0.05).toFixed(2);
  const ask = (last + 0.05).toFixed(2);
  const notional = (qty * parseFloat(px||0));
  return (
    <div className="apex-ticket">
      <div className="seg">
        <span className="lab mono">SYM</span>
        <span className="mono v">{sym}</span>
      </div>
      <div className="seg">
        <span className="lab mono">TYPE</span>
        {['MKT','LMT','STP'].map(t=>(
          <span key={t} onClick={()=>setType(t)}
                className={'pill mono '+(t===type?'on':'')}>{t}</span>
        ))}
      </div>
      <div className="seg">
        <span className="lab mono">QTY</span>
        <input className="mono in" value={qty} onChange={e=>setQty(parseInt(e.target.value)||0)}/>
        <div className="step">
          <span onClick={()=>setQty(Math.max(0,qty-100))}>−</span>
          <span onClick={()=>setQty(qty+100)}>+</span>
        </div>
      </div>
      {type !== 'MKT' && (
        <div className="seg">
          <span className="lab mono">PX</span>
          <input className="mono in wide" value={px} onChange={e=>setPx(e.target.value)}/>
          <div className="step">
            <span onClick={()=>setPx((parseFloat(px)-0.05).toFixed(2))}>−</span>
            <span onClick={()=>setPx((parseFloat(px)+0.05).toFixed(2))}>+</span>
          </div>
        </div>
      )}
      <div className="seg quote">
        <span className="lab mono">BID</span><span className="mono up">{bid}</span>
        <span className="lab mono" style={{marginLeft:10}}>ASK</span><span className="mono down">{ask}</span>
        <span className="lab mono" style={{marginLeft:10}}>NOT.</span><span className="mono">${(notional/1000).toFixed(1)}K</span>
      </div>
      <div onClick={()=>setSide('buy')}  className={'tbtn buy mono '+(side==='buy'?'on':'')}>BUY {qty}</div>
      <div onClick={()=>setSide('sell')} className={'tbtn sell mono '+(side==='sell'?'on':'')}>SELL {qty}</div>
    </div>
  );
}

window.ApexLayout = ApexLayout;
