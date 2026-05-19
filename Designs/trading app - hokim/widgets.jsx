// Widget components for trading terminal
const { useState: useS, useEffect: useE, useRef: useR, useMemo: useM } = React;

function fmt(n, d=2){ return n.toLocaleString('en-US',{minimumFractionDigits:d, maximumFractionDigits:d}); }
function fmtSign(n, d=2){ const s=n>=0?'+':''; return s+n.toLocaleString('en-US',{minimumFractionDigits:d, maximumFractionDigits:d}); }

/* ----- Watchlist ----- */
function Watchlist({ data, active, onSelect }) {
  return (
    <div className="wl">
      <div className="wl-head">
        <div>SYM</div><div>NAME</div><div className="r-num">LAST</div><div className="r-num">CHG%</div>
      </div>
      {data.map(r => (
        <div key={r.sym} className={`wl-row ${active===r.sym?'on':''}`} onClick={()=>onSelect&&onSelect(r.sym)}>
          <div className="sym">{r.sym}</div>
          <div style={{color:'var(--muted)', overflow:'hidden', textOverflow:'ellipsis', whiteSpace:'nowrap'}}>{r.name}</div>
          <div className="r-num">{fmt(r.last)}</div>
          <div className={'r-num ' + (r.pct>=0?'up':'down')}>{fmtSign(r.pct)}%</div>
        </div>
      ))}
    </div>
  );
}

/* ----- Order book ----- */
function OrderBook({ ob }) {
  const maxTotal = Math.max(ob.bids[ob.bids.length-1].total, ob.asks[ob.asks.length-1].total);
  return (
    <div className="ob">
      <div className="ob-head"><div>PRICE</div><div>SIZE</div><div>TOTAL</div></div>
      <div style={{display:'flex', flexDirection:'column-reverse'}}>
        {ob.asks.map((r,i) => (
          <div key={'a'+i} className="ob-row ask">
            <div className="bar" style={{width: (r.total/maxTotal*100)+'%'}}/>
            <div className="down">{r.px.toFixed(2)}</div>
            <div>{r.sz}</div>
            <div style={{color:'var(--muted)'}}>{r.total}</div>
          </div>
        ))}
      </div>
      <div className="ob-spread">
        <div>
          <div className="label">MID</div>
          <div className="mid mono">{ob.mid.toFixed(2)}</div>
        </div>
        <div style={{textAlign:'right'}}>
          <div className="label">SPREAD</div>
          <div className="mono" style={{fontSize:14}}>0.05 / 0.004%</div>
        </div>
      </div>
      <div>
        {ob.bids.map((r,i) => (
          <div key={'b'+i} className="ob-row bid">
            <div className="bar" style={{width: (r.total/maxTotal*100)+'%'}}/>
            <div className="up">{r.px.toFixed(2)}</div>
            <div>{r.sz}</div>
            <div style={{color:'var(--muted)'}}>{r.total}</div>
          </div>
        ))}
      </div>
    </div>
  );
}

/* ----- Order ticket ----- */
function OrderTicket({ symbol='NVDA', last=1284.32 }) {
  const [side, setSide] = useS('buy');
  const [type, setType] = useS('LMT');
  const [tif, setTif] = useS('DAY');
  const [qty, setQty] = useS(100);
  const [px,  setPx]  = useS(last.toFixed(2));
  const notional = (qty * parseFloat(px||0));
  return (
    <div className="ticket">
      <div className="seg">
        <button className={'buy '+(side==='buy'?'on':'')} onClick={()=>setSide('buy')}>Buy</button>
        <button className={'sell '+(side==='sell'?'on':'')} onClick={()=>setSide('sell')}>Sell</button>
      </div>

      <div className="field">
        <label>Symbol</label>
        <div className="input">
          <span style={{fontWeight:600}}>{symbol}</span>
          <span style={{color:'var(--muted)', fontSize:11}}>NASDAQ · USD</span>
        </div>
      </div>

      <div className="row2">
        <div className="field">
          <label>Order Type</label>
          <div className="input">
            <span>{type}</span>
            <span style={{color:'var(--muted)', fontSize:11}}>▾</span>
          </div>
        </div>
        <div className="field">
          <label>Quantity</label>
          <div className="input">
            <span>{qty.toLocaleString()}</span>
            <div className="stepper"><span onClick={()=>setQty(Math.max(0,qty-100))}>−</span><span onClick={()=>setQty(qty+100)}>+</span></div>
          </div>
        </div>
      </div>

      <div className="field">
        <label>Limit Price</label>
        <div className="input">
          <span>{px}</span>
          <div className="stepper">
            <span onClick={()=>setPx((parseFloat(px)-0.05).toFixed(2))}>−</span>
            <span onClick={()=>setPx((parseFloat(px)+0.05).toFixed(2))}>+</span>
          </div>
        </div>
      </div>

      <div>
        <label className="label" style={{display:'block', marginBottom:6}}>Time in force</label>
        <div className="tif">
          {['DAY','GTC','IOC'].map(t=>(
            <div key={t} className={tif===t?'on':''} onClick={()=>setTif(t)}>{t}</div>
          ))}
        </div>
      </div>

      <div className="summary">
        <div className="k">Estimated cost</div><div className="mono">${fmt(notional)}</div>
        <div className="k">Buying power</div><div className="mono">$2,418,294.10</div>
        <div className="k">Commission</div><div className="mono">$0.00</div>
        <div className="k">Margin req.</div><div className="mono">${fmt(notional*0.5)}</div>
      </div>

      <button className={`btn-submit ${side}`}>
        Review {side==='buy'?'Buy':'Sell'} · {qty} @ {px}
      </button>
    </div>
  );
}

/* ----- Heatmap ----- */
function Heatmap({ data }) {
  // Compute layout: each section is a row, items split horizontally by weight
  return (
    <div className="heatmap">
      {data.map(sec => (
        <div key={sec.sec} className="sec" style={{flex: sec.w}}>
          <div className="sec-head">{sec.sec} <span style={{opacity:.6}}>· {sec.w}%</span></div>
          <div className="tiles" style={{
            gridTemplateColumns: sec.items.map(i=>`${i.w}fr`).join(' '),
            height: `${sec.w*4 + 28}px`,
          }}>
            {sec.items.map(it => {
              const intensity = Math.min(1, Math.abs(it.p)/3);
              const bg = it.p>=0
                ? `color-mix(in srgb, var(--up) ${20+intensity*70}%, var(--bg-2))`
                : `color-mix(in srgb, var(--down) ${20+intensity*70}%, var(--bg-2))`;
              return (
                <div key={it.s} className="htile" style={{background:bg}}>
                  <div className="s">{it.s}</div>
                  <div className="p">{fmtSign(it.p)}%</div>
                </div>
              );
            })}
          </div>
        </div>
      ))}
    </div>
  );
}

/* ----- News ----- */
function News({ items }) {
  return (
    <div>
      {items.map((n,i) => (
        <div key={i} className={'news-row '+n.tone}>
          <div className="t">{n.t}</div>
          <div className="src">{n.src}</div>
          <div className="h">{n.head}</div>
          <div className="tag">{n.tag}</div>
        </div>
      ))}
    </div>
  );
}

/* ----- Tape ----- */
function Tape({ rows }) {
  return (
    <div className="tape">
      <div className="head">TIME</div>
      <div className="head r-num">PRICE</div>
      <div className="head r-num">SIZE</div>
      <div className="head"></div>
      {rows.map((r,i) => (
        <React.Fragment key={i}>
          <div style={{color:'var(--muted)'}}>{r.time}</div>
          <div className={'r-num '+(r.side==='B'?'up':'down')}>{r.px.toFixed(2)}</div>
          <div className="r-num">{r.sz}</div>
          <div className={r.side==='B'?'side-b':'side-s'}>{r.side}</div>
        </React.Fragment>
      ))}
    </div>
  );
}

/* ----- Positions ----- */
function Positions({ rows }) {
  const total = rows.reduce((a,b)=>a+b.pl,0);
  const totalMv = rows.reduce((a,b)=>a+b.mv,0);
  return (
    <div style={{display:'flex', flexDirection:'column', height:'100%'}}>
      <div style={{display:'grid', gridTemplateColumns:'repeat(4,1fr)', borderBottom:'1px solid var(--hair)'}}>
        <div style={{padding:'14px 16px', borderRight:'1px solid var(--hair)'}}>
          <div className="label">Net P&L (today)</div>
          <div className="midnum mono" style={{color:'var(--up)'}}>{fmtSign(total/1000,1)}K</div>
        </div>
        <div style={{padding:'14px 16px', borderRight:'1px solid var(--hair)'}}>
          <div className="label">Market value</div>
          <div className="midnum mono">${fmt(totalMv/1e6,2)}M</div>
        </div>
        <div style={{padding:'14px 16px', borderRight:'1px solid var(--hair)'}}>
          <div className="label">Cash</div>
          <div className="midnum mono">$2.42M</div>
        </div>
        <div style={{padding:'14px 16px'}}>
          <div className="label">Buying power</div>
          <div className="midnum mono">$5.83M</div>
        </div>
      </div>
      <div className="scroll" style={{flex:1}}>
        <div className="pos">
          <div className="head">SYM</div>
          <div className="head">SIDE</div>
          <div className="head r-num">QTY</div>
          <div className="head r-num">AVG</div>
          <div className="head r-num">MARKET VALUE</div>
          <div className="head r-num">P&L</div>
          {rows.map((r,i)=>(
            <React.Fragment key={i}>
              <div className="cell" style={{fontWeight:600}}>{r.sym}</div>
              <div className="cell" style={{color: r.side==='LONG'?'var(--up)':'var(--down)'}}>{r.side}</div>
              <div className="cell r-num">{r.qty.toLocaleString()}</div>
              <div className="cell r-num">{fmt(r.avg)}</div>
              <div className="cell r-num">${fmt(r.mv,0)}</div>
              <div className={'cell r-num '+(r.pl>=0?'up':'down')}>{fmtSign(r.pl,0)} · {fmtSign(r.plp)}%</div>
            </React.Fragment>
          ))}
        </div>
      </div>
    </div>
  );
}

/* ----- Indices ticker tape ----- */
function TickerTape({ items }) {
  // duplicate for seamless loop
  const all = [...items, ...items, ...items];
  return (
    <div className="ticker">
      <div className="ticker-track">
        {all.map((it,i)=>(
          <div key={i} className="tick-item">
            <span className="nm">{it.n}</span>
            <span className="v">{it.lvl}</span>
            <span className={it.c>=0?'up':'down'}>{fmtSign(it.c)}%</span>
          </div>
        ))}
      </div>
    </div>
  );
}

/* ----- Hero (oversized symbol info) ----- */
function Hero({ row, candles }) {
  const last = candles[candles.length-1].c;
  return (
    <div className="hero">
      <div className="hero-cell">
        <div className="sym-row">
          <div className="sym">{row.sym}</div>
          <div className="name">{row.name} · NASDAQ</div>
        </div>
        <div className="mega mono">{fmt(last,2)}</div>
        <div className="pricewrap" style={{marginTop:8}}>
          <div className={'delta '+(row.chg>=0?'up':'down')}>{fmtSign(row.chg,2)} · {fmtSign(row.pct)}%</div>
          <div className="label">vs prev close</div>
        </div>
      </div>
      <div className="hero-cell">
        <div style={{display:'grid', gridTemplateRows:'auto 1fr', height:'100%'}}>
          <div className="label" style={{marginBottom:8}}>1D · INTRADAY</div>
          <div style={{minHeight:0}}><HeroSpark candles={candles.slice(-60)} /></div>
        </div>
      </div>
      <div className="hero-cell">
        <div className="stat-grid">
          <div className="stat"><div className="k">OPEN</div><div className="v">{fmt(candles[candles.length-1].o)}</div></div>
          <div className="stat"><div className="k">HIGH</div><div className="v">{fmt(candles[candles.length-1].h)}</div></div>
          <div className="stat"><div className="k">LOW</div><div className="v">{fmt(candles[candles.length-1].l)}</div></div>
          <div className="stat"><div className="k">VOL</div><div className="v">{row.vol}</div></div>
          <div className="stat"><div className="k">52W H</div><div className="v">1,348.55</div></div>
          <div className="stat"><div className="k">52W L</div><div className="v">718.10</div></div>
          <div className="stat"><div className="k">MKT CAP</div><div className="v">{row.mcap}</div></div>
          <div className="stat"><div className="k">P/E</div><div className="v">68.2</div></div>
        </div>
      </div>
    </div>
  );
}

/* ----- Options chain ----- */
function OptionsChain({ rows, spot }) {
  return (
    <div className="opts">
      <div className="head">
        <div>BID</div><div>ASK</div><div>VOL</div><div>OI</div>
        <div className="strike" style={{textAlign:'center'}}>STRIKE</div>
        <div>BID</div><div>ASK</div><div>VOL</div><div>OI</div>
      </div>
      <div style={{textAlign:'center', fontSize:9, padding:'4px', color:'var(--muted)', letterSpacing:'.12em', display:'grid', gridTemplateColumns:'1fr 1fr', borderBottom:'1px solid var(--hair)'}}>
        <div>◀ CALLS</div><div>PUTS ▶</div>
      </div>
      {rows.map(r => {
        const itmC = r.k < spot, itmP = r.k > spot;
        return (
          <div key={r.k} className={'row ' + (itmC?'itm-c ':'') + (itmP?'itm-p':'')}>
            <div className="c-bid up">{r.cBid}</div>
            <div className="c-ask">{r.cAsk}</div>
            <div style={{color:'var(--muted)'}}>{r.cVol.toLocaleString()}</div>
            <div style={{color:'var(--muted)'}}>{r.cOI.toLocaleString()}</div>
            <div className="strike">{r.k}</div>
            <div className="p-bid down">{r.pBid}</div>
            <div className="p-ask">{r.pAsk}</div>
            <div style={{color:'var(--muted)'}}>{r.pVol.toLocaleString()}</div>
            <div style={{color:'var(--muted)'}}>{r.pOI.toLocaleString()}</div>
          </div>
        );
      })}
    </div>
  );
}

/* ----- Command palette ----- */
function CommandPalette({ open, onClose, onPick }) {
  const [q, setQ] = useS('');
  const [sel, setSel] = useS(0);
  const items = useM(()=>{
    const all = [
      ...window.TERMINAL_DATA.watchlist.map(w=>({type:'EQUITY', name:w.sym, meta:w.name, action:()=>onPick({kind:'symbol',sym:w.sym})})),
      {type:'CMD', name:'TOP', meta:'Top movers',         action:()=>onPick({kind:'page',page:'top'})},
      {type:'CMD', name:'GIP', meta:'Global indices',     action:()=>onPick({kind:'page',page:'gip'})},
      {type:'CMD', name:'PORT',meta:'Portfolio overview', action:()=>onPick({kind:'page',page:'port'})},
      {type:'CMD', name:'NEWS',meta:'News headlines',     action:()=>onPick({kind:'page',page:'news'})},
      {type:'CMD', name:'OMON',meta:'Options monitor',    action:()=>onPick({kind:'page',page:'omon'})},
      {type:'NAV', name:'CHART',meta:'Open chart view',   action:()=>onPick({kind:'route',route:'chart'})},
      {type:'NAV', name:'DASH',meta:'Open dashboard',     action:()=>onPick({kind:'route',route:'dash'})},
      {type:'NAV', name:'TRADE',meta:'Open trade workstation',action:()=>onPick({kind:'route',route:'trade'})},
      {type:'NAV', name:'TILES',meta:'Open bold tiles dashboard',action:()=>onPick({kind:'route',route:'tiles'})},
    ];
    if (!q.trim()) return all.slice(0,12);
    const Q = q.toUpperCase();
    return all.filter(a => a.name.toUpperCase().includes(Q) || a.meta.toUpperCase().includes(Q)).slice(0,14);
  }, [q]);

  useE(()=>{ setSel(0); }, [q]);
  useE(()=>{
    if (!open) return;
    function k(e){
      if (e.key==='Escape') { onClose(); }
      if (e.key==='ArrowDown') { setSel(s=>Math.min(items.length-1, s+1)); e.preventDefault(); }
      if (e.key==='ArrowUp')   { setSel(s=>Math.max(0, s-1)); e.preventDefault(); }
      if (e.key==='Enter')     { items[sel] && items[sel].action(); onClose(); }
    }
    window.addEventListener('keydown', k);
    return ()=>window.removeEventListener('keydown', k);
  }, [open, items, sel]);

  if (!open) return null;
  return (
    <div className="cmd-overlay" onClick={onClose}>
      <div className="cmd" onClick={e=>e.stopPropagation()}>
        <div className="cmd-input">
          <span className="gt">GO</span>
          <input autoFocus placeholder="Type a symbol or function… (e.g. NVDA, NEWS, PORT)"
                 value={q} onChange={e=>setQ(e.target.value)} />
          <span style={{fontFamily:'JetBrains Mono', color:'var(--muted)', fontSize:10, letterSpacing:'.12em'}}>ESC TO CLOSE</span>
        </div>
        <div className="cmd-list">
          {items.map((it,i)=>(
            <div key={i} className={'cmd-item '+(i===sel?'on':'')}
                 onMouseEnter={()=>setSel(i)}
                 onClick={()=>{ it.action(); onClose(); }}>
              <div className="typ">{it.type}</div>
              <div className="nm">{it.name}</div>
              <div className="meta">{it.meta}</div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

/* ----- Panel wrapper ----- */
function Panel({ num, title, sub, actions, children, style, noScroll }) {
  return (
    <div className="panel" style={style}>
      <div className="panel-head">
        <div className="ttl">
          <span className="num">{num}</span>
          <span className="nm">{title}</span>
          {sub && <span className="label" style={{marginLeft:6}}>{sub}</span>}
        </div>
        <div className="actions">
          {actions || (
            <>
              <button className="icon-btn" title="expand"><svg width="11" height="11" viewBox="0 0 11 11"><path d="M1 1h3M1 1v3M10 1H7M10 1v3M1 10h3M1 10v-3M10 10H7M10 10v-3" stroke="currentColor" fill="none"/></svg></button>
              <button className="icon-btn">⋯</button>
            </>
          )}
        </div>
      </div>
      <div className={'body ' + (noScroll ? 'no-scroll' : '')}>{children}</div>
    </div>
  );
}

Object.assign(window, {
  Watchlist, OrderBook, OrderTicket, Heatmap, News, Tape, Positions,
  TickerTape, Hero, OptionsChain, CommandPalette, Panel, fmt, fmtSign
});
