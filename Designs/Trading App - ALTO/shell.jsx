// Shell — single top row (logo · nav tabs · timeframe + tools · search · user),
// rounded right-side watchlist (Spotify-style), left toggleable DOM ladder, floating order ticket.

const { useState, useMemo, useEffect, useRef } = React;

function fmt(n, d = 2) {
  return Number(n).toLocaleString('en-US', { minimumFractionDigits: d, maximumFractionDigits: d });
}
function fmtPct(p, d = 2) {
  const sign = p >= 0 ? '+' : '−';
  return `${sign}${Math.abs(p).toFixed(d)}%`;
}
function fmtSigned(n, d = 2) {
  const sign = n >= 0 ? '+' : '−';
  return `${sign}$${Math.abs(n).toLocaleString('en-US', { minimumFractionDigits: d, maximumFractionDigits: d })}`;
}

// Single top row: brand · tabs · timeframe + tools (contextual) · search · user
function TopBar({ active, onNav, tools, onToggleDom, onToggleOrder, domOpen, orderOpen }) {
  const tabs = [
    { id: 'tiles',     label: 'Dashboard', icon: <Ic.Home size={13}/> },
    { id: 'apex',      label: 'Markets',   icon: <Ic.Chart size={13}/> },
    { id: 'portfolio', label: 'Portfolio', icon: <Ic.Briefcase size={13}/> },
    { id: 'plays',     label: 'Plays',     icon: <Ic.Bolt size={13}/> },
    { id: 'screener',  label: 'Screener',  icon: <Ic.Filter size={13}/> },
    { id: 'news',      label: 'News',      icon: <Ic.News size={13}/> },
    { id: 'alerts',    label: 'Alerts',    icon: <Ic.Bell size={13}/> },
  ];
  return (
    <div className="topbar">
      <div className="tb-left">
        <div className="brand">
          <span className="brand-mark"/>
          <span className="brand-name">Cadence</span>
        </div>
        <div className="tb-divider"/>
        <div className="tabs">
          {tabs.map(t => (
            <button key={t.id}
              className={'tab' + (active === t.id ? ' active' : '')}
              onClick={() => onNav(t.id)}>
              {t.icon}{t.label}
            </button>
          ))}
        </div>
        <div className="tb-divider"/>
        {tools && <div className="tool-strip">{tools}</div>}
      </div>

      <div className="tb-right">
        <div className="search-pill">
          <Ic.Search size={14}/>
          <input placeholder="Symbol, news, function…"/>
          <span className="kbd">⌘K</span>
        </div>
        <button className="search-icon" aria-label="Search"><Ic.Search size={15}/></button>
        {active === 'apex' && (
          <>
            <button className={'panel-tog' + (domOpen ? ' active' : '')} onClick={onToggleDom} title="Depth ladder">
              <Ic.List size={14}/> <span className="pt-l">DOM</span>
            </button>
            <button className={'panel-tog' + (orderOpen ? ' active' : '')} onClick={onToggleOrder} title="Order ticket">
              <Ic.Plus size={14}/> <span className="pt-l">Ticket</span>
            </button>
          </>
        )}
        <div className="tb-divider mkt-div"/>
        <span className="market-state">
          <span className="dot-live"/><span className="ms-l">NYSE OPEN · </span>06:42:18 ET
        </span>
        <div className="avatar">JM</div>
      </div>

      <style>{`
        .topbar {
          height: 56px; flex: 0 0 auto;
          display: flex; align-items: center; justify-content: space-between;
          padding: 0 16px; gap: 16px;
          background: #000; border-bottom: 1px solid var(--line);
        }
        .tb-left, .tb-right { display: flex; align-items: center; gap: 10px; min-width: 0; }
        .tb-left { flex: 1; min-width: 0; overflow: hidden; }
        .tb-right { flex: 0 0 auto; }
        .tb-divider { width: 1px; height: 22px; background: var(--line); flex: 0 0 auto; }

        .brand { display: flex; align-items: center; gap: 8px; padding-right: 4px; }
        .brand-mark {
          width: 22px; height: 22px; border-radius: 6px;
          background: radial-gradient(circle at 30% 30%, var(--green), #0d6e35);
          box-shadow: inset 0 0 0 1px rgba(255,255,255,0.12);
        }
        .brand-name { font: 700 16px 'Inter Tight', sans-serif; letter-spacing: -0.025em; color: #fff; }

        .tabs { display: flex; gap: 2px; overflow-x: auto; scrollbar-width: none; flex: 0 1 auto; min-width: 0; }
        .tabs::-webkit-scrollbar { display: none; }
        .tab { white-space: nowrap; flex: 0 0 auto; }
        .tab {
          height: 32px; padding: 0 12px; border-radius: 999px; background: transparent;
          color: var(--text-secondary); border: 0; cursor: pointer;
          font: 600 13px 'Inter', sans-serif; letter-spacing: -0.01em;
          transition: color .12s ease, background .12s ease;
          display: inline-flex; align-items: center; gap: 6px;
        }
        .tab svg { opacity: 0.7; }
        .tab.active svg { opacity: 1; }
        .tab:hover { color: var(--text-primary); background: rgba(255,255,255,0.05); }
        .tab.active { background: rgba(255,255,255,0.10); color: #fff; }

        .tool-strip {
          display: flex; align-items: center; gap: 2px;
          flex-wrap: nowrap; overflow: hidden;
          background: rgba(255,255,255,0.04);
          border-radius: 999px; padding: 3px;
        }
        .tool-strip > * { flex: 0 0 auto; }

        .panel-tog {
          height: 32px; padding: 0 12px; border-radius: 999px;
          background: transparent; border: 1px solid var(--line);
          color: var(--text-secondary); cursor: pointer;
          font: 600 12px 'Inter', sans-serif; letter-spacing: -0.005em;
          display: inline-flex; align-items: center; gap: 6px;
        }
        .panel-tog:hover { color: #fff; border-color: var(--line-strong); }
        .panel-tog.active { background: var(--green); color: #000; border-color: var(--green); }

        .market-state {
          display: inline-flex; align-items: center; gap: 8px;
          font: 500 12px 'Inter', sans-serif; color: var(--text-secondary);
          letter-spacing: -0.005em;
        }
        .dot-live { width: 7px; height: 7px; border-radius: 50%; background: var(--green); }

        .search-pill {
          display: flex; align-items: center; gap: 8px;
          height: 36px; padding: 0 12px; width: 240px;
          background: #1a1a1a; border-radius: 999px; color: var(--text-secondary);
          border: 1px solid transparent; flex: 0 0 auto;
        }
        .search-icon {
          display: none; width: 36px; height: 36px; border-radius: 50%;
          background: #1a1a1a; border: 0; color: var(--text-secondary); cursor: pointer;
          align-items: center; justify-content: center;
        }
        .search-icon:hover { color: #fff; }

        /* Responsive collapse */
        @media (max-width: 1340px) {
          .ms-l { display: none; }
        }
        @media (max-width: 1240px) {
          .search-pill { width: 200px; }
          .search-pill .kbd { display: none; }
        }
        @media (max-width: 1140px) {
          .market-state, .mkt-div { display: none; }
        }
        @media (max-width: 1020px) {
          .search-pill { display: none; }
          .search-icon { display: inline-flex; }
          .pt-l { display: none; }
          .panel-tog { padding: 0 10px; }
        }
        @media (max-width: 880px) {
          .tab { padding: 0 8px; }
          .tab span:not(.kbd) { display: none; }
          .tab svg { opacity: 1; }
          .topbar { padding: 0 12px; gap: 10px; }
          .brand-name { display: none; }
        }
        .search-pill:focus-within { border-color: var(--green); }
        .search-pill input {
          flex: 1; background: transparent; border: 0; outline: 0; color: var(--text-primary);
          font: 500 13px 'Inter', sans-serif; letter-spacing: -0.005em;
        }
        .search-pill input::placeholder { color: var(--text-tertiary); }
        .kbd {
          font: 600 10px 'Inter Tight', sans-serif; color: var(--text-tertiary);
          padding: 2px 6px; border: 1px solid var(--line); border-radius: 4px;
        }

        .avatar {
          width: 32px; height: 32px; border-radius: 50%;
          background: linear-gradient(135deg, #1ed760, #0a8c3f);
          display: inline-flex; align-items: center; justify-content: center;
          color: #052b14; font: 800 11px 'Inter Tight', sans-serif; letter-spacing: 0.02em;
          cursor: pointer;
        }
      `}</style>
    </div>
  );
}

// Reusable tool button — pill style consistent with Spotify chips
function ToolBtn({ children, active, onClick }) {
  return (
    <button onClick={onClick} className={'tool-btn' + (active ? ' active' : '')}>
      {children}
      <style>{`
        .tool-btn {
          height: 26px; padding: 0 12px; border-radius: 999px;
          background: transparent; border: 0; color: var(--text-secondary); cursor: pointer;
          font: 600 12px 'Inter', sans-serif; letter-spacing: -0.005em;
          display: inline-flex; align-items: center; gap: 4px;
          white-space: nowrap;
        }
        .tool-btn:hover { color: #fff; }
        .tool-btn.active { background: #fff; color: #000; }
      `}</style>
    </button>
  );
}

// Mini sparkline used inside cards / rows
function MiniSpark({ data, color, w=80, h=22 }) {
  const min = Math.min(...data), max = Math.max(...data);
  const range = max - min || 1;
  const pts = data.map((v, i) => [(i / (data.length-1)) * w, h - ((v - min) / range) * (h-2) - 1]);
  return (
    <svg viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none" style={{ width: w, height: h, display:'block' }}>
      <path d={pts.map((p,i)=>(i===0?'M':'L')+p[0]+','+p[1]).join(' ')} stroke={color} strokeWidth="1.4" fill="none" strokeLinecap="round" strokeLinejoin="round"/>
    </svg>
  );
}

// Conviction dots
function Conviction({ n }) {
  return (
    <span style={{ display:'inline-flex', gap:2 }}>
      {Array.from({length:5}).map((_,i)=>(
        <span key={i} style={{ width:5, height:5, borderRadius:50, background: i<n ? 'var(--green)' : 'rgba(255,255,255,0.15)' }}/>
      ))}
    </span>
  );
}

// Compact play card for sidebar
function PlayCardMini({ p, onPick }) {
  return (
    <button className="pc-mini" onClick={() => onPick && onPick(p.sym)}>
      <div className="pc-mini-head">
        <span className={'pc-side ' + p.side.toLowerCase()}>
          {p.side === 'LONG' ? <Ic.TrendUp size={10}/> : <Ic.TrendDown size={10}/>} {p.side}
        </span>
        <span className="pc-sym">{p.sym}</span>
        <span className="pc-mini-rr num">{p.rr.toFixed(1)}R</span>
      </div>
      <div className="pc-mini-name">{p.name}</div>
      <div className="pc-mini-foot">
        <Conviction n={p.conviction}/>
        <MiniSpark data={p.series} color={p.accent} w={70} h={18}/>
        <span className="pc-mini-time"><Ic.Clock size={10}/> {p.posted}</span>
      </div>
      <style>{`
        .pc-mini {
          width: 100%; text-align:left; cursor:pointer;
          background: rgba(255,255,255,0.03); border-radius: 10px;
          border: 1px solid var(--line);
          padding: 12px; margin-bottom: 8px;
          display: flex; flex-direction: column; gap: 8px;
          position: relative; overflow: hidden;
          transition: background .12s ease, border-color .12s ease;
        }
        .pc-mini:hover { background: rgba(255,255,255,0.06); border-color: var(--line-strong); }
        .pc-mini-head { display:flex; align-items:center; gap:8px; }
        .pc-side {
          display:inline-flex; align-items:center; gap:3px;
          font: 700 9px 'Inter', sans-serif; letter-spacing: 0.06em;
          padding: 2px 6px; border-radius: 4px;
        }
        .pc-side.long { color: var(--green); background: var(--green-bg); }
        .pc-side.short { color: var(--red); background: var(--red-bg); }
        .pc-sym { font: 700 13px 'Inter Tight', sans-serif; letter-spacing: -0.02em; color:#fff; }
        .pc-mini-rr {
          margin-left:auto;
          font: 700 11px 'Inter Tight', sans-serif; letter-spacing: -0.01em;
          color: var(--green); padding: 2px 6px; background: var(--green-bg); border-radius: 4px;
        }
        .pc-mini-name { font: 600 12px 'Inter', sans-serif; color: var(--text-secondary); letter-spacing: -0.005em; }
        .pc-mini-foot { display:flex; align-items:center; justify-content: space-between; }
        .pc-mini-time {
          display:inline-flex; align-items:center; gap:4px;
          font: 500 10px 'Inter', sans-serif; color: var(--text-tertiary); letter-spacing: -0.005em;
        }
      `}</style>
    </button>
  );
}

// Right-side sidebar — tabs: Watchlist | Plays | Alerts
function Sidebar({ activeSym, onPickSym }) {
  const [tab, setTab] = useState('watchlist');
  const [filter, setFilter] = useState('All');
  const filters = ['All', 'Equities', 'Crypto', 'FX'];

  const tabs = [
    { id:'watchlist', label:'Watchlist', icon: <Ic.Library size={13}/>, count: WATCHLIST.length },
    { id:'plays',     label:'Plays',     icon: <Ic.Bolt size={13}/>,    count: PLAYS.length },
    { id:'alerts',    label:'Alerts',    icon: <Ic.Bell size={13}/>,    count: ALERTS.filter(a=>a.status==='armed').length },
  ];

  return (
    <aside className="sidebar">
      <div className="sb-card">
        <div className="sb-tabs">
          {tabs.map(t => (
            <button key={t.id} className={'sb-tab' + (tab===t.id?' active':'')} onClick={() => setTab(t.id)}>
              {t.icon}
              <span>{t.label}</span>
              <span className="sb-tab-count">{t.count}</span>
            </button>
          ))}
        </div>

        {tab === 'watchlist' && <>
          <div className="sb-chips">
            {filters.map(f => (
              <button key={f} className={'sl-chip' + (filter === f ? ' active' : '')} onClick={() => setFilter(f)}>{f}</button>
            ))}
            <button className="icon-btn-sm" style={{ marginLeft:'auto' }} aria-label="Add"><Ic.Plus size={14}/></button>
          </div>
          <div className="sb-search">
            <Ic.Search size={12}/>
            <input placeholder="Search in watchlist…"/>
          </div>
          <div className="sb-cols">
            <span>Symbol</span>
            <span style={{ textAlign: 'right' }}>Last</span>
            <span style={{ textAlign: 'right' }}>Chg %</span>
          </div>
          <div className="sb-list">
            {WATCHLIST.map(w => {
              const up = w.chg >= 0;
              const active = activeSym === w.sym;
              return (
                <button key={w.sym} className={'sb-row' + (active ? ' active' : '')} onClick={() => onPickSym(w.sym)}>
                  <div className="sb-meta">
                    <div className="sb-sym">{w.sym}</div>
                    <div className="sb-name">{w.name}</div>
                  </div>
                  <div className="sb-px num">{w.px.toLocaleString('en-US', {minimumFractionDigits:2, maximumFractionDigits:2})}</div>
                  <div className={'sb-chg num ' + (up ? 'pos' : 'neg')}>
                    {up ? '+' : '−'}{Math.abs(w.chg).toFixed(2)}%
                  </div>
                </button>
              );
            })}
          </div>
        </>}

        {tab === 'plays' && <>
          <div className="sb-chips">
            <button className="sl-chip active">All <span style={{opacity:.6}}>· {PLAYS.length}</span></button>
            <button className="sl-chip">Long</button>
            <button className="sl-chip">Short</button>
            <button className="icon-btn-sm" style={{ marginLeft:'auto' }} aria-label="New"><Ic.Plus size={14}/></button>
          </div>
          <div className="sb-list" style={{ padding:'8px 12px 12px' }}>
            {PLAYS.map(p => (
              <PlayCardMini key={p.id} p={p} onPick={onPickSym}/>
            ))}
          </div>
        </>}

        {tab === 'alerts' && <>
          <div className="sb-chips">
            <button className="sl-chip active">Armed</button>
            <button className="sl-chip">Triggered</button>
            <button className="sl-chip">All</button>
            <button className="icon-btn-sm" style={{ marginLeft:'auto' }} aria-label="New"><Ic.Plus size={14}/></button>
          </div>
          <div className="sb-list" style={{ padding:'4px 12px 12px' }}>
            {ALERTS.map(a => (
              <div key={a.id} className="sb-alert">
                <div className={'sb-alert-icon ' + a.status}>
                  {a.status==='triggered' ? <Ic.Alert size={12}/> : <Ic.Bell size={12}/>}
                </div>
                <div className="sb-alert-body">
                  <div className="sb-alert-head">
                    <span className="pc-sym">{a.sym}</span>
                    <span className="sb-alert-kind">{a.kind}</span>
                  </div>
                  <div className="sb-alert-cond">{a.cond} <b className="num">{a.val}</b></div>
                  <div className="sb-alert-meta"><Ic.Clock size={9}/> {a.set}</div>
                </div>
              </div>
            ))}
          </div>
        </>}
      </div>

      <style>{`
        .sidebar {
          width: 296px; flex: 0 0 296px;
          padding: 8px 8px 8px 0;
          min-height: 0; display: flex;
        }
        .sb-card {
          flex: 1; min-height: 0;
          background: var(--bg-elev-1);
          border-radius: 12px;
          display: flex; flex-direction: column;
          overflow: hidden;
        }
        .sb-tabs {
          display: flex; gap: 2px; padding: 8px;
          flex: 0 0 auto; border-bottom: 1px solid var(--line);
        }
        .sb-tab {
          flex: 1; height: 34px; border-radius: 8px;
          background: transparent; border: 0; cursor: pointer;
          color: var(--text-secondary);
          display: inline-flex; align-items: center; justify-content: center; gap: 6px;
          font: 600 12px 'Inter', sans-serif; letter-spacing: -0.005em;
        }
        .sb-tab:hover { color:#fff; background: rgba(255,255,255,0.04); }
        .sb-tab.active { color: #fff; background: rgba(255,255,255,0.10); }
        .sb-tab-count {
          font: 700 9px 'Inter Tight', sans-serif; letter-spacing: 0;
          color: var(--text-tertiary);
          padding: 1px 5px; border-radius: 999px; background: rgba(255,255,255,0.08);
          min-width: 16px; text-align: center;
        }
        .sb-tab.active .sb-tab-count { color: #000; background: var(--green); }

        .sb-alert {
          display: grid; grid-template-columns: 28px 1fr; gap: 10px;
          padding: 10px; border-radius: 8px; margin-bottom: 4px;
          background: rgba(255,255,255,0.02);
          border: 1px solid var(--line);
        }
        .sb-alert-icon {
          width:28px; height:28px; border-radius:8px;
          display:inline-flex; align-items:center; justify-content:center;
          background: rgba(255,255,255,0.06); color: var(--text-secondary);
        }
        .sb-alert-icon.triggered { background: var(--green-bg); color: var(--green); }
        .sb-alert-head { display:flex; align-items:center; gap:8px; }
        .sb-alert-kind {
          font: 600 9px 'Inter', sans-serif; letter-spacing: 0.06em; text-transform: uppercase;
          color: var(--text-tertiary); padding: 1px 6px; border-radius: 4px;
          background: rgba(255,255,255,0.04);
        }
        .sb-alert-cond { font: 500 12px 'Inter', sans-serif; color: var(--text-secondary); margin-top: 2px; letter-spacing: -0.005em; }
        .sb-alert-cond b { color: #fff; font-weight: 600; }
        .sb-alert-meta {
          display:inline-flex; align-items:center; gap:4px;
          font: 500 10px 'Inter', sans-serif; color: var(--text-tertiary); margin-top: 4px;
        }
        .sb-chips {
          display: flex; align-items:center; gap: 6px; padding: 12px 12px 8px; flex: 0 0 auto;
          overflow-x: auto; scrollbar-width: none;
        }
        .sb-chips::-webkit-scrollbar { display: none; }
        .sl-chip {
          height: 28px; padding: 0 12px; border-radius: 999px;
          background: rgba(255,255,255,0.06); color: #fff;
          border: 0; cursor: pointer; white-space: nowrap;
          font: 600 12px 'Inter', sans-serif; letter-spacing: -0.005em;
        }
        .sl-chip:hover { background: rgba(255,255,255,0.10); }
        .sl-chip.active { background: #fff; color: #000; }

        .sb-search {
          display: flex; align-items: center; gap: 8px;
          margin: 0 12px 8px; padding: 0 12px;
          height: 32px;
          background: rgba(255,255,255,0.04); border-radius: 8px;
          color: var(--text-secondary); flex: 0 0 auto;
        }
        .sb-search input {
          flex: 1; background: transparent; border: 0; outline: 0; color: var(--text-primary);
          font: 500 12px 'Inter', sans-serif; letter-spacing: -0.005em;
        }
        .sb-search input::placeholder { color: var(--text-tertiary); }

        .sb-cols {
          display: grid; grid-template-columns: 1fr 70px 60px; gap: 12px;
          padding: 8px 16px; margin: 0 4px;
          font: 600 10px 'Inter', sans-serif; letter-spacing: 0.06em; text-transform: uppercase;
          color: var(--text-tertiary);
          border-bottom: 1px solid var(--line);
          flex: 0 0 auto;
        }

        .sb-list { flex: 1; min-height: 0; overflow-y: auto; padding: 4px; }
        .sb-row {
          width: 100%;
          display: grid; grid-template-columns: 1fr 70px 60px; gap: 12px;
          align-items: center; padding: 8px 12px;
          background: transparent; border: 0; cursor: pointer; text-align: left;
          color: var(--text-primary); border-radius: 6px;
          transition: background .1s ease;
        }
        .sb-row:hover { background: rgba(255,255,255,0.05); }
        .sb-row.active { background: rgba(255,255,255,0.10); }
        .sb-meta { min-width: 0; }
        .sb-sym { font: 700 13px 'Inter', sans-serif; letter-spacing: -0.01em; color: #fff; }
        .sb-name { font: 500 11px 'Inter', sans-serif; color: var(--text-secondary); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 140px; }
        .sb-px { font: 600 12px 'Inter Tight', sans-serif; color: var(--text-secondary); text-align: right; }
        .sb-chg { font: 600 12px 'Inter Tight', sans-serif; text-align: right; }

        .icon-btn-sm {
          width: 26px; height: 26px; border-radius: 50%;
          background: transparent; border: 0; color: var(--text-secondary); cursor: pointer;
          display: inline-flex; align-items: center; justify-content: center;
        }
        .icon-btn-sm:hover { background: rgba(255,255,255,0.08); color: #fff; }
      `}</style>
    </aside>
  );
}

// Left-side DOM ladder — toggleable, replaces the right rail order book
function DomLadder({ activeSym, onClose }) {
  const sym = WATCHLIST.find(w => w.sym === activeSym) || WATCHLIST[0];
  const mid = sym.px;
  const rows = [];
  // 12 asks above, 12 bids below
  for (let i = 12; i >= 1; i--) {
    const px = mid + i * 0.05;
    const sz = Math.floor(50 + Math.abs(Math.sin(i * 1.3)) * 1400);
    rows.push({ side: 'ask', px, sz });
  }
  for (let i = 1; i <= 12; i++) {
    const px = mid - i * 0.05;
    const sz = Math.floor(50 + Math.abs(Math.cos(i * 1.1)) * 1400);
    rows.push({ side: 'bid', px, sz });
  }
  const maxSz = Math.max(...rows.map(r => r.sz));

  return (
    <aside className="dom">
      <div className="dom-card">
        <div className="dom-head">
          <div className="dom-title">
            <Ic.List size={14}/>
            <span>DOM · {sym.sym}</span>
          </div>
          <button className="icon-btn-sm" onClick={onClose} aria-label="Close DOM"><Ic.X size={14}/></button>
        </div>
        <div className="dom-cols">
          <span>Bid Sz</span>
          <span>Price</span>
          <span style={{ textAlign:'right' }}>Ask Sz</span>
        </div>
        <div className="dom-rows">
          {rows.map((r, i) => {
            const isAsk = r.side === 'ask';
            const w = (r.sz / maxSz) * 100;
            // First bid row (i = 12) is the spread row marker
            const isSpread = i === 12;
            return (
              <React.Fragment key={i}>
                {isSpread && (
                  <div className="dom-spread">
                    <span>SPREAD</span>
                    <span className="num">0.02</span>
                    <span className="num">{mid.toFixed(2)}</span>
                  </div>
                )}
                <div className={'dom-row ' + r.side}>
                  <div className="dom-cell sz">
                    {!isAsk && <div className="dom-bg" style={{ width: `${w}%` }}/>}
                    <span className="num" style={{ position: 'relative', zIndex: 1 }}>{!isAsk ? r.sz.toLocaleString() : ''}</span>
                  </div>
                  <div className={'dom-cell px ' + (isAsk ? 'neg' : 'pos')}>
                    <span className="num">{r.px.toFixed(2)}</span>
                  </div>
                  <div className="dom-cell sz" style={{ textAlign: 'right' }}>
                    {isAsk && <div className="dom-bg ask" style={{ width: `${w}%` }}/>}
                    <span className="num" style={{ position: 'relative', zIndex: 1 }}>{isAsk ? r.sz.toLocaleString() : ''}</span>
                  </div>
                </div>
              </React.Fragment>
            );
          })}
        </div>
      </div>

      <style>{`
        .dom { width: 240px; flex: 0 0 240px; padding: 8px 0 8px 8px; min-height: 0; display: flex; }
        .dom-card {
          flex: 1; min-height: 0;
          background: var(--bg-elev-1);
          border-radius: 12px;
          display: flex; flex-direction: column; overflow: hidden;
        }
        .dom-head {
          display: flex; align-items: center; justify-content: space-between;
          padding: 12px 14px 8px;
          flex: 0 0 auto;
        }
        .dom-title {
          display: flex; align-items: center; gap: 8px;
          font: 700 14px 'Inter Tight', sans-serif; letter-spacing: -0.02em; color: #fff;
        }
        .dom-title svg { color: var(--text-secondary); }
        .dom-cols {
          display: grid; grid-template-columns: 1fr 70px 1fr; gap: 6px;
          padding: 6px 12px;
          font: 600 9px 'Inter', sans-serif; letter-spacing: 0.06em; text-transform: uppercase;
          color: var(--text-tertiary); border-bottom: 1px solid var(--line);
          flex: 0 0 auto;
        }
        .dom-cols > span:nth-child(2) { text-align: center; }
        .dom-rows { flex: 1; min-height: 0; overflow-y: auto; padding: 2px 0; }
        .dom-row {
          display: grid; grid-template-columns: 1fr 70px 1fr; gap: 6px;
          padding: 3px 12px;
          font-size: 12px;
          align-items: center;
        }
        .dom-row:hover { background: rgba(255,255,255,0.04); }
        .dom-cell { position: relative; }
        .dom-cell.px { text-align: center; font: 600 12px 'Inter Tight', sans-serif; }
        .dom-cell.sz { color: var(--text-secondary); font: 500 12px 'Inter Tight', sans-serif; }
        .dom-bg {
          position: absolute; left: 0; top: -1px; bottom: -1px;
          background: var(--green-bg); z-index: 0;
        }
        .dom-bg.ask { left: auto; right: 0; background: var(--red-bg); }
        .dom-spread {
          display: grid; grid-template-columns: 1fr 70px 1fr; gap: 6px;
          padding: 6px 12px;
          background: rgba(255,255,255,0.04);
          border-top: 1px solid var(--line); border-bottom: 1px solid var(--line);
          font: 700 9px 'Inter', sans-serif; letter-spacing: 0.06em; text-transform: uppercase;
          color: var(--text-tertiary);
        }
        .dom-spread > span:nth-child(2) { text-align: center; color: #fff; font-family: 'Inter Tight'; }
        .dom-spread > span:nth-child(3) { text-align: right; color: #fff; font-family: 'Inter Tight'; }
      `}</style>
    </aside>
  );
}

// Floating order ticket — draggable, anchored bottom-right above sidebar
function OrderTicket({ activeSym, onClose }) {
  const sym = WATCHLIST.find(w => w.sym === activeSym) || WATCHLIST[0];
  const [side, setSide] = useState('buy');
  const [qty, setQty] = useState(100);
  const [type, setType] = useState('Limit');
  const [tif, setTif] = useState('Day');
  const [limit, setLimit] = useState((sym.px - 0.05).toFixed(2));
  const [typeMenu, setTypeMenu] = useState(false);
  const [tifMenu, setTifMenu] = useState(false);
  const typeRef = useRef(null);
  const tifRef = useRef(null);

  const bid = (sym.px - 0.05);
  const ask = (sym.px + 0.05);
  const fillPx = type === 'Market' ? sym.px : parseFloat(limit) || sym.px;
  const est = qty * fillPx;
  const up = sym.chg >= 0;
  const sideColor = side === 'buy' ? 'var(--green)' : 'var(--red)';
  const typeShort = { 'Market':'MKT', 'Limit':'LMT', 'Stop':'STP', 'Stop‑Lim':'STP·L' };
  const isTypeOverflow = type === 'Stop' || type === 'Stop‑Lim';
  const isTifOverflow = tif === 'IOC' || tif === 'FOK';

  useEffect(() => {
    if (!typeMenu && !tifMenu) return;
    const onDoc = (e) => {
      if (typeRef.current && !typeRef.current.contains(e.target)) setTypeMenu(false);
      if (tifRef.current && !tifRef.current.contains(e.target)) setTifMenu(false);
    };
    document.addEventListener('mousedown', onDoc);
    return () => document.removeEventListener('mousedown', onDoc);
  }, [typeMenu, tifMenu]);

  return (
    <div className={'ot ' + side}>
      {/* TOP: sym/price/change inline + Buy/Sell + close */}
      <div className="ot-top">
        <div className="ot-top-l">
          <span className="ot-sym-big">{sym.sym}</span>
          <span className="ot-last">${sym.px.toFixed(2)}</span>
          <span className={'ot-chg ' + (up ? 'pos' : 'neg')}>
            {up ? '+' : ''}{sym.chg.toFixed(2)}%
          </span>
        </div>
        <div className="ot-top-r">
          <div className="ot-side compact">
            <button
              className={'ot-side-btn buy ' + (side === 'buy' ? 'active' : '')}
              onClick={() => setSide('buy')}>Buy</button>
            <button
              className={'ot-side-btn sell ' + (side === 'sell' ? 'active' : '')}
              onClick={() => setSide('sell')}>Sell</button>
            <div className="ot-side-thumb" data-side={side}/>
          </div>
          <button className="ot-x" onClick={onClose} aria-label="Close"><Ic.X size={12}/></button>
        </div>
      </div>

      {/* BID / ASK quote */}
      <div className="ot-quote-row">
        <button
          className={'ot-quote-cell bid ' + (parseFloat(limit) === +bid.toFixed(2) ? 'on' : '')}
          onClick={() => setLimit(bid.toFixed(2))}>
          <span className="ot-q-lab">BID</span>
          <span className="ot-q-px">{bid.toFixed(2)}</span>
          <span className="ot-q-sz">× 800</span>
        </button>
        <div className="ot-q-mid">
          <span className="ot-q-spread">0.10</span>
          <span className="ot-q-spread-lab">spread</span>
        </div>
        <button
          className={'ot-quote-cell ask ' + (parseFloat(limit) === +ask.toFixed(2) ? 'on' : '')}
          onClick={() => setLimit(ask.toFixed(2))}>
          <span className="ot-q-lab">ASK</span>
          <span className="ot-q-px">{ask.toFixed(2)}</span>
          <span className="ot-q-sz">× 1.2K</span>
        </button>
      </div>

      {/* limit + qty side-by-side */}
      <div className="ot-pair">
        <div className="ot-pair-col">
          <div className="ot-lab-row">
            <span className="ot-lab">{type === 'Market' ? 'Price' : 'Limit'}</span>
            {type !== 'Market' && (
              <span className="ot-mid-hint">mid {((bid + ask) / 2).toFixed(2)}</span>
            )}
          </div>
          <div className={'ot-field ' + (type === 'Market' ? 'disabled' : '')}>
            <button disabled={type === 'Market'} onClick={() => setLimit((parseFloat(limit) - 0.01).toFixed(2))}>−</button>
            <div className="ot-input-wrap">
              <span className="ot-prefix">$</span>
              <input
                disabled={type === 'Market'}
                value={type === 'Market' ? 'MKT' : limit}
                onChange={e => setLimit(e.target.value)}/>
            </div>
            <button disabled={type === 'Market'} onClick={() => setLimit((parseFloat(limit) + 0.01).toFixed(2))}>+</button>
          </div>
        </div>

        <div className="ot-pair-col">
          <div className="ot-lab-row">
            <span className="ot-lab">Quantity</span>
            <div className="ot-qchips">
              {[50, 100, 'Max'].map(p => (
                <button key={p} className="ot-qchip" onClick={() => setQty(p === 'Max' ? 235 : p)}>
                  {p === 'Max' ? 'Max' : p + '%'}
                </button>
              ))}
            </div>
          </div>
          <div className="ot-field">
            <button onClick={() => setQty(Math.max(0, qty - 10))}>−</button>
            <div className="ot-input-wrap">
              <input value={qty} onChange={e => setQty(parseInt(e.target.value) || 0)}/>
            </div>
            <button onClick={() => setQty(qty + 10)}>+</button>
          </div>
        </div>
      </div>

      {/* type + TIF on one line; each = 2-toggle + caret menu */}
      <div className="ot-pair">
        <div className="ot-combo" ref={typeRef}>
          <button className={'ot-cb ' + (type === 'Market' ? 'on' : '')} onClick={() => setType('Market')}>MKT</button>
          <button className={'ot-cb ' + (type === 'Limit' ? 'on' : '')} onClick={() => setType('Limit')}>LMT</button>
          <button
            className={'ot-cb-more ' + (isTypeOverflow ? 'on' : '')}
            onClick={() => { setTypeMenu(v => !v); setTifMenu(false); }}
            aria-label="More order types">
            {isTypeOverflow && <span className="ot-cb-overflow">{typeShort[type]}</span>}
            <span className="ot-caret">▾</span>
          </button>
          {typeMenu && (
            <div className="ot-menu">
              {[
                ['Market','Fill at next price'],
                ['Limit','Buy/sell at limit'],
                ['Stop','Trigger at stop'],
                ['Stop‑Lim','Stop → limit'],
              ].map(([k, l]) => (
                <button key={k} className={type === k ? 'on' : ''}
                  onClick={() => { setType(k); setTypeMenu(false); }}>
                  <span>{k}</span><span className="ot-menu-tag">{l}</span>
                </button>
              ))}
            </div>
          )}
        </div>

        <div className="ot-combo" ref={tifRef}>
          <button className={'ot-cb ' + (tif === 'Day' ? 'on' : '')} onClick={() => setTif('Day')}>Day</button>
          <button className={'ot-cb ' + (tif === 'GTC' ? 'on' : '')} onClick={() => setTif('GTC')}>GTC</button>
          <button
            className={'ot-cb-more ' + (isTifOverflow ? 'on' : '')}
            onClick={() => { setTifMenu(v => !v); setTypeMenu(false); }}
            aria-label="More TIF options">
            {isTifOverflow && <span className="ot-cb-overflow">{tif}</span>}
            <span className="ot-caret">▾</span>
          </button>
          {tifMenu && (
            <div className="ot-menu">
              {[
                ['Day','Day order'],
                ['GTC','Good til cancel'],
                ['IOC','Immediate or cancel'],
                ['FOK','Fill or kill'],
              ].map(([k, l]) => (
                <button key={k} className={tif === k ? 'on' : ''}
                  onClick={() => { setTif(k); setTifMenu(false); }}>
                  <span>{k}</span><span className="ot-menu-tag">{l}</span>
                </button>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* finalize: summary inline with submit button */}
      <div className="ot-finalize">
        <div className="ot-fin-l">
          <div className="ot-sum-lab">Est. {side === 'buy' ? 'cost' : 'proceeds'}</div>
          <div className="ot-sum-val">
            ${est.toLocaleString('en-US', {minimumFractionDigits: 2, maximumFractionDigits: 2})}
          </div>
          <div className="ot-sum-fee">
            @ ${fillPx.toFixed(2)} · BP ${(71180 - est).toLocaleString('en-US',{maximumFractionDigits:0})} after
          </div>
        </div>
        <button className={'ot-submit ' + side}>
          <span className="ot-submit-action">{side === 'buy' ? 'Buy' : 'Sell'}</span>
          <span className="ot-submit-detail">{qty} {sym.sym}</span>
          <span className="ot-submit-r">↵</span>
        </button>
      </div>

      <style>{`
        .ot {
          position: absolute; right: 24px; bottom: 24px; z-index: 20;
          width: 348px;
          background: linear-gradient(180deg, rgba(28,28,28,0.96) 0%, rgba(20,20,20,0.96) 100%);
          backdrop-filter: blur(24px) saturate(1.4); -webkit-backdrop-filter: blur(24px) saturate(1.4);
          border-radius: 16px;
          border: 1px solid var(--line-strong);
          box-shadow:
            0 24px 60px rgba(0,0,0,0.55),
            0 2px 8px rgba(0,0,0,0.35),
            inset 0 1px 0 rgba(255,255,255,0.06);
          padding: 10px 12px 12px;
          display: flex; flex-direction: column; gap: 9px;
          overflow: visible;
          --side: ${sideColor};
        }
        .ot.sell { --side: var(--red); }

        /* TOP */
        .ot-top {
          display: flex; align-items: center; justify-content: space-between;
          gap: 8px;
          padding-bottom: 9px;
          border-bottom: 1px solid;
          border-image: linear-gradient(90deg, transparent, var(--line) 20%, var(--line) 80%, transparent) 1;
        }
        .ot-top-l {
          display: flex; align-items: baseline; gap: 8px; min-width: 0; flex: 1; min-width: 0;
        }
        .ot-top-r { display: flex; align-items: center; gap: 6px; flex: 0 0 auto; }
        .ot-sym-big {
          font: 800 16px 'Inter Tight', sans-serif; letter-spacing: -0.04em;
          color: #fff;
        }
        .ot-last {
          font: 700 14px 'Inter Tight', sans-serif; letter-spacing: -0.025em;
          color: #fff; font-variant-numeric: tabular-nums;
        }
        .ot-chg {
          font: 600 10px 'Inter Tight', sans-serif; letter-spacing: -0.005em;
          padding: 2px 5px; border-radius: 4px;
          font-variant-numeric: tabular-nums;
        }
        .ot-chg.pos { color: var(--green); background: var(--green-bg); }
        .ot-chg.neg { color: var(--red); background: var(--red-bg); }

        /* compact side toggle inline at top */
        .ot-side.compact {
          position: relative;
          display: grid; grid-template-columns: 1fr 1fr;
          background: rgba(255,255,255,0.04);
          border-radius: 6px; padding: 2px;
          height: 24px; width: 88px;
        }
        .ot-side.compact .ot-side-btn {
          position: relative; z-index: 2;
          border: 0; background: transparent; cursor: pointer;
          color: var(--text-secondary);
          font: 700 10.5px 'Inter', sans-serif; letter-spacing: 0.02em;
          transition: color .15s ease;
        }
        .ot-side.compact .ot-side-btn:hover { color: #fff; }
        .ot-side.compact .ot-side-btn.active.buy,
        .ot-side.compact .ot-side-btn.active.sell { color: #000; }
        .ot-side.compact .ot-side-thumb {
          position: absolute; z-index: 1; top: 2px; bottom: 2px;
          width: calc(50% - 2px);
          border-radius: 4px;
          transition: left .2s cubic-bezier(.5,.1,.25,1), background .2s ease;
          box-shadow: 0 1px 4px rgba(0,0,0,0.4);
        }
        .ot-side.compact .ot-side-thumb[data-side="buy"]  { left: 2px; background: var(--green); }
        .ot-side.compact .ot-side-thumb[data-side="sell"] { left: 50%; background: var(--red); }

        .ot-x {
          width: 22px; height: 22px;
          border: 0; background: transparent; color: var(--text-tertiary);
          border-radius: 5px; cursor: pointer;
          display: flex; align-items: center; justify-content: center;
        }
        .ot-x:hover { background: rgba(255,255,255,0.06); color: #fff; }

        /* BID / ASK */
        .ot-quote-row {
          display: grid; grid-template-columns: 1fr auto 1fr; gap: 4px;
          background: rgba(0,0,0,0.4);
          border: 1px solid var(--line);
          border-radius: 10px;
          padding: 4px;
        }
        .ot-quote-cell {
          display: flex; flex-direction: column; align-items: flex-start;
          gap: 1px; padding: 6px 10px;
          border: 0; cursor: pointer;
          background: transparent; border-radius: 7px;
          transition: background .12s ease;
        }
        .ot-quote-cell.ask { align-items: flex-end; }
        .ot-quote-cell:hover { background: rgba(255,255,255,0.05); }
        .ot-quote-cell.on.bid { background: var(--red-bg); }
        .ot-quote-cell.on.ask { background: var(--green-bg); }
        .ot-q-lab {
          font: 700 9px 'Inter', sans-serif; letter-spacing: 0.12em;
          color: var(--text-tertiary);
        }
        .ot-quote-cell.bid .ot-q-lab { color: var(--red); }
        .ot-quote-cell.ask .ot-q-lab { color: var(--green); }
        .ot-q-px {
          font: 700 19px 'Inter Tight', sans-serif; letter-spacing: -0.035em;
          color: #fff; line-height: 1.05; font-variant-numeric: tabular-nums;
        }
        .ot-q-sz {
          font: 500 9.5px 'Inter Tight', sans-serif; letter-spacing: -0.005em;
          color: var(--text-tertiary); font-variant-numeric: tabular-nums;
        }
        .ot-q-mid {
          display: flex; flex-direction: column; align-items: center; justify-content: center;
          gap: 1px; padding: 0 4px;
          border-left: 1px solid var(--line);
          border-right: 1px solid var(--line);
        }
        .ot-q-spread {
          font: 600 11px 'Inter Tight', sans-serif; letter-spacing: -0.02em;
          color: var(--text-secondary); font-variant-numeric: tabular-nums;
        }
        .ot-q-spread-lab {
          font: 600 7.5px 'Inter', sans-serif; letter-spacing: 0.1em; text-transform: uppercase;
          color: var(--text-tertiary);
        }

        /* limit + qty pair container */
        .ot-pair {
          display: grid; grid-template-columns: 1fr 1fr; gap: 8px;
          position: relative;
        }
        .ot-pair-col { display: flex; flex-direction: column; gap: 5px; min-width: 0; }
        .ot-pair-col .ot-input-wrap input { width: 100%; }
        .ot-mid-hint {
          font: 500 9px 'Inter Tight', sans-serif; letter-spacing: -0.005em;
          color: var(--text-tertiary); font-variant-numeric: tabular-nums;
        }
        .ot-field.disabled { opacity: 0.5; pointer-events: none; }
        .ot-lab {
          font: 600 9.5px 'Inter', sans-serif; letter-spacing: 0.08em; text-transform: uppercase;
          color: var(--text-tertiary);
        }
        .ot-lab-row { display: flex; justify-content: space-between; align-items: center; }

        /* numeric field */
        .ot-field {
          display: grid; grid-template-columns: 28px 1fr 28px;
          background: rgba(0,0,0,0.35);
          border-radius: 8px; height: 32px;
          border: 1px solid var(--line);
          transition: border-color .15s ease, box-shadow .15s ease;
        }
        .ot-field:focus-within {
          border-color: var(--side);
          box-shadow: 0 0 0 3px color-mix(in oklch, var(--side) 18%, transparent);
        }
        .ot-field button {
          background: transparent; border: 0;
          color: var(--text-tertiary); cursor: pointer;
          font: 500 16px 'Inter Tight', sans-serif; line-height: 1;
        }
        .ot-field button:hover { color: #fff; }
        .ot-input-wrap {
          display: flex; align-items: center; justify-content: center;
          gap: 4px; min-width: 0;
        }
        .ot-input-wrap input {
          background: transparent; border: 0; outline: 0;
          color: #fff;
          font: 700 14px 'Inter Tight', sans-serif; letter-spacing: -0.025em;
          font-variant-numeric: tabular-nums;
          text-align: right; width: 70px; padding: 0;
        }
        .ot-prefix {
          font: 500 11px 'Inter', sans-serif; letter-spacing: -0.005em;
          color: var(--text-tertiary);
        }

        /* Type / TIF combo (toggle + caret menu) */
        .ot-combo {
          position: relative;
          display: grid; grid-template-columns: 1fr 1fr auto;
          background: rgba(255,255,255,0.04);
          border-radius: 7px; padding: 2px;
          height: 28px; gap: 1px;
        }
        .ot-cb {
          border: 0; background: transparent; cursor: pointer;
          color: var(--text-secondary);
          font: 700 10.5px 'Inter', sans-serif; letter-spacing: 0.02em;
          border-radius: 5px;
          transition: background .12s ease, color .12s ease;
        }
        .ot-cb:hover { color: #fff; }
        .ot-cb.on {
          background: rgba(255,255,255,0.10); color: #fff;
          box-shadow: inset 0 0 0 1px rgba(255,255,255,0.06);
        }
        .ot-cb-more {
          border: 0; cursor: pointer;
          background: transparent; color: var(--text-tertiary);
          padding: 0 6px;
          display: flex; align-items: center; gap: 3px;
          border-radius: 5px;
          font: 700 10px 'Inter Tight', sans-serif; letter-spacing: 0.02em;
          transition: background .12s ease, color .12s ease;
          min-width: 22px;
          font-variant-numeric: tabular-nums;
        }
        .ot-cb-more:hover { color: #fff; background: rgba(255,255,255,0.05); }
        .ot-cb-more.on {
          background: rgba(255,255,255,0.10); color: #fff;
          box-shadow: inset 0 0 0 1px rgba(255,255,255,0.06);
        }
        .ot-caret { opacity: 0.55; font-size: 9px; line-height: 1; }
        .ot-cb-more:hover .ot-caret,
        .ot-cb-more.on .ot-caret { opacity: 1; }

        /* dropdown menu */
        .ot-menu {
          position: absolute; right: 0; top: calc(100% + 4px);
          min-width: 180px; z-index: 30;
          background: rgba(28,28,28,0.98);
          backdrop-filter: blur(20px);
          border: 1px solid var(--line-strong);
          border-radius: 9px;
          box-shadow: 0 14px 36px rgba(0,0,0,0.65), inset 0 1px 0 rgba(255,255,255,0.04);
          padding: 4px;
          display: flex; flex-direction: column; gap: 1px;
        }
        .ot-menu button {
          display: flex; align-items: center; justify-content: space-between;
          height: 26px; padding: 0 8px;
          border: 0; background: transparent; cursor: pointer;
          color: var(--text-secondary);
          font: 600 11px 'Inter', sans-serif; letter-spacing: -0.005em;
          border-radius: 5px;
          gap: 12px;
        }
        .ot-menu button:hover { background: rgba(255,255,255,0.06); color: #fff; }
        .ot-menu button.on { color: var(--side); background: rgba(255,255,255,0.04); }
        .ot-menu-tag {
          font: 500 9.5px 'Inter Tight', sans-serif; letter-spacing: -0.005em;
          color: var(--text-tertiary);
        }

        /* qty quick chips */
        .ot-qchips { display: flex; gap: 1px; }
        .ot-qchip {
          height: 16px; padding: 0 4px;
          border: 0; cursor: pointer; background: transparent;
          color: var(--text-tertiary);
          border-radius: 3px;
          font: 600 9px 'Inter', sans-serif; letter-spacing: 0.02em;
          font-variant-numeric: tabular-nums;
        }
        .ot-qchip:hover { background: rgba(255,255,255,0.06); color: #fff; }

        /* Finalize: summary inline with submit */
        .ot-finalize {
          display: grid; grid-template-columns: 1fr auto;
          gap: 10px; align-items: stretch;
          background: rgba(0,0,0,0.40);
          border: 1px solid var(--line);
          border-radius: 11px;
          padding: 8px 8px 8px 12px;
          margin-top: 1px;
        }
        .ot-fin-l {
          display: flex; flex-direction: column; justify-content: center;
          gap: 1px; min-width: 0;
        }
        .ot-sum-lab {
          font: 600 9px 'Inter', sans-serif; letter-spacing: 0.08em; text-transform: uppercase;
          color: var(--text-tertiary);
        }
        .ot-sum-val {
          font: 800 18px 'Inter Tight', sans-serif; letter-spacing: -0.03em;
          color: #fff;
          font-variant-numeric: tabular-nums;
          line-height: 1.05;
        }
        .ot-sum-fee {
          font: 500 9.5px 'Inter Tight', sans-serif; letter-spacing: -0.005em;
          color: var(--text-tertiary);
          font-variant-numeric: tabular-nums;
          margin-top: 2px;
        }

        .ot-submit {
          height: auto; min-height: 54px; border: 0; cursor: pointer;
          border-radius: 12px;
          display: flex; flex-direction: column; align-items: center; justify-content: center;
          padding: 0 16px;
          gap: 1px;
          min-width: 116px;
          transition: transform .12s ease, background .12s ease, box-shadow .12s ease;
          position: relative;
        }
        .ot-submit.buy {
          background: var(--green); color: #000;
          box-shadow: 0 6px 20px color-mix(in oklch, var(--green) 30%, transparent), inset 0 1px 0 rgba(255,255,255,0.25);
        }
        .ot-submit.buy:hover { background: var(--green-hover); transform: translateY(-1px); }
        .ot-submit.sell {
          background: var(--red); color: #000;
          box-shadow: 0 6px 20px color-mix(in oklch, var(--red) 30%, transparent), inset 0 1px 0 rgba(255,255,255,0.25);
        }
        .ot-submit.sell:hover { background: #f57581; transform: translateY(-1px); }
        .ot-submit-action {
          font: 800 14px 'Inter Tight', sans-serif; letter-spacing: -0.025em;
        }
        .ot-submit-detail {
          font: 600 10px 'Inter', sans-serif; letter-spacing: 0.02em;
          opacity: 0.72;
        }
        .ot-submit-r {
          position: absolute; right: 8px; top: 8px;
          width: 16px; height: 16px;
          display: flex; align-items: center; justify-content: center;
          background: rgba(0,0,0,0.20); border-radius: 4px;
          font: 600 9px 'Inter', sans-serif;
        }
      `}</style>
    </div>
  );
}

// Compact ticker tape (above main, below topbar)
function TickerTape() {
  const items = [...TICKER_BAR, ...TICKER_BAR];
  return (
    <div className="ticker-tape">
      <div className="tt-track">
        {items.map((t, i) => (
          <div key={i} className="tt-item">
            <span className="tt-sym">{t.sym}</span>
            <span className="tt-px num">{t.px}</span>
            <span className={'tt-chg num ' + (t.dir === 'up' ? 'pos' : 'neg')}>{t.chg}</span>
          </div>
        ))}
      </div>
      <style>{`
        .ticker-tape {
          height: 24px; flex: 0 0 auto; overflow: hidden;
          background: rgba(0,0,0,0.6); border-bottom: 1px solid var(--line);
          display: flex; align-items: center;
          opacity: 0.78;
        }
        .ticker-tape:hover { opacity: 1; }
        .tt-track { display: flex; gap: 0; align-items: center; height: 100%; animation: tt-scroll 140s linear infinite; }
        .tt-item {
          display: flex; align-items: baseline; gap: 6px;
          padding: 0 14px; height: 100%;
          font-size: 10.5px; white-space: nowrap;
        }
        .tt-sym { font: 600 9.5px 'Inter', sans-serif; letter-spacing: 0.04em; color: var(--text-tertiary); text-transform: uppercase; }
        .tt-px { font: 500 10.5px 'Inter Tight', sans-serif; color: var(--text-secondary); letter-spacing: -0.005em; font-variant-numeric: tabular-nums; }
        .tt-chg { font: 500 10.5px 'Inter Tight', sans-serif; letter-spacing: -0.005em; opacity: 0.8; font-variant-numeric: tabular-nums; }
        @keyframes tt-scroll {
          from { transform: translateX(0); }
          to   { transform: translateX(-50%); }
        }
      `}</style>
    </div>
  );
}

window.TopBar = TopBar;
window.TickerTape = TickerTape;
window.Sidebar = Sidebar;
window.DomLadder = DomLadder;
window.OrderTicket = OrderTicket;
window.ToolBtn = ToolBtn;
window.fmt = fmt;
window.fmtPct = fmtPct;
window.fmtSigned = fmtSigned;
