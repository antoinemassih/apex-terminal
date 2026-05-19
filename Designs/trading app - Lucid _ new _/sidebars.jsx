// Sidebars + upgraded command palette + settings modal overlay
// All editorial-light by default, but the command palette has the dark
// "spotlight" treatment from the reference.
const { useState: sS, useEffect: sE, useMemo: sM, useRef: sR } = React;

// ─────────────────────────────────────────────────────────────────────
// SIDEBAR SHELL — drawer that slides in from the right
// ─────────────────────────────────────────────────────────────────────
function Sidebar({ open, onClose, title, sub, width=380, dark=false, children }){
  if (!open) return null;
  return (
    <div className="sb-overlay" onClick={onClose}>
      <div className={'sb '+(dark?'sb-dark':'')} style={{width}} onClick={e=>e.stopPropagation()}>
        <div className="sb-head">
          <div className="sb-tag">
            <span className="sb-dot"/>
            <div>
              <div className="sb-ttl mono">{title}</div>
              {sub && <div className="sb-sub mono">{sub}</div>}
            </div>
          </div>
          <span className="sb-x mono" onClick={onClose}>×</span>
        </div>
        <div className="sb-body">{children}</div>
      </div>
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────
// OPTIONS CHAIN SIDEBAR
// ─────────────────────────────────────────────────────────────────────
function OptionsChainSidebar({ open, onClose, sym, last }){
  const [exp, setExp] = sS('May 17');
  const [strikes] = sS(()=>{
    const out = [];
    for (let i=10; i>=-10; i--){
      const k = Math.round((last + i*5)/5)*5;
      const itm = i<0;
      const cBid = Math.max(0.05, (last - k) + (Math.abs(i)*0.4 + 1.2)).toFixed(2);
      const cAsk = (parseFloat(cBid)+0.18).toFixed(2);
      const pBid = Math.max(0.05, (k - last) + (Math.abs(i)*0.4 + 1.2)).toFixed(2);
      const pAsk = (parseFloat(pBid)+0.18).toFixed(2);
      const seed = Math.sin(k*0.7)+Math.cos(k*0.4);
      const cVol = Math.round(2000 + seed*1500 + Math.abs(i)*-80);
      const pVol = Math.round(1500 + seed*1200 + Math.abs(i)*-60);
      const cIv = (28 + Math.abs(i)*0.6 + seed*0.4).toFixed(1);
      const pIv = (29 + Math.abs(i)*0.5 + seed*0.3).toFixed(1);
      out.push({k, itm, cBid, cAsk, pBid, pAsk, cVol, pVol, cIv, pIv});
    }
    return out;
  });

  const expiries = ['May 03','May 10','May 17','May 24','Jun 21','Jul 19','Sep 20','Dec 20','Jan-26','Jan-27'];

  return (
    <Sidebar open={open} onClose={onClose} title="OPTIONS CHAIN" sub={`${sym} · ATM ${last.toFixed(2)}`} width={620}>
      <div className="oc-exp-strip">
        {expiries.map(e=>(
          <span key={e} onClick={()=>setExp(e)} className={'pill mono '+(e===exp?'on':'')}>{e}</span>
        ))}
      </div>
      <div className="oc-meta">
        <div><span className="lab mono">DTE</span><span className="mono v">7</span></div>
        <div><span className="lab mono">IV</span><span className="mono v acc">28.4%</span></div>
        <div><span className="lab mono">IV RANK</span><span className="mono v">62</span></div>
        <div><span className="lab mono">P/C RATIO</span><span className="mono v down">0.82</span></div>
        <div><span className="lab mono">TOT VOL</span><span className="mono v">418K</span></div>
        <div><span className="lab mono">OI</span><span className="mono v">1.24M</span></div>
      </div>
      <div className="oc-grid">
        <div className="oc-head mono">
          <span>VOL</span><span>IV</span><span>BID</span><span>ASK</span>
          <span className="strike">STRIKE</span>
          <span>BID</span><span>ASK</span><span>IV</span><span>VOL</span>
        </div>
        <div className="oc-side-lab">
          <span className="mono up">CALLS</span>
          <span className="mono down">PUTS</span>
        </div>
        {strikes.map((r,i)=>{
          const atm = Math.abs(r.k - last) < 3;
          return (
            <div key={i} className={'oc-row '+(atm?'atm ':'')+(r.itm?'itm ':'')}>
              <span className="mono mut">{(r.cVol/1000).toFixed(1)}K</span>
              <span className="mono mut">{r.cIv}</span>
              <span className="mono up">{r.cBid}</span>
              <span className="mono">{r.cAsk}</span>
              <span className={'mono strike '+(atm?'atm':'')}>{r.k}</span>
              <span className="mono down">{r.pBid}</span>
              <span className="mono">{r.pAsk}</span>
              <span className="mono mut">{r.pIv}</span>
              <span className="mono mut">{(r.pVol/1000).toFixed(1)}K</span>
            </div>
          );
        })}
      </div>
      <div className="oc-foot">
        <span className="mono lab">STRATEGY</span>
        <span className="pill mono on">Single</span>
        <span className="pill mono">Vertical</span>
        <span className="pill mono">Iron Condor</span>
        <span className="pill mono">Calendar</span>
        <span className="pill mono">Strangle</span>
        <span className="pill mono">Butterfly</span>
      </div>
    </Sidebar>
  );
}

// ─────────────────────────────────────────────────────────────────────
// NEWS FEED SIDEBAR
// ─────────────────────────────────────────────────────────────────────
function NewsFeedSidebar({ open, onClose, items=[] }){
  const [filter, setFilter] = sS('ALL');
  const filters = ['ALL','EQUITIES','MACRO','EARNINGS','M&A','REGULATORY'];
  const data = (items.length && items[0] && items[0].tic) ? items : [
    {tm:'09:42', src:'BLOOMBERG', tag:'EQUITIES', t:'NVIDIA Blackwell wafer allocations show 15% upside vs. consensus, says Morgan Stanley.', tic:['NVDA','AMD','TSM'], imp:'high'},
    {tm:'09:38', src:'REUTERS',   tag:'MACRO',    t:'US Initial Jobless Claims fell to 211k vs. 216k expected; continuing claims drift lower.', tic:['SPY','TLT'], imp:'med'},
    {tm:'09:31', src:'WSJ',       tag:'M&A',      t:'Berkshire reportedly takes 4.2% stake in payments firm — filing due Friday.', tic:['BRK.B'], imp:'med'},
    {tm:'09:24', src:'BLOOMBERG', tag:'EARNINGS', t:'Apple guides Services growth to "double-digit" through FY26 on AI add-ons.', tic:['AAPL'], imp:'high'},
    {tm:'09:18', src:'CNBC',      tag:'EQUITIES', t:'Tesla recalls 240k Model Y vehicles over rear camera firmware issue.', tic:['TSLA'], imp:'low'},
    {tm:'09:14', src:'FT',        tag:'REGULATORY',t:'EU AI Act compliance grace period extended to Q1 2027 for foundation models.', tic:['GOOGL','MSFT','META'], imp:'med'},
    {tm:'09:08', src:'BLOOMBERG', tag:'MACRO',    t:'Brent crude up 1.4% as Red Sea tanker delays widen; refining margins firm.', tic:['XOM','CVX'], imp:'med'},
    {tm:'08:58', src:'REUTERS',   tag:'M&A',      t:'Adobe ends pursuit of Figma deal alternatives; redirects $20B to buybacks.', tic:['ADBE'], imp:'high'},
    {tm:'08:46', src:'WSJ',       tag:'EARNINGS', t:'Microsoft Q3 cloud bookings +28% Y/Y, Copilot seats +41%; Azure margins steady.', tic:['MSFT'], imp:'high'},
  ];
  const filtered = filter==='ALL' ? data : data.filter(d=>d.tag===filter);

  return (
    <Sidebar open={open} onClose={onClose} title="NEWS FEED" sub="Live · 124 today" width={460}>
      <div className="nf-filter">
        {filters.map(f=>(
          <span key={f} onClick={()=>setFilter(f)} className={'pill mono '+(f===filter?'on':'')}>{f}</span>
        ))}
      </div>
      <div className="nf-search">
        <span className="mono">⌕</span>
        <input placeholder="Filter headlines, tickers, sources…" className="mono"/>
      </div>
      <div className="nf-list">
        {filtered.map((n,i)=>(
          <div key={i} className="nf-row">
            <div className={'nf-imp '+n.imp}/>
            <div className="nf-meta">
              <span className="mono tm">{n.tm}</span>
              <span className="mono src">{n.src}</span>
              <span className="mono tag">· {n.tag}</span>
            </div>
            <div className="nf-t">{n.t}</div>
            <div className="nf-tic">
              {(n.tic||[]).map(t=>(<span key={t} className="mono">{t}</span>))}
            </div>
          </div>
        ))}
      </div>
    </Sidebar>
  );
}

// ─────────────────────────────────────────────────────────────────────
// SOCIAL FEED SIDEBAR — fintwit / sentiment stream
// ─────────────────────────────────────────────────────────────────────
function SocialFeedSidebar({ open, onClose }){
  const [src, setSrc] = sS('ALL');
  const sources = ['ALL','X / TWITTER','REDDIT','STOCKTWITS','DISCORD'];
  const posts = [
    {h:'cathie_w',     n:'Cathie Wood', src:'X', t:'Robotics + AI compute compounding — Blackwell ramp better than the buy-side models. Long.', s:'bull', tic:'NVDA', tm:'2m', en:412, rt:38},
    {h:'fundstrat',    n:'Tom Lee',     src:'X', t:'Setup looks like late-1998 — wall of worry, but breadth is improving. Year-end target unchanged.', s:'bull', tic:'SPY', tm:'14m', en:1240, rt:204},
    {h:'unusual_w',    n:'Unusual Whales', src:'X', t:'Dark pool prints at $1,228 NVDA — 480k size, off-exchange. Watching close above 1,235.', s:'bull', tic:'NVDA', tm:'22m', en:892, rt:142},
    {h:'wsbcoyote',    n:'WSB · Coyote', src:'REDDIT', t:'IV crush incoming on TSLA. Earnings Wednesday. Selling iron condors 240/280.', s:'neutral', tic:'TSLA', tm:'34m', en:218, rt:11},
    {h:'macroalf',     n:'Alfonso Peccatiello', src:'X', t:'Yield curve disinverting — historically equities have ~9 months before recession bites. Watch credit spreads.', s:'bear', tic:'TLT', tm:'1h', en:680, rt:96},
    {h:'beth_kindig',  n:'Beth Kindig', src:'X', t:'AMD MI300 traction at hyperscalers underrated — supply contracts trickle out next 60 days.', s:'bull', tic:'AMD', tm:'1h', en:512, rt:74},
    {h:'modest_money', n:'Modest Money', src:'STOCKTWITS', t:'AAPL bouncing off the 200d for the 3rd time. Loaded calls.', s:'bull', tic:'AAPL', tm:'2h', en:142, rt:8},
    {h:'mr_market',    n:'mr_market',   src:'DISCORD', t:'NQ futures showing accumulation overnight — 18,400 looking like new support.', s:'bull', tic:'NQ', tm:'3h', en:88, rt:14},
  ];
  const filtered = src==='ALL' ? posts : posts.filter(p => p.src.replace(/\s/g,'') === src.replace(/\s\/\s/g,'').replace(/\s/g,''));
  return (
    <Sidebar open={open} onClose={onClose} title="SOCIAL FEED" sub="FinTwit · Discord · Reddit" width={440}>
      <div className="sf-sentiment">
        <div className="sf-sent-row">
          <span className="mono lab">SENTIMENT</span>
          <div className="sf-bar">
            <div className="seg up" style={{width:'62%'}}/>
            <div className="seg neutral" style={{width:'21%'}}/>
            <div className="seg down" style={{width:'17%'}}/>
          </div>
        </div>
        <div className="sf-sent-num">
          <div><span className="mono lab">BULL</span><span className="mono v up">62%</span></div>
          <div><span className="mono lab">NEUTRAL</span><span className="mono v">21%</span></div>
          <div><span className="mono lab">BEAR</span><span className="mono v down">17%</span></div>
          <div><span className="mono lab">MENTIONS</span><span className="mono v">8.4K / hr</span></div>
        </div>
      </div>
      <div className="nf-filter">
        {sources.map(f=>(
          <span key={f} onClick={()=>setSrc(f)} className={'pill mono '+(f===src?'on':'')}>{f}</span>
        ))}
      </div>
      <div className="sf-list">
        {filtered.map((p,i)=>(
          <div key={i} className="sf-row">
            <div className="sf-av mono">{p.h.slice(0,2).toUpperCase()}</div>
            <div className="sf-body">
              <div className="sf-meta">
                <span className="nm">{p.n}</span>
                <span className="mono h">@{p.h}</span>
                <span className={'mono dot '+p.s}/>
                <span className="mono tic">${p.tic}</span>
                <span className="mono tm">{p.tm}</span>
              </div>
              <div className="sf-t">{p.t}</div>
              <div className="sf-en mono">
                <span>♥ {p.en}</span>
                <span>↻ {p.rt}</span>
                <span className="mono src">· {p.src}</span>
              </div>
            </div>
          </div>
        ))}
      </div>
    </Sidebar>
  );
}

// ─────────────────────────────────────────────────────────────────────
// PORTFOLIO SIDEBAR — quick account snapshot
// ─────────────────────────────────────────────────────────────────────
function PortfolioSidebar({ open, onClose, D }){
  const positions = D?.positions || [];
  const totMv = positions.reduce((a,b)=>a+b.mv,0);
  const totPl = positions.reduce((a,b)=>a+b.pl,0);
  return (
    <Sidebar open={open} onClose={onClose} title="PORTFOLIO" sub="Main · USD" width={420}>
      <div className="ps-hero">
        <div className="lab mono">EQUITY</div>
        <div className="bignum mono">$5.83M</div>
        <div className="ps-d mono up">+$98,420 · +1.72% today</div>
      </div>
      <div className="ps-kpis">
        {[
          {k:'CASH',     v:'$842K'},
          {k:'BUY POW',  v:'$5.83M'},
          {k:'MARGIN',   v:'$1.20M'},
          {k:'YTD P&L',  v:'+$842K', c:'up'},
          {k:'MTD P&L',  v:'+$184K', c:'up'},
          {k:'DAY P&L',  v:'+$98.4K', c:'up'},
        ].map(k=>(
          <div key={k.k} className="ps-kpi">
            <div className="lab mono">{k.k}</div>
            <div className={'mono v '+(k.c||'')}>{k.v}</div>
          </div>
        ))}
      </div>
      <div className="ps-sec">
        <div className="lab mono">EXPOSURE</div>
        <div className="ps-exp">
          <div className="bar">
            <div className="long" style={{width:'72%'}}/>
            <div className="short" style={{width:'18%'}}/>
          </div>
          <div className="legend mono">
            <span className="up">LONG 72%</span>
            <span className="muted">CASH 10%</span>
            <span className="down">SHORT 18%</span>
          </div>
        </div>
      </div>
      <div className="ps-sec">
        <div className="ps-secrow">
          <span className="lab mono">POSITIONS · 7 OPEN</span>
          <span className="mono ps-link">VIEW ALL →</span>
        </div>
        <div className="ps-pos">
          {positions.slice(0,7).map((p,i)=>{
            const w = Math.min(100, Math.abs(p.mv)/totMv*100);
            return (
              <div key={i} className="ps-prow">
                <div className="ps-pmeta">
                  <span className="mono sym">{p.sym}</span>
                  <span className="mono qty">{p.qty>0?'+':''}{p.qty}</span>
                </div>
                <div className="ps-pbar">
                  <div className={'fill '+(p.pl>=0?'up':'down')} style={{width:w+'%'}}/>
                </div>
                <div className={'mono pl '+(p.pl>=0?'up':'down')}>
                  {p.pl>=0?'+':''}${(Math.abs(p.pl)/1000).toFixed(1)}K
                </div>
              </div>
            );
          })}
        </div>
      </div>
      <div className="ps-foot">
        <div className="ps-fbtn mono">FLATTEN ALL</div>
        <div className="ps-fbtn primary mono">FULL PORTFOLIO →</div>
      </div>
    </Sidebar>
  );
}

// ─────────────────────────────────────────────────────────────────────
// DARK COMMAND PALETTE — like the spotlight reference
// ─────────────────────────────────────────────────────────────────────
function CommandPaletteV2({ open, onClose, onPick }){
  const [q, setQ] = sS('');
  const [sel, setSel] = sS(0);
  const [aiMode, setAiMode] = sS(false);
  const D = window.TERMINAL_DATA;

  const items = sM(()=>{
    const all = [
      {kind:'AI',  name:'Ask Gemma', meta:'Conversational AI · Tab to enter', desc:{h:'Ask Gemma', body:'Conversational assistant powered by fine-tuned Gemma 4.', bullets:['scanners in plain English','alert creation','context-aware answers']}, action:()=>{ setAiMode(true); }},
      {kind:'WIDGET', name:'Add widget · RSI Multi', meta:'Multi-symbol RSI scanner', desc:{h:'RSI Multi', body:'A scanner that surfaces extreme RSI readings across your watchlist in real time.', bullets:['threshold alerts','timeframe overlay','export to CSV']}, action:()=>onPick({kind:'page',page:'rsi'})},
      ...(D?.watchlist||[]).map(w=>({kind:'SYM', name:w.sym, meta:w.name, desc:{h:w.sym+' · '+w.name, body:`Last ${w.last.toFixed(2)} · ${w.pct>=0?'+':''}${w.pct.toFixed(2)}%`, bullets:['Open chart','Add to watchlist','Buy / Sell']}, action:()=>onPick({kind:'symbol',sym:w.sym})})),
      {kind:'CMD',  name:'Buy 100 NVDA @ MKT', meta:'Pre-fill ticket', desc:{h:'Quick order', body:'Pre-fills the order ticket with this exact intent.', bullets:['Confirm before submit','Routed Smart by default']}, action:()=>onPick({kind:'route',route:'apex'})},
      {kind:'CMD',  name:'Set alert · NVDA > 1,300', meta:'Price alert', desc:{h:'Create alert', body:'Persists across sessions. Notifies via terminal toast + email.', bullets:['Editable in Settings → Alerts']}, action:()=>{}},
      {kind:'PLAY', name:'Replay · 2024-08-05 selloff', meta:'Market replay', desc:{h:'Replay session', body:'Step through any historical session at variable speed.', bullets:['Realistic data feed','Paper-trade replays']}, action:()=>{}},
      {kind:'NAV',  name:'Open Portfolio',  meta:'Switch view', action:()=>onPick({kind:'route',route:'port'})},
      {kind:'NAV',  name:'Open Research',   meta:'Switch view', action:()=>onPick({kind:'route',route:'research'})},
      {kind:'NAV',  name:'Open Risk',       meta:'Switch view', action:()=>onPick({kind:'route',route:'risk'})},
      {kind:'NAV',  name:'Open Apex Layout',meta:'Multi-pane chart trader', action:()=>onPick({kind:'route',route:'apex'})},
      {kind:'NAV',  name:'Open Options Chain', meta:'Full chain page',  action:()=>onPick({kind:'route',route:'options'})},
      {kind:'SET',  name:'Settings',        meta:'Theme · Hotkeys · Account', action:()=>onPick({kind:'route',route:'settings'})},
      {kind:'CALC', name:'Calculator · Position size', meta:'Risk → shares', action:()=>{}},
    ];
    if (!q.trim()) return all.slice(0, 12);
    const Q = q.toUpperCase();
    return all.filter(a => a.name.toUpperCase().includes(Q) || a.meta.toUpperCase().includes(Q)).slice(0, 14);
  }, [q]);

  sE(()=>{ setSel(0); }, [q]);
  sE(()=>{
    if (!open) return;
    function k(e){
      if (e.key==='Escape') onClose();
      if (e.key==='ArrowDown'){ setSel(s=>Math.min(items.length-1, s+1)); e.preventDefault(); }
      if (e.key==='ArrowUp'){ setSel(s=>Math.max(0, s-1)); e.preventDefault(); }
      if (e.key==='Enter'){ items[sel] && items[sel].action(); onClose(); }
      if (e.key==='Tab'){ e.preventDefault(); setAiMode(true); }
    }
    window.addEventListener('keydown', k);
    return ()=>window.removeEventListener('keydown', k);
  }, [open, items, sel]);

  if (!open) return null;
  const cur = items[sel];

  return (
    <div className="cmdv2-overlay" onClick={onClose}>
      <div className={'cmdv2 '+(aiMode?'ai':'')} onClick={e=>e.stopPropagation()}>
        <div className="cmdv2-input">
          <span className="mono gt">▢</span>
          <input autoFocus value={q} onChange={e=>setQ(e.target.value)}
            placeholder={aiMode? 'Ask Gemma anything about the market…' : 'Search symbols, commands, widgets…  (Tab for AI)'}/>
          <div className={'cmdv2-ai-pill mono '+(aiMode?'on':'')} onClick={()=>setAiMode(a=>!a)}>
            <span>▢</span> Gemma 4
          </div>
        </div>
        <div className="cmdv2-pills">
          {[
            {k:'>',  l:'cmd', c:'mut'},
            {k:'@',  l:'sym', c:'acc'},
            {k:'#',  l:'play',c:'mag'},
            {k:'/',  l:'set', c:'mut'},
            {k:'?',  l:'ai',  c:'acc'},
            {k:'=',  l:'calc',c:'mut'},
          ].map(p=>(
            <span key={p.l} className={'cmdv2-pill '+p.c}><b className="mono">{p.k}</b><span className="mono">{p.l}</span></span>
          ))}
          <span className="cmdv2-ctx mono">Chart pane</span>
        </div>
        <div className="cmdv2-body">
          <div className="cmdv2-list">
            {items.map((it,i)=>(
              <div key={i} className={'cmdv2-row '+(i===sel?'on':'')+' '+(it.kind==='AI'?'ai':'')}
                onMouseEnter={()=>setSel(i)}
                onClick={()=>{ it.action(); onClose(); }}>
                <span className={'cmdv2-tag mono '+it.kind.toLowerCase()}>{it.kind}</span>
                <span className="cmdv2-nm">{it.name}</span>
                <span className="cmdv2-meta mono">{it.meta}</span>
                {i===sel && <span className="cmdv2-key mono">Tab</span>}
              </div>
            ))}
          </div>
          {cur?.desc && (
            <div className="cmdv2-detail">
              <div className="mono lab">{cur.kind}</div>
              <div className="hd">{cur.desc.h}</div>
              <div className="bd">{cur.desc.body}</div>
              {cur.desc.bullets && (
                <ul className="bul">
                  {cur.desc.bullets.map((b,i)=>(<li key={i}>{b}</li>))}
                </ul>
              )}
            </div>
          )}
        </div>
        <div className="cmdv2-foot mono">
          <span><b>↑↓/jk</b> nav</span>
          <span><b>⏎</b> run</span>
          <span><b>Tab</b> AI</span>
          <span><b>then</b> chain</span>
          <span><b>Esc</b> close</span>
          <span className="r">{items.length} results</span>
        </div>
      </div>
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────
// SETTINGS MODAL — dark editorial overlay (matches reference)
// ─────────────────────────────────────────────────────────────────────
function SettingsModal({ open, onClose, tweaks, setTweak }){
  const [tab, setTab] = sS('appearance');
  if (!open) return null;
  const tabs = [
    {k:'appearance', n:'Appearance'},
    {k:'chart',      n:'Chart'},
    {k:'trading',    n:'Trading'},
    {k:'shortcuts',  n:'Shortcuts'},
  ];
  const themes = [
    {k:'midnight', n:'Midnight', bg:'#0d0f12', fg:'#cfd6e0'},
    {k:'nord',     n:'Nord',     bg:'#1d242c', fg:'#9aa6b8'},
    {k:'monokai',  n:'Monokai',  bg:'#272822', fg:'#f8f8f2'},
    {k:'solarized',n:'Solarized',bg:'#073642', fg:'#93a1a1'},
    {k:'dracula',  n:'Dracula',  bg:'#282a36', fg:'#bd93f9'},
    {k:'gruvbox',  n:'Gruvbox',  bg:'#1d2021', fg:'#e8c170', sel:true},
    {k:'catppuccin',n:'Catppuccin',bg:'#1e1e2e', fg:'#cba6f7'},
    {k:'tokyonight',n:'Tokyo Night',bg:'#1a1b26', fg:'#7aa2f7'},
    {k:'kanagawa', n:'Kanagawa', bg:'#1f1f28', fg:'#dcd7ba'},
    {k:'everforest',n:'Everforest',bg:'#2d353b', fg:'#a7c080'},
    {k:'vesper',   n:'Vesper',   bg:'#101010', fg:'#ffc799'},
    {k:'rose-pine',n:'Rosé Pine',bg:'#191724', fg:'#ebbcba'},
    {k:'bauhaus',  n:'Bauhaus',  bg:'#fdf5e3', fg:'#1a1a1a', light:true},
    {k:'peach',    n:'Peach',    bg:'#fff2e6', fg:'#1a1a1a', light:true},
    {k:'ivory',    n:'Ivory',    bg:'#f3efe8', fg:'#1a1a1a', light:true, sel:false},
  ];
  const fonts = [
    {k:'JetBrains Mono', tag:'mono'},
    {k:'Inter',          tag:'mono'},
    {k:'Plus Jakarta',   tag:'mono'},
    {k:'Space Grotesk',  tag:'sans'},
    {k:'DM Sans',        tag:'sans', sel:true},
    {k:'Geist',          tag:'sans'},
  ];
  const sizes = ['60%','80%','100%','120%','140%','160%'];
  const [size, setSize] = sS('100%');
  const [font, setFont] = sS('DM Sans');
  const [paneHeader, setPaneHeader] = sS('Normal');
  const [compactToolbar, setCompactToolbar] = sS(false);
  const [autoHide, setAutoHide] = sS(false);

  return (
    <div className="setm-overlay" onClick={onClose}>
      <div className="setm" onClick={e=>e.stopPropagation()}>
        <div className="setm-head">
          <span className="mono ttl">SETTINGS</span>
          <span className="mono x" onClick={onClose}>×</span>
        </div>
        <div className="setm-tabs">
          {tabs.map(t=>(
            <span key={t.k} onClick={()=>setTab(t.k)} className={'tab '+(t.k===tab?'on':'')}>{t.n}</span>
          ))}
        </div>
        <div className="setm-body">
          {tab==='appearance' && (
            <>
              <div className="setm-sec mono">THEME</div>
              <div className="setm-themes">
                {themes.map(t=>(
                  <div key={t.k} className={'th-card '+(t.k==='gruvbox'?'sel':'')+(t.light?' light':'')}>
                    <div className="th-prev" style={{background:t.bg}}>
                      <CandleStripPreview fg={t.fg}/>
                    </div>
                    <div className="th-lab" style={{color:t.light?'#1a1a1a':'#cdd6e0'}}>{t.n}</div>
                  </div>
                ))}
              </div>

              <div className="setm-sec mono">FONT SCALE</div>
              <div className="setm-row">
                <div className="setm-lbl">Size</div>
                <div className="setm-rval mono">{size}%</div>
              </div>
              <div className="setm-segrow">
                {sizes.map(s=>(
                  <span key={s} onClick={()=>setSize(s)} className={'setm-sz '+(s===size?'on':'')}>{s}</span>
                ))}
              </div>

              <div className="setm-sec mono">FONT</div>
              <div className="setm-fonts">
                {fonts.map(f=>(
                  <div key={f.k} onClick={()=>setFont(f.k)} className={'fnt-card '+(f.k===font?'sel':'')}>
                    <div className="fnt-name" style={{fontFamily:f.k==='JetBrains Mono'?'JetBrains Mono':'inherit'}}>{f.k}</div>
                    <div className="fnt-meta mono">
                      <span className="t">{f.tag}</span>
                      <span className="s">0123 AAPL $9.50</span>
                    </div>
                  </div>
                ))}
              </div>

              <div className="setm-sec mono">LAYOUT</div>
              <div className="setm-row tg">
                <div className="setm-lbl">Compact Toolbar</div>
                <Sw on={compactToolbar} onChange={setCompactToolbar}/>
              </div>
              <div className="setm-row tg">
                <div className="setm-lbl">Auto-Hide Toolbar</div>
                <Sw on={autoHide} onChange={setAutoHide}/>
              </div>
              <div className="setm-row">
                <div className="setm-lbl">Pane Headers</div>
                <div className="setm-segr">
                  {['Expanded','Normal','Compact'].map(o=>(
                    <span key={o} onClick={()=>setPaneHeader(o)} className={'sg '+(o===paneHeader?'on':'')}>{o}</span>
                  ))}
                </div>
              </div>
            </>
          )}
          {tab==='chart' && (
            <>
              <div className="setm-sec mono">CANDLES</div>
              <div className="setm-row tg">
                <div className="setm-lbl">Show volume</div>
                <Sw on={tweaks.showVol} onChange={v=>setTweak('showVol',v)}/>
              </div>
              <div className="setm-row tg">
                <div className="setm-lbl">Moving averages</div>
                <Sw on={tweaks.showMA} onChange={v=>setTweak('showMA',v)}/>
              </div>
              <div className="setm-row tg">
                <div className="setm-lbl">Bollinger bands</div>
                <Sw on={tweaks.showBB} onChange={v=>setTweak('showBB',v)}/>
              </div>
              <div className="setm-row tg">
                <div className="setm-lbl">Editorial OHLC callouts</div>
                <Sw on={tweaks.dotmatrix} onChange={v=>setTweak('dotmatrix',v)}/>
              </div>
              <div className="setm-sec mono">DEFAULTS</div>
              <div className="setm-row">
                <div className="setm-lbl">Default timeframe</div>
                <div className="setm-segr">
                  {['1m','5m','15m','1H','1D'].map(o=>(
                    <span key={o} className={'sg '+(o==='5m'?'on':'')}>{o}</span>
                  ))}
                </div>
              </div>
            </>
          )}
          {tab==='trading' && (
            <>
              <div className="setm-sec mono">ORDER ROUTING</div>
              <div className="setm-row">
                <div className="setm-lbl">Default route</div>
                <div className="setm-segr">
                  {['Smart','NSDQ','ARCA','IEX'].map(o=>(
                    <span key={o} className={'sg '+(o==='Smart'?'on':'')}>{o}</span>
                  ))}
                </div>
              </div>
              <div className="setm-row">
                <div className="setm-lbl">Default order type</div>
                <div className="setm-segr">
                  {['MKT','LMT','STP'].map(o=>(
                    <span key={o} className={'sg '+(o==='LMT'?'on':'')}>{o}</span>
                  ))}
                </div>
              </div>
              <div className="setm-row tg">
                <div className="setm-lbl">Confirm before submit</div>
                <Sw on={true} onChange={()=>{}}/>
              </div>
              <div className="setm-row tg">
                <div className="setm-lbl">Auto-bracket on entry</div>
                <Sw on={false} onChange={()=>{}}/>
              </div>
            </>
          )}
          {tab==='shortcuts' && (
            <div className="setm-keys">
              {[
                ['Open palette',         '⌘ K'],
                ['Quick search',         '/'],
                ['Buy ticket',           'B'],
                ['Sell ticket',          'S'],
                ['Cancel orders',        '⌘ ⇧ X'],
                ['Flatten symbol',       '⌘ ⇧ F'],
                ['Flatten all',          '⌘ ⌥ F'],
                ['Toggle dotmatrix',     'D'],
                ['Cycle timeframe',      'T'],
                ['Toggle theme',         '⌘ J'],
                ['Toggle MA',            'M'],
                ['Toggle Bollinger',     'V'],
                ['Open news',            'N'],
                ['Open social',          '⌘ ⇧ S'],
                ['Open chain',           '⌘ O'],
                ['Open portfolio',       'P'],
              ].map(([n,k])=>(
                <div key={n} className="setm-kr">
                  <span className="n">{n}</span>
                  <span className="mono k">{k}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function Sw({ on, onChange }){
  return (
    <div className={'sw '+(on?'on':'')} onClick={()=>onChange(!on)}>
      <div className="kn"/>
    </div>
  );
}

function CandleStripPreview({ fg }){
  // small candlestick strip used in theme cards
  const candles = sM(()=>{
    const out=[]; let p=10;
    for (let i=0;i<14;i++){
      const o=p, c=p+(Math.sin(i*1.3)*1.4 + (Math.random()-0.5));
      const h=Math.max(o,c)+Math.random()*0.6+0.2;
      const l=Math.min(o,c)-Math.random()*0.6-0.2;
      out.push({o,c,h,l}); p=c;
    }
    return out;
  }, []);
  const min = Math.min(...candles.map(c=>c.l));
  const max = Math.max(...candles.map(c=>c.h));
  const span = max-min;
  const W=120, H=44;
  const y = v => 4 + (1-(v-min)/span)*(H-8);
  return (
    <svg viewBox={`0 0 ${W} ${H}`} style={{width:'100%', height:'100%', display:'block'}}>
      {candles.map((c,i)=>{
        const x = 6 + i*(W-12)/(candles.length-1);
        const isUp = c.c>=c.o;
        const col = isUp ? '#7fb069' : '#c2532a';
        const top = Math.min(y(c.o), y(c.c));
        const bh = Math.max(1, Math.abs(y(c.c)-y(c.o)));
        return (
          <g key={i}>
            <line x1={x} x2={x} y1={y(c.h)} y2={y(c.l)} stroke={col} strokeWidth="0.6"/>
            <rect x={x-2} y={top} width="4" height={bh} fill={col}/>
          </g>
        );
      })}
    </svg>
  );
}

Object.assign(window, {
  Sidebar, OptionsChainSidebar, NewsFeedSidebar, SocialFeedSidebar, PortfolioSidebar,
  CommandPaletteV2, SettingsModal,
});
