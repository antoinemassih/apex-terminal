// watchlist.jsx — shared Watchlist panel used by all three screens
// Header → tab strip (Watch/Pos/Alert) → search → list subheader → rows

function SymbolSearch({ placeholder = "Add symbol…", kbd = "⌘K", value, onChange }){
  const [internal, setInternal] = useState('');
  const v = value !== undefined ? value : internal;
  const set = onChange || setInternal;
  return (
    <div className="symsearch">
      <span className="symsearch-icon"><Icon.Search size={11}/></span>
      <input
        className="symsearch-input"
        placeholder={placeholder}
        value={v}
        onChange={e => set(e.target.value)}
        spellCheck={false}
        autoCorrect="off"
      />
      <span className="symsearch-kbd">{kbd}</span>
    </div>
  );
}

function WatchlistPanel({
  num = '01',
  title = 'Watchlist',
  group = 'Megacaps',
  symbols = null,                 // override syms
  count = 12,
  activeSym = null,
  setActiveSym = null,
  showFooter = false,             // P&L footer (dashboard)
  showTabs = true,
  showColumns = true,             // SYM/NAME/LAST/CHG% header
  flex = true,
}){
  const [tab, setTab] = useState('watch');
  const [query, setQuery] = useState('');

  const syms = (symbols || SYMS).slice(0, count);
  const filtered = query
    ? syms.filter(s =>
        s.sym.toLowerCase().includes(query.toLowerCase()) ||
        (s.name || '').toLowerCase().includes(query.toLowerCase()))
    : syms;

  return (
    <div style={{ display:'flex', flexDirection:'column', minHeight:0, flex: flex ? 1 : 'none', overflow:'hidden' }}>
      {/* Header */}
      <div className="ph">
        <div className="ph-l">
          <span className="fr-label"><span className="num">{num}</span> {title}</span>
        </div>
        {showTabs && (
          <div className="seg" style={{ padding:1 }}>
            {[['watch','Watch'],['pos','Pos'],['alert','Alert']].map(([id,label]) => (
              <button key={id} className={`sg ${tab===id ? 'on-acc':''}`}
                      onClick={() => setTab(id)}
                      style={{ height:18, padding:'0 7px', fontSize:9.5 }}>
                {label}
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Search */}
      <div className="symsearch-wrap">
        <SymbolSearch value={query} onChange={setQuery}/>
      </div>

      {/* Group subheader */}
      <div className="wl-group">
        <span>{group}</span>
        <span>{filtered.length}</span>
      </div>

      {/* Column header */}
      {showColumns && (
        <div className="wl-col">
          <span>SYM</span>
          <span>NAME</span>
          <span style={{ textAlign:'right' }}>LAST</span>
          <span style={{ textAlign:'right' }}>CHG%</span>
        </div>
      )}

      {/* Rows */}
      <div className="scroll" style={{ flex:1, minHeight:0 }}>
        {filtered.length === 0 && (
          <div style={{ padding:'14px 12px', fontFamily:'var(--mono)', fontSize:10.5, color:'var(--ink-3)', letterSpacing:'.06em' }}>
            NO MATCH FOR “{query}”
          </div>
        )}
        {filtered.map((s) => {
          const on = activeSym === s.sym;
          return (
            <div key={s.sym}
                 className={`wl-row2 ${on ? 'on' : ''}`}
                 onClick={() => setActiveSym && setActiveSym(s.sym)}>
              {on && <span className="wl-rail"/>}
              <span className="sym">{s.sym}</span>
              <span className="nm">{(s.name || '').slice(0, 10)}</span>
              <span className="num px">{fmtNum(s.px)}</span>
              <span className={`num ch ${s.ch>=0 ? 't-up':'t-dn'}`}>{fmtCh(s.ch)}</span>
            </div>
          );
        })}
      </div>

      {/* Optional P&L footer (live tiles) */}
      {showFooter && (
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
      )}
    </div>
  );
}

window.WatchlistPanel = WatchlistPanel;
window.SymbolSearch = SymbolSearch;
