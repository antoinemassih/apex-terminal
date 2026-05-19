// Apex extras — floating order ticket, context menu, tools popover, overlay dropdown, analysis sidebar
const { useState: xS, useEffect: xE, useRef: xR } = React;

// ────────────────────────────────────────────────────────────────────
// FLOATING ORDER TICKET — draggable, compact, editorial styling
// ────────────────────────────────────────────────────────────────────
function ApexFloatingTicket({ sym, last, onClose }){
  const [pos, setPos] = xS({x: window.innerWidth - 360, y: 220});
  const [side, setSide] = xS('buy');
  const [type, setType] = xS('LMT');
  const [tif,  setTif]  = xS('DAY');
  const [qty, setQty]   = xS(100);
  const [px, setPx]     = xS(last.toFixed(2));
  const [stop, setStop] = xS((last*0.98).toFixed(2));
  const [tp, setTp]     = xS((last*1.04).toFixed(2));
  const [bracket, setBracket] = xS(false);

  const dragRef = xR(null);
  function onMouseDown(e){
    const start = {x:e.clientX, y:e.clientY, ox:pos.x, oy:pos.y};
    function mm(ev){ setPos({x: start.ox + ev.clientX - start.x, y: start.oy + ev.clientY - start.y}); }
    function mu(){ window.removeEventListener('mousemove', mm); window.removeEventListener('mouseup', mu); }
    window.addEventListener('mousemove', mm);
    window.addEventListener('mouseup', mu);
  }

  const bid = (last - 0.05).toFixed(2);
  const ask = (last + 0.05).toFixed(2);
  const not = (qty * parseFloat(px||0));

  return (
    <div className="float-tkt" style={{left:pos.x, top:pos.y}}>
      <div className="ft-head" onMouseDown={onMouseDown} ref={dragRef}>
        <span className="acc-dot"/>
        <span className="mono ttl">ORDER TICKET</span>
        <span className="mono sym">{sym}</span>
        <div className="grow"/>
        <span className="mono x" onClick={onClose}>×</span>
      </div>
      <div className="ft-quote">
        <div className="qcol">
          <div className="lab">BID</div>
          <div className="up mono v">{bid}</div>
        </div>
        <div className="qcol main">
          <div className="lab">LAST</div>
          <div className="mono v">{last.toFixed(2)}</div>
        </div>
        <div className="qcol">
          <div className="lab">ASK</div>
          <div className="down mono v">{ask}</div>
        </div>
      </div>
      <div className="ft-side">
        <div onClick={()=>setSide('buy')}  className={'ft-sb buy '+(side==='buy'?'on':'')}>BUY · LONG</div>
        <div onClick={()=>setSide('sell')} className={'ft-sb sell '+(side==='sell'?'on':'')}>SELL · SHORT</div>
      </div>
      <div className="ft-row">
        <span className="lab">TYPE</span>
        <div className="seg">
          {['MKT','LMT','STP','STP-LMT'].map(t=>(
            <span key={t} onClick={()=>setType(t)} className={t===type?'on':''}>{t}</span>
          ))}
        </div>
      </div>
      <div className="ft-row">
        <span className="lab">TIF</span>
        <div className="seg">
          {['DAY','GTC','IOC','FOK'].map(t=>(
            <span key={t} onClick={()=>setTif(t)} className={t===tif?'on':''}>{t}</span>
          ))}
        </div>
      </div>
      <div className="ft-grid">
        <div className="fld">
          <div className="lab">QUANTITY</div>
          <div className="ipt">
            <input value={qty} onChange={e=>setQty(parseInt(e.target.value)||0)}/>
            <div className="step">
              <span onClick={()=>setQty(Math.max(0,qty-100))}>−</span>
              <span onClick={()=>setQty(qty+100)}>+</span>
            </div>
          </div>
          <div className="presets">
            {[100,500,1000].map(n=>(
              <span key={n} onClick={()=>setQty(n)}>{n}</span>
            ))}
          </div>
        </div>
        {type !== 'MKT' && (
          <div className="fld">
            <div className="lab">LIMIT PRICE</div>
            <div className="ipt">
              <input value={px} onChange={e=>setPx(e.target.value)}/>
              <div className="step">
                <span onClick={()=>setPx((parseFloat(px)-0.05).toFixed(2))}>−</span>
                <span onClick={()=>setPx((parseFloat(px)+0.05).toFixed(2))}>+</span>
              </div>
            </div>
            <div className="presets">
              <span onClick={()=>setPx(bid)}>BID</span>
              <span onClick={()=>setPx(last.toFixed(2))}>LAST</span>
              <span onClick={()=>setPx(ask)}>ASK</span>
            </div>
          </div>
        )}
      </div>

      <div className="ft-bracket">
        <label className="check" onClick={()=>setBracket(!bracket)}>
          <span className={'box '+(bracket?'on':'')}>{bracket?'✓':''}</span>
          <span className="mono lab">BRACKET · STOP + TARGET</span>
        </label>
        {bracket && (
          <div className="ft-grid" style={{marginTop:8}}>
            <div className="fld">
              <div className="lab">STOP LOSS</div>
              <div className="ipt"><input value={stop} onChange={e=>setStop(e.target.value)} className="dn"/></div>
            </div>
            <div className="fld">
              <div className="lab">TAKE PROFIT</div>
              <div className="ipt"><input value={tp} onChange={e=>setTp(e.target.value)} className="up"/></div>
            </div>
          </div>
        )}
      </div>

      <div className="ft-summary">
        <div><span className="lab">NOTIONAL</span><span className="mono v">${(not).toLocaleString('en-US',{maximumFractionDigits:0})}</span></div>
        <div><span className="lab">BUYING POWER</span><span className="mono v">$5.83M</span></div>
        <div><span className="lab">EST. SLIPPAGE</span><span className="mono v">0.8 bp</span></div>
      </div>
      <div className={'ft-submit '+side}>
        REVIEW {side==='buy'?'BUY':'SELL'} · {qty} @ {type==='MKT'?'MKT':px}
      </div>
    </div>
  );
}

// ────────────────────────────────────────────────────────────────────
// CONTEXT MENU — right-click chart
// ────────────────────────────────────────────────────────────────────
function ApexContextMenu({ x, y, price, onClose }){
  xE(()=>{
    const close = () => onClose();
    document.addEventListener('click', close);
    return () => document.removeEventListener('click', close);
  }, []);
  const items = [
    {sec:null, rows:[
      {ic:'⟲', l:'Reset View'},
      {ic:'⊕', l:'Drag Zoom'},
      {ic:'⟚', l:'Measure (Shift+Drag)'},
    ]},
    {sec:`ORDERS @ ${price}`, rows:[
      {ic:'↑', l:'Buy Order',  c:'up'},
      {ic:'↓', l:'Sell Order', c:'down'},
      {ic:'⛨', l:'Stop Loss',  c:'down'},
      {ic:'☐', l:'OCO Bracket', c:'acc'},
      {ic:'☐', l:'Bracket Presets', c:'acc', arrow:true},
      {ic:'⟲', l:'Trigger Order', c:'acc'},
    ]},
    {sec:`ALERTS @ ${price}`, rows:[
      {ic:'↑', l:`Alert Above ${price}`},
      {ic:'↓', l:`Alert Below ${price}`},
    ]},
    {sec:'DRAWING TOOLS', rows:[
      {ic:'',l:'Lines',     arrow:true},
      {ic:'',l:'Channels',  arrow:true},
      {ic:'',l:'Fibonacci', arrow:true},
      {ic:'',l:'Ranges',    arrow:true},
      {ic:'',l:'Computed',  arrow:true},
      {ic:'',l:'Patterns',  arrow:true},
      {ic:'',l:'Markers',   arrow:true},
      {ic:'',l:'Annotations', arrow:true},
    ]},
    {sec:null, rows:[
      {ic:'★', l:'Save as Template'},
      {ic:'◍', l:'Hide All'},
      {ic:'◐', l:'Hide / Show', arrow:true},
    ]},
    {sec:null, rows:[
      {ic:'⌫', l:'Delete All Drawings', c:'down'},
      {ic:'⌫', l:'Delete Temp Drawings (4)', c:'down'},
      {ic:'⌫', l:'Delete', c:'down', arrow:true},
    ]},
  ];
  return (
    <div className="ctx-menu" style={{left:x, top:y}} onClick={e=>e.stopPropagation()}>
      {items.map((g,gi)=>(
        <div key={gi} className="ctx-grp">
          {g.sec && <div className="ctx-sec mono">{g.sec}</div>}
          {g.rows.map((r,ri)=>(
            <div key={ri} className={'ctx-row '+(r.c||'')}>
              <span className="ic mono">{r.ic}</span>
              <span className="l">{r.l}</span>
              {r.arrow && <span className="ar">▸</span>}
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}

// ────────────────────────────────────────────────────────────────────
// TOOLS POPOVER — drawing tools palette
// ────────────────────────────────────────────────────────────────────
function ApexToolsPopover({ x, y, onClose }){
  xE(()=>{
    const close = (e) => { if (!e.target.closest?.('.tools-pop')) onClose(); };
    setTimeout(()=>document.addEventListener('click', close), 50);
    return () => document.removeEventListener('click', close);
  }, []);
  const favs = [
    {ic:'⟍', on:true},
    {ic:'⊕'},
    {ic:'⟚'},
    {ic:'▭'},
    {ic:'⟿'},
    {ic:'●'},
  ];
  const cats = ['LINES','ZONES','FIBONACCI','GANN / PITCHFORK','HARMONIC','UTILITY'];
  return (
    <div className="tools-pop" style={{left:x, top:y}}>
      <div className="tp-sec mono">FAVORITES</div>
      <div className="tp-favs">
        {favs.map((f,i)=>(
          <div key={i} className={'tp-fav '+(f.on?'on':'')}>{f.ic}</div>
        ))}
      </div>
      <div className="tp-sec mono">ALL TOOLS</div>
      <div className="tp-cats">
        {cats.map(c=>(
          <div key={c} className="tp-cat mono">
            <span>{c}</span>
            <span className="ar">▸</span>
          </div>
        ))}
      </div>
    </div>
  );
}

// ────────────────────────────────────────────────────────────────────
// OVERLAY DROPDOWN — small popup from toolbar
// ────────────────────────────────────────────────────────────────────
function ApexOverlayDrop({ x, y, onClose }){
  xE(()=>{
    const close = (e) => { if (!e.target.closest?.('.ovl-drop')) onClose(); };
    setTimeout(()=>document.addEventListener('click', close), 50);
    return () => document.removeEventListener('click', close);
  }, []);
  const rows = [
    {ic:'≈', l:'Technical', arrow:true},
    {ic:'≡', l:'Structure',  arrow:true},
    {ic:'≈', l:'Regime',     arrow:true},
    {ic:'▮', l:'Data',       arrow:true},
  ];
  return (
    <div className="ovl-drop" style={{left:x, top:y}}>
      {rows.map((r,i)=>(
        <div key={i} className="od-row">
          <span className="ic mono">{r.ic}</span>
          <span className="l">{r.l}</span>
          {r.arrow && <span className="ar">▸</span>}
        </div>
      ))}
      <div className="od-sec mono">SYMBOL OVERLAY</div>
      <div className="od-row acc">
        <span className="ic mono">⊞</span>
        <span className="l">Add Symbol Overlay</span>
      </div>
    </div>
  );
}

// ────────────────────────────────────────────────────────────────────
// ANALYSIS SIDEBAR — RRG, T&S, Scanner, Scripts, Seasonality, Research
// ────────────────────────────────────────────────────────────────────
function ApexAnalysisSidebar({ onClose }){
  const [tab, setTab] = xS('RRG');
  const tabs = ['RRG','T&S','Scanner','Scripts','Seasonality','Research'];
  return (
    <div className="ana-side">
      <div className="ana-head">
        <span className="acc-dot"/>
        <span className="mono ttl">ANALYSIS</span>
        <div className="grow"/>
        <span className="mono add">+</span>
        <span className="mono x" onClick={onClose}>×</span>
      </div>
      <div className="ana-tabs">
        {tabs.map(t=>(
          <span key={t} onClick={()=>setTab(t)} className={'tab mono '+(t===tab?'on':'')}>{t}</span>
        ))}
      </div>
      {tab==='RRG' && <RRGPanel/>}
      {tab!=='RRG' && (
        <div className="ana-empty mono">
          <div>{tab.toUpperCase()}</div>
          <div className="hint">No data yet — connect a feed.</div>
        </div>
      )}
    </div>
  );
}

function RRGPanel(){
  // Relative rotation graph — quadrants improving / leading / lagging / weakening
  const W=440, H=300;
  const pts = [
    {k:'XLK', q:'lead', x:340, y:120, c:'#3aa6e8'},
    {k:'XLU', q:'impr', x:160, y:140, c:'#e8c93a'},
    {k:'XLV', q:'impr', x:200, y:170, c:'#e83a78'},
    {k:'XLC', q:'lead', x:300, y:170, c:'#3ad6c8'},
    {k:'XLRE',q:'impr', x:200, y:200, c:'#a4e83a'},
    {k:'XLB', q:'lag',  x:140, y:230, c:'#c87a3a'},
    {k:'XLI', q:'weak', x:300, y:220, c:'#e89e3a'},
    {k:'XLY', q:'weak', x:340, y:240, c:'#d63aa6'},
    {k:'XLF', q:'weak', x:330, y:260, c:'#3ae87a'},
    {k:'XLP', q:'lag',  x:180, y:250, c:'#e8c93a'},
  ];
  return (
    <div className="rrg-wrap">
      <div className="rrg-ttl">
        <span className="mono lab">RRG</span>
        <span className="mono ttl-r">— Relative Rotation</span>
      </div>
      <svg viewBox={`0 0 ${W} ${H}`} style={{width:'100%', height:300, display:'block'}}>
        {/* quadrants */}
        <rect x="0" y="0" width={W/2} height={H/2} fill="#1a2b3f" opacity="0.6"/>
        <rect x={W/2} y="0" width={W/2} height={H/2} fill="#1f3a26" opacity="0.7"/>
        <rect x="0" y={H/2} width={W/2} height={H/2} fill="#3a261f" opacity="0.55"/>
        <rect x={W/2} y={H/2} width={W/2} height={H/2} fill="#3a341f" opacity="0.55"/>
        {/* axes */}
        <line x1="0" x2={W} y1={H/2} y2={H/2} stroke="rgba(255,255,255,.18)" strokeDasharray="2 3"/>
        <line x1={W/2} x2={W/2} y1="0" y2={H} stroke="rgba(255,255,255,.18)" strokeDasharray="2 3"/>
        {/* labels */}
        <text x="10" y="16" fontSize="9" fill="#b3ad9c" fontFamily="JetBrains Mono" letterSpacing="1.5">IMPROVING</text>
        <text x={W-10} y="16" fontSize="9" fill="#b3ad9c" fontFamily="JetBrains Mono" letterSpacing="1.5" textAnchor="end">LEADING</text>
        <text x="10" y={H-8} fontSize="9" fill="#b3ad9c" fontFamily="JetBrains Mono" letterSpacing="1.5">LAGGING</text>
        <text x={W-10} y={H-8} fontSize="9" fill="#b3ad9c" fontFamily="JetBrains Mono" letterSpacing="1.5" textAnchor="end">WEAKENING</text>
        {/* trail + points */}
        {pts.map((p,i)=>{
          const tail = `M${p.x-22} ${p.y-8} L${p.x} ${p.y}`;
          return (
            <g key={i}>
              <path d={tail} stroke={p.c} strokeWidth="1.4" fill="none" opacity="0.7"/>
              <circle cx={p.x} cy={p.y} r="6" fill={p.c}/>
              <text x={p.x+10} y={p.y+3} fontSize="9" fill="#fff" fontFamily="JetBrains Mono" fontWeight="600">{p.k}</text>
            </g>
          );
        })}
        <text x={W/2} y={H-22} fontSize="9" fill="#8a8675" fontFamily="JetBrains Mono" textAnchor="middle">RS-Ratio</text>
      </svg>

      <div className="rrg-ctl">
        <div className="row"><span className="mono lab">TIME</span><div className="track"><div className="dot l"/></div></div>
        <div className="row"><span className="mono lab">TAIL</span><div className="track"><div className="dot r"/></div></div>
      </div>

      <div className="rrg-cyc">
        <div className="head">
          <span className="mono lab">CYCLE:</span>
          <span className="mono v acc">LATE EXPANSION</span>
        </div>
        <div className="legend">
          {[['XLK','#3aa6e8'],['XLB','#c87a3a'],['XLC','#3ad6c8'],['XLP','#e8c93a'],['XLF','#3ae87a'],['XLU','#e8c93a'],['XLI','#e89e3a'],['XLV','#e83a78'],['XLY','#d63aa6'],['XLRE','#a4e83a'],['XLE','#c43a1f']].map(([k,c])=>(
            <div key={k} className="ent"><span className="d" style={{background:c}}/><span className="mono">{k}</span></div>
          ))}
        </div>
      </div>
    </div>
  );
}

Object.assign(window, { ApexFloatingTicket, ApexContextMenu, ApexToolsPopover, ApexOverlayDrop, ApexAnalysisSidebar });
