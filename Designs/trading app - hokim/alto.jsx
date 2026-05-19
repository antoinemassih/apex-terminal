// Alto — editorial-print alternative direction
// Three screens: The Folio, The Reading Room, The Book

const { useState: alS, useEffect: alE, useMemo: alM, useRef: alR } = React;

function AltoApp({ D, route='folio', setRoute, activeSym, setActiveSym }) {
  const activeRow = D.watchlist.find(w => w.sym === activeSym) || D.watchlist[0];
  return (
    <div className="alto-root" data-screen-label="ALTO root">
      <AltoMasthead activeRow={activeRow} />
      <AltoNav route={route} setRoute={setRoute} />
      {route === 'folio'   && <Folio D={D} activeRow={activeRow} setActiveSym={setActiveSym} />}
      {route === 'reading' && <ReadingRoom D={D} activeRow={activeRow} setActiveSym={setActiveSym} />}
      {route === 'book'    && <Book D={D} />}
      <AltoTape items={D.indices} />
    </div>
  );
}

function AltoMasthead({ activeRow }) {
  const now = new Date();
  const date = now.toLocaleDateString('en-US', { weekday:'long', month:'long', day:'numeric', year:'numeric' });
  const time = now.toTimeString().slice(0,5);
  return (
    <div className="alto-masthead">
      <div className="left">
        <div className="edition">No. 4,182 · Late Edition</div>
        <div className="date ser-it">{date}</div>
      </div>
      <div className="ttl">Alto<span className="sup">·</span></div>
      <div className="right">
        <div className="session"><span className="dot"/> NYSE OPEN</div>
        <div className="clock">{time} ET · LATENCY 4ms · NY-4</div>
      </div>
    </div>
  );
}

function AltoNav({ route, setRoute }) {
  const items = [
    {k:'folio',   l:'The Folio'},
    {k:'reading', l:'The Reading Room'},
    {k:'book',    l:'The Book'},
  ];
  return (
    <div className="alto-nav">
      {items.map(it => (
        <div key={it.k} className={'item ser '+(route===it.k?'on':'')} onClick={()=>setRoute(it.k)}>{it.l}</div>
      ))}
      <div style={{flex:1}}/>
      <div className="item ser-it" style={{color:'var(--alto-muted)'}}>Search · ⌘K</div>
    </div>
  );
}

// =========================================================
// THE FOLIO — front page
// =========================================================
function Folio({ D, activeRow, setActiveSym }) {
  const top = activeRow;
  return (
    <div className="folio" data-screen-label="Alto · The Folio">
      <div className="folio-hero">
        <div>
          <div className="kicker ser-it">Tape Lead · {new Date().toLocaleTimeString('en-US',{hour:'2-digit',minute:'2-digit'})}</div>
          <h1>Nvidia powers the index higher as breadth quietly widens.</h1>
          <div className="deck ser-it">
            A late-session bid in semiconductors carried the tape, with {top.sym} up{' '}
            {top.pct.toFixed(2)}% on heavier-than-typical volume; advancers led decliners
            on the Big Board by a margin not seen in three weeks.
          </div>
          <div className="byline">By the Tape Desk · {top.sym}, AVGO, AMD, META</div>
        </div>
        <div className="hero-price">
          <div className="sym">{top.sym} <span style={{color:'var(--alto-muted)', fontStyle:'italic', fontSize:13, marginLeft:6}}>{top.name}</span></div>
          <div className="last">{top.last.toLocaleString('en-US',{minimumFractionDigits:2})}</div>
          <div className={'chg '+(top.chg>=0?'up':'down')}>{top.chg>=0?'+':''}{top.chg.toFixed(2)} · {top.pct>=0?'+':''}{top.pct.toFixed(2)}%</div>
          <div style={{marginTop:8}}>
            {[
              {l:'Open',   v:(top.last-top.chg).toFixed(2)},
              {l:'High',   v:(top.last+1.84).toFixed(2)},
              {l:'Low',    v:(top.last-3.42).toFixed(2)},
              {l:'Volume', v:top.vol},
              {l:'52W High', v:'1,348.55'},
              {l:'P/E',    v:'68.2'},
            ].map(r => (
              <div key={r.l} className="row"><span className="l">{r.l}</span><span className="v">{r.v}</span></div>
            ))}
          </div>
        </div>
      </div>

      <div className="folio-body">
        {/* Left column: Indices */}
        <div className="folio-col">
          <div className="alto-sec">
            <div className="h ser-it">Markets at a glance</div>
            <div className="meta">Live · ET</div>
          </div>
          <table className="alto-idx-table">
            <tbody>
              {D.indices.map(ix => (
                <tr key={ix.n}>
                  <td className="n">{ix.n}</td>
                  <td className="v">{ix.lvl}</td>
                  <td className={'c '+(ix.c>=0?'up':'down')}>{ix.c>=0?'+':''}{ix.c.toFixed(2)}%</td>
                </tr>
              ))}
            </tbody>
          </table>
          <div className="alto-sec" style={{marginTop:24}}>
            <div className="h ser-it">Sectors</div>
            <div className="meta">S&amp;P</div>
          </div>
          <table className="alto-idx-table">
            <tbody>
              {(D.heatmap || []).slice(0,6).map(s => (
                <tr key={s.sec}>
                  <td className="n">{s.sec}</td>
                  <td className="v">{s.w}% w</td>
                  <td className={'c '+(s.items[0].p>=0?'up':'down')}>{s.items[0].p>=0?'+':''}{s.items[0].p.toFixed(2)}%</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        {/* Center column: Watchlist */}
        <div className="folio-col">
          <div className="alto-sec">
            <div className="h ser-it">The Watch — twelve names</div>
            <div className="meta">As of close</div>
          </div>
          <table className="alto-wl">
            <thead>
              <tr><th>Symbol</th><th>Name</th><th className="r">Last</th><th className="r">Chg</th><th className="r">Vol</th><th className="r">Mkt cap</th></tr>
            </thead>
            <tbody>
              {D.watchlist.map(w => (
                <tr key={w.sym} onClick={()=>setActiveSym(w.sym)} style={{cursor:'pointer'}}>
                  <td className="sym">{w.sym}</td>
                  <td className="nm">{w.name}</td>
                  <td className="num">{w.last.toLocaleString('en-US',{minimumFractionDigits:2})}</td>
                  <td className={'num '+(w.pct>=0?'up':'down')}>{w.pct>=0?'+':''}{w.pct.toFixed(2)}%</td>
                  <td className="num" style={{color:'var(--alto-muted)'}}>{w.vol}</td>
                  <td className="num" style={{color:'var(--alto-muted)'}}>{w.mcap}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        {/* Right column: Wire */}
        <div className="folio-col">
          <div className="alto-sec">
            <div className="h ser-it">From the wires</div>
            <div className="meta">Bloomberg · Reuters · FT</div>
          </div>
          <div className="alto-wire">
            {D.news.map((n,i) => (
              <div key={i} className="item">
                <div className="meta"><span className="src">{n.src}</span><span>{n.t}</span><span className={'tag '+n.tone}>{n.tag}</span></div>
                <div className={'h '+(i===0?'lede':'')}>{n.head}</div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

// =========================================================
// THE READING ROOM — chart workstation as research note
// =========================================================
function ReadingRoom({ D, activeRow, setActiveSym }) {
  return (
    <div data-screen-label="Alto · The Reading Room">
      <div className="rr-headline">
        <div>
          <div className="sym ser">{activeRow.sym} <span style={{color:'var(--alto-muted)', fontStyle:'italic', fontSize:16, fontWeight:400}}>{activeRow.name}</span></div>
          <div className="meta">NASDAQ · USD · Common Stock · CUSIP 67066G104</div>
        </div>
        <div style={{textAlign:'right'}}>
          <div className="meta">As of close</div>
          <div className="price">{activeRow.last.toLocaleString('en-US',{minimumFractionDigits:2})}</div>
          <div className={'delta '+(activeRow.chg>=0?'up':'down')}>{activeRow.chg>=0?'+':''}{activeRow.chg.toFixed(2)} · {activeRow.pct>=0?'+':''}{activeRow.pct.toFixed(2)}%</div>
        </div>
        <div style={{display:'flex', gap:8}}>
          <div className="alto-pill on">1D</div>
          <div className="alto-pill">5D</div>
          <div className="alto-pill">1M</div>
          <div className="alto-pill">6M</div>
          <div className="alto-pill">1Y</div>
          <div className="alto-pill">5Y</div>
        </div>
      </div>
      <div className="reading-room">
        {/* Left rail: watchlist */}
        <div className="col">
          <div className="alto-sec"><div className="h ser-it">The Watch</div><div className="meta">12 names</div></div>
          <table className="alto-wl">
            <tbody>
              {D.watchlist.map(w => (
                <tr key={w.sym} onClick={()=>setActiveSym(w.sym)} style={{cursor:'pointer', background: w.sym===activeRow.sym?'var(--alto-stain)':'transparent'}}>
                  <td className="sym" style={{width:60}}>{w.sym}</td>
                  <td className="num">{w.last.toFixed(2)}</td>
                  <td className={'num '+(w.pct>=0?'up':'down')} style={{width:60}}>{w.pct>=0?'+':''}{w.pct.toFixed(2)}%</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        {/* Center: chart + footnotes */}
        <div className="col">
          <div className="rr-chart-wrap">
            <AltoChart candles={D.candles} />
            <div className="annotation" style={{top:60, left:'18%'}}><span className="ref">¹</span> Breakout above resistance at 1,196.</div>
            <div className="annotation" style={{top:200, left:'52%'}}><span className="ref">²</span> Reversal on Q1 earnings beat.</div>
            <div className="annotation" style={{top:120, left:'78%'}}><span className="ref">³</span> Trend continuation; volume confirms.</div>
          </div>
          <div className="rr-footnotes">
            <div className="f"><span className="n">¹</span><span className="t">Apr 03 — Broke through three-month resistance on heavy volume; first close above prior swing high since the gap of Aug 5 prior year.</span></div>
            <div className="f"><span className="n">²</span><span className="t">Apr 24 — Q1 print: $30.2B revenue vs $28.6B est; gross margin 75.1%. Forward guide raised; the buy-side consensus PT moved to $1,400.</span></div>
            <div className="f"><span className="n">³</span><span className="t">May 06 — Trend channel intact; OBV diverges higher with rising 20-day average daily volume of 48.2M shares.</span></div>
          </div>
        </div>

        {/* Right rail: DOM + ticket */}
        <div className="col rr-dom-col">
          <div className="alto-sec"><div className="h ser-it">The Book</div><div className="meta">Lvl 2 · NSDQ</div></div>
          <div className="rr-dom">
            {[...Array(7)].reverse().map((_,i)=>{ const off=7-i; const px=(activeRow.last+off*0.05).toFixed(2); const sz=Math.floor(220+Math.sin(off*1.3)*120); return (
              <div key={'a'+i} className="row"><span className="sz"></span><span className="px">{px}</span><span className="ask">{sz}</span></div>
            );})}
            <div className="row mid"><span className="bid"></span><span className="px">{activeRow.last.toFixed(2)}</span><span className="ask"></span></div>
            {[...Array(7)].map((_,i)=>{ const off=i+1; const px=(activeRow.last-off*0.05).toFixed(2); const sz=Math.floor(220+Math.cos(off*1.1)*120); return (
              <div key={'b'+i} className="row"><span className="bid">{sz}</span><span className="px">{px}</span><span className="sz"></span></div>
            );})}
          </div>

          <div className="rr-form">
            <div className="row"><span className="l">Account</span><span className="v">Main · USD</span></div>
            <div className="row"><span className="l">Order type</span><span className="v">Limit</span></div>
            <div className="row"><span className="l">Quantity</span><span className="v in">100</span></div>
            <div className="row"><span className="l">Limit price</span><span className="v in">{activeRow.last.toFixed(2)}</span></div>
            <div className="row"><span className="l">Time in force</span><span className="v">Day</span></div>
            <div className="row"><span className="l">Notional</span><span className="v">${(activeRow.last*100).toLocaleString('en-US',{maximumFractionDigits:0})}</span></div>
            <div className="side-row">
              <div className="b buy">Buy</div>
              <div className="b sell">Sell</div>
            </div>
            <div style={{fontSize:11, fontStyle:'italic', color:'var(--alto-muted)', textAlign:'center', marginTop:6}}>Review before submitting.</div>
          </div>
        </div>
      </div>
    </div>
  );
}

// =========================================================
// THE BOOK — positions ledger / balance sheet
// =========================================================
function Book({ D }) {
  const positions = D.positions || [];
  const totMv = positions.reduce((a,b)=>a+b.mv,0);
  const totPl = positions.reduce((a,b)=>a+b.pl,0);
  return (
    <div className="book" data-screen-label="Alto · The Book">
      <h2 className="ser">Holdings &amp; Performance</h2>
      <div className="lede ser-it">A monthly statement of the Main account, reconciled to the close on {new Date().toLocaleDateString('en-US',{month:'long', day:'numeric'})}. Realized P&amp;L is settled; unrealized P&amp;L is mark-to-market.</div>

      <div className="book-summary">
        <div>
          <div className="l">Total equity</div>
          <div className="v">${(totMv/1e6).toFixed(2)}M</div>
          <div className="sub">Long $5.41M · Cash $0.42M</div>
        </div>
        <div>
          <div className="l">P&amp;L · today</div>
          <div className={'v '+(totPl>=0?'up':'down')}>{totPl>=0?'+':''}${(totPl/1000).toFixed(1)}K</div>
          <div className="sub">+1.74% on equity</div>
        </div>
        <div>
          <div className="l">P&amp;L · YTD</div>
          <div className="v up">+$842.1K</div>
          <div className="sub">+16.8% · vs SPX +9.8%</div>
        </div>
        <div>
          <div className="l">Sharpe · 12m</div>
          <div className="v">2.41</div>
          <div className="sub">σ 14.2% · β 1.18</div>
        </div>
      </div>

      <div className="alto-sec" style={{marginTop:28}}><div className="h ser-it">Open positions</div><div className="meta">{positions.length} lines</div></div>
      <table className="ledger">
        <thead>
          <tr>
            <th>Symbol</th>
            <th>Side</th>
            <th className="r">Qty</th>
            <th className="r">Avg cost</th>
            <th className="r">Last</th>
            <th className="r">Market value</th>
            <th>P&amp;L curve</th>
            <th className="r">P&amp;L · 1D</th>
            <th className="r">Return</th>
          </tr>
        </thead>
        <tbody>
          {positions.map(p => (
            <tr key={p.sym}>
              <td><span className="sym">{p.sym}</span><div className="nm">{D.watchlist.find(w=>w.sym===p.sym)?.name || ''}</div></td>
              <td className="ser-it">{p.side === 'LONG' ? 'Long' : 'Short'}</td>
              <td className="r">{p.qty.toLocaleString()}</td>
              <td className="r">{p.avg.toFixed(2)}</td>
              <td className="r">{p.last.toFixed(2)}</td>
              <td className="r">${(p.mv).toLocaleString()}</td>
              <td><InlineSpark pl={p.pl}/></td>
              <td className={'r '+(p.pl>=0?'up':'down')}>{p.pl>=0?'+':''}${(p.pl/1000).toFixed(1)}K</td>
              <td className={'r '+(p.plp>=0?'up':'down')}>{p.plp>=0?'+':''}{p.plp.toFixed(2)}%</td>
            </tr>
          ))}
        </tbody>
        <tfoot>
          <tr>
            <td>Total · open positions</td>
            <td></td><td></td><td></td><td></td>
            <td className="r">${totMv.toLocaleString()}</td>
            <td></td>
            <td className={'r '+(totPl>=0?'up':'down')}>{totPl>=0?'+':''}${(totPl/1000).toFixed(1)}K</td>
            <td className="r">{((totPl/totMv)*100).toFixed(2)}%</td>
          </tr>
        </tfoot>
      </table>

      <h2 className="ser" style={{marginTop:36}}>Sector exposure</h2>
      <div className="alto-heat">
        {(D.heatmap || []).map(sec => (
          <div key={sec.sec} className="sec">
            <div className="h">{sec.sec} · {sec.w}%</div>
            <div className="body">
              {sec.items.slice(0,4).map(it => (
                <div key={it.s} className="row"><span className="s">{it.s}</span><span className={'p '+(it.p>=0?'up':'down')}>{it.p>=0?'+':''}{it.p.toFixed(2)}%</span></div>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

// Small inline sparkline for ledger — minimal, ink only
function InlineSpark({ pl }) {
  const up = pl >= 0;
  const W = 84, H = 18;
  const r = alM(()=>Math.random,[]);
  const pts = alM(()=>{
    const seed = Math.abs(pl)/1000;
    const arr = []; let v = 0;
    for (let i=0;i<24;i++){ v += (Math.sin(i*1.3+seed)+Math.cos(i*0.7+seed))*0.5 + (up?0.18:-0.18); arr.push(v); }
    return arr;
  },[pl]);
  const min = Math.min(...pts), max = Math.max(...pts), span = max-min || 1;
  const path = pts.map((p,i)=>`${i?'L':'M'}${(i/(pts.length-1)*W).toFixed(1)} ${(H-((p-min)/span)*H).toFixed(1)}`).join(' ');
  return (
    <svg viewBox={`0 0 ${W} ${H}`} className="spk" width={W} height={H}>
      <path d={path} fill="none" stroke={up?'var(--alto-up)':'var(--alto-down)'} strokeWidth="1.4"/>
    </svg>
  );
}

// =========================================================
// CHART — editorial line chart with serif gridlines
// =========================================================
function AltoChart({ candles }) {
  const W = 900, H = 360, pad = 24;
  const innerW = W - pad*2 - 50;
  const innerH = H - pad*2;
  const closes = candles.map(c=>c.c);
  const min = Math.min(...closes), max = Math.max(...closes), span = max-min || 1;
  const x = i => pad + (i/(closes.length-1)) * innerW;
  const y = v => pad + (1 - (v-min)/span) * innerH;
  const path = closes.map((v,i)=>`${i?'L':'M'}${x(i).toFixed(1)} ${y(v).toFixed(1)}`).join(' ');
  // Light gridlines as serif rules
  const gridY = [0, 0.25, 0.5, 0.75, 1].map(t => pad + t*innerH);
  // Y-axis labels
  return (
    <svg viewBox={`0 0 ${W} ${H}`} style={{width:'100%', height:H, display:'block'}}>
      {gridY.map((gy,i) => (
        <g key={i}>
          <line x1={pad} x2={W-pad-50} y1={gy} y2={gy} stroke="var(--alto-hair-2)" strokeWidth="0.5"/>
          <text x={W-pad-46} y={gy+3.5} fontFamily="JetBrains Mono" fontSize="10" fill="var(--alto-muted)">
            {(max - (i*0.25)*span).toFixed(0)}
          </text>
        </g>
      ))}
      {/* x ticks */}
      {[0, 0.25, 0.5, 0.75, 1].map((t,i)=>{
        const xi = Math.round(t*(closes.length-1));
        return (
          <text key={i} x={x(xi)} y={H-pad+14} textAnchor="middle"
                fontFamily="Spectral" fontStyle="italic" fontSize="10" fill="var(--alto-muted)">
            {['Mar','Apr','Apr','May','May'][i]}
          </text>
        );
      })}
      <path d={path} fill="none" stroke="var(--alto-ink)" strokeWidth="1.4"/>
      {/* Area below — newsprint stain */}
      <path d={path + ` L ${x(closes.length-1).toFixed(1)} ${H-pad} L ${x(0).toFixed(1)} ${H-pad} Z`} fill="var(--alto-stain)" opacity="0.45"/>
      {/* Reference points (matching footnotes) */}
      {[
        {i: Math.round(closes.length*0.18), n:'¹'},
        {i: Math.round(closes.length*0.52), n:'²'},
        {i: Math.round(closes.length*0.78), n:'³'},
      ].map((r,k) => (
        <g key={k}>
          <circle cx={x(r.i)} cy={y(closes[r.i])} r="4" fill="var(--alto-down)"/>
          <text x={x(r.i)} y={y(closes[r.i])-10} textAnchor="middle"
                fontFamily="Spectral" fontWeight="700" fontSize="13" fill="var(--alto-down)">{r.n}</text>
        </g>
      ))}
    </svg>
  );
}

function AltoTape({ items }) {
  return (
    <div className="alto-tape">
      {items.map((ix,i) => (
        <div key={i} className="item">
          <span className="nm ser-it">{ix.n}</span>
          <span className="v">{ix.lvl}</span>
          <span className={'v '+(ix.c>=0?'up':'down')}>{ix.c>=0?'+':''}{ix.c.toFixed(2)}%</span>
        </div>
      ))}
    </div>
  );
}

window.AltoApp = AltoApp;
