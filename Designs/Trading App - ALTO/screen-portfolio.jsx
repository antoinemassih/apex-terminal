// PORTFOLIO — positions table + summary, no hero block

function PortfolioScreen({ activeSym, onPickSym }) {
  const totalMV = POSITIONS.reduce((s, p) => s + p.mv, 0);
  const totalPnl = POSITIONS.reduce((s, p) => s + p.pnl, 0);
  const totalPnlPct = (totalPnl / (totalMV - totalPnl)) * 100;
  const [sortBy, setSortBy] = useState('mv');
  const [sortDir, setSortDir] = useState('desc');

  const sorted = [...POSITIONS].sort((a, b) => {
    const va = a[sortBy], vb = b[sortBy];
    if (typeof va === 'string') return sortDir === 'asc' ? va.localeCompare(vb) : vb.localeCompare(va);
    return sortDir === 'asc' ? va - vb : vb - va;
  });

  const toggleSort = (k) => {
    if (sortBy === k) setSortDir(sortDir === 'asc' ? 'desc' : 'asc');
    else { setSortBy(k); setSortDir('desc'); }
  };

  return (
    <div className="port-screen">
      {/* Summary strip */}
      <div className="port-summary">
        <div className="ps-tile">
          <div className="ps-l">Net Asset Value</div>
          <div className="ps-v mono num">${totalMV.toLocaleString()}</div>
          <div className="ps-sub">{POSITIONS.length} positions · 100% long</div>
        </div>
        <div className="ps-tile">
          <div className="ps-l">Open P&amp;L</div>
          <div className={'ps-v mono num ' + (totalPnl >= 0 ? 'pos' : 'neg')}>{fmtSigned(totalPnl, 0)}</div>
          <div className={'ps-sub ' + (totalPnl >= 0 ? 'pos' : 'neg')}>{fmtPct(totalPnlPct)} on cost</div>
        </div>
        <div className="ps-tile">
          <div className="ps-l">Today</div>
          <div className="ps-v mono num pos">+$3,210</div>
          <div className="ps-sub">+0.25% · {POSITIONS.filter(p=>p.pnl>0).length}/{POSITIONS.length} winners</div>
        </div>
        <div className="ps-tile">
          <div className="ps-l">Cash</div>
          <div className="ps-v mono num">$58,302</div>
          <div className="ps-sub">4.6% of NAV</div>
        </div>
        <div className="ps-tile">
          <div className="ps-l">Buying Power</div>
          <div className="ps-v mono num">$116,604</div>
          <div className="ps-sub">2× margin · used 0%</div>
        </div>
        <div className="ps-tile">
          <div className="ps-l">Day's β-adj. exposure</div>
          <div className="ps-v mono num">$1.51M</div>
          <div className="ps-sub">net long · β 1.18</div>
        </div>
      </div>

      {/* Positions table */}
      <div className="positions-card">
        <div className="card-head">
          <span className="card-eyebrow"><b style={{ color: '#fff' }}>Positions</b> · sorted by {sortBy.toUpperCase()}</span>
          <div style={{ display: 'flex', gap: 8 }}>
            <button className="add-pill"><Ic.Filter size={12}/> Filters</button>
            <button className="add-pill">Group: Sector <Ic.Chevron size={12} dir="down"/></button>
            <button className="add-pill"><Ic.Download size={12}/> Export</button>
          </div>
        </div>
        <div className="ptable">
          <div className="pt-head">
            <span>#</span>
            <span>SYMBOL</span>
            <span className="pt-sortable" onClick={() => toggleSort('qty')}>QTY</span>
            <span className="pt-sortable" onClick={() => toggleSort('avg')}>AVG COST</span>
            <span className="pt-sortable" onClick={() => toggleSort('px')}>LAST</span>
            <span className="pt-sortable" onClick={() => toggleSort('mv')}>MARKET VALUE</span>
            <span className="pt-sortable" onClick={() => toggleSort('pnl')}>OPEN P&amp;L</span>
            <span className="pt-sortable" onClick={() => toggleSort('pnlPct')}>%</span>
            <span>WEIGHT</span>
          </div>
          {sorted.map((p, i) => {
            const up = p.pnl >= 0;
            const wPct = p.mv / totalMV * 100;
            const active = p.sym === activeSym;
            return (
              <div key={p.sym} className={'pt-row' + (active ? ' active' : '')} onClick={() => onPickSym(p.sym)}>
                <span className="pt-i">{i+1}</span>
                <span className="pt-sym-cell">
                  <span>
                    <span className="pt-sym">{p.sym}</span>
                    <span className="pt-name">{p.name}</span>
                  </span>
                </span>
                <span className="num mono">{p.qty.toLocaleString()}</span>
                <span className="num mono">{p.avg.toFixed(2)}</span>
                <span className="num mono">{p.px.toFixed(2)}</span>
                <span className="num mono" style={{ color: '#fff', fontWeight: 600 }}>${p.mv.toLocaleString()}</span>
                <span className={'num mono ' + (up ? 'pos' : 'neg')}>{fmtSigned(p.pnl, 0)}</span>
                <span className={'num mono ' + (up ? 'pos' : 'neg')}>{fmtPct(p.pnlPct)}</span>
                <span className="pt-bar"><span className="pt-bar-fill" style={{ width: `${wPct}%` }}/><span className="pt-bar-val mono num">{wPct.toFixed(1)}%</span></span>
              </div>
            );
          })}
        </div>
      </div>

      {/* Bottom: sector + activity */}
      <div className="port-bottom">
        <div className="port-sub-card">
          <div className="card-head">
            <span className="card-eyebrow"><b style={{ color: '#fff' }}>Sector exposure</b></span>
            <span className="card-eyebrow">% of NAV</span>
          </div>
          <div className="port-sub-body">
            {[
              { l: 'Information Technology', v: 62.4, c: '#1ed760' },
              { l: 'Communication Services', v: 14.2, c: '#7c3aed' },
              { l: 'Consumer Discretionary',  v: 12.0, c: '#3b82f6' },
              { l: 'Financials',              v: 6.8,  c: '#f59e0b' },
              { l: 'Other',                   v: 4.6,  c: '#6b7280' },
            ].map(s => (
              <div key={s.l} className="sec-row">
                <span className="sec-l">{s.l}</span>
                <span className="sec-bar"><span style={{ width: `${s.v*1.4}%`, background: s.c }}/></span>
                <span className="sec-v num mono">{s.v.toFixed(1)}%</span>
              </div>
            ))}
          </div>
        </div>

        <div className="port-sub-card">
          <div className="card-head">
            <span className="card-eyebrow"><b style={{ color: '#fff' }}>Recent activity</b></span>
            <button className="add-pill">View all</button>
          </div>
          <div className="port-sub-body">
            {[
              { t: '14:32', s: 'BUY', sym: 'NVDA', q: 40, px: 1278.40, val: 51136 },
              { t: '11:08', s: 'SELL',sym: 'TSLA', q: 80, px: 256.90,  val: 20552 },
              { t: '10:24', s: 'BUY', sym: 'META', q: 25, px: 568.10,  val: 14202 },
              { t: '09:48', s: 'BUY', sym: 'MSFT', q: 60, px: 436.20,  val: 26172 },
              { t: '09:32', s: 'DIV', sym: 'AAPL', q: 1200, px: 0.24,  val: 288 },
            ].map((a,i)=>(
              <div key={i} className="act-row">
                <span className="act-t mono">{a.t}</span>
                <span className={'act-s ' + (a.s==='BUY'?'act-buy':a.s==='SELL'?'act-sell':'act-div')}>{a.s}</span>
                <span className="act-sym">{a.sym}</span>
                <span className="act-q num mono">{a.q.toLocaleString()} @ {a.px.toFixed(2)}</span>
                <span className="act-v num mono">${a.val.toLocaleString()}</span>
              </div>
            ))}
          </div>
        </div>
      </div>

      <style>{`
        .port-screen { padding: 16px; display: flex; flex-direction: column; gap: 12px; height: 100%; overflow-y: auto; }
        .port-summary {
          display: grid; grid-template-columns: repeat(6, 1fr); gap: 1px;
          background: var(--line); border: 1px solid var(--line); border-radius: 8px; overflow: hidden;
        }
        .ps-tile { background: var(--bg-elev-1); padding: 14px 18px; display: flex; flex-direction: column; gap: 4px; }
        .ps-l { font: 700 10px 'Figtree', sans-serif; letter-spacing: 0.08em; text-transform: uppercase; color: var(--text-tertiary); }
        .ps-v { font: 700 22px 'JetBrains Mono', monospace; color: #fff; }
        .ps-sub { font: 500 11px 'Figtree', sans-serif; color: var(--text-secondary); }

        .positions-card {
          background: var(--bg-elev-1); border-radius: 8px; overflow: hidden; border: 1px solid var(--line);
        }
        .positions-card .card-head {
          display: flex; align-items: center; justify-content: space-between;
          padding: 10px 16px; border-bottom: 1px solid var(--line);
        }
        .ptable { padding: 0; }
        .pt-head, .pt-row {
          display: grid;
          grid-template-columns: 28px minmax(180px, 1.8fr) 70px 90px 90px 130px 110px 70px 1.2fr;
          gap: 14px; align-items: center;
          padding: 8px 16px;
          font-size: 12px;
        }
        .pt-head { color: var(--text-tertiary); font: 700 9px 'Figtree', sans-serif; letter-spacing: 0.08em; border-bottom: 1px solid var(--line); }
        .pt-sortable { cursor: pointer; user-select: none; }
        .pt-sortable:hover { color: #fff; }
        .pt-row { color: var(--text-primary); cursor: pointer; border-bottom: 1px solid rgba(255,255,255,0.04); }
        .pt-row:hover { background: var(--bg-row-hover); }
        .pt-row.active { background: rgba(30,215,96,0.08); }
        .pt-i { color: var(--text-tertiary); font: 500 12px 'JetBrains Mono', monospace; }
        .pt-sym-cell { display: flex; align-items: center; gap: 12px; min-width: 0; }
        .pt-sym { display: block; font: 700 13px 'Figtree', sans-serif; color: #fff; letter-spacing: 0.01em; }
        .pt-name { display: block; font: 500 11px 'Figtree', sans-serif; color: var(--text-secondary); }
        .pt-bar { display: flex; align-items: center; gap: 8px; height: 18px; position: relative; }
        .pt-bar::before { content: ''; flex: 1; height: 4px; background: rgba(255,255,255,0.06); border-radius: 2px; position: relative; }
        .pt-bar-fill { position: absolute; left: 0; top: 50%; transform: translateY(-50%); height: 4px; background: var(--green); border-radius: 2px; }
        .pt-bar-val { font: 600 11px 'JetBrains Mono', monospace; color: var(--text-secondary); min-width: 40px; text-align: right; }

        .port-bottom { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
        .port-sub-card { background: var(--bg-elev-1); border: 1px solid var(--line); border-radius: 8px; overflow: hidden; }
        .port-sub-card .card-head { display: flex; align-items: center; justify-content: space-between; padding: 10px 16px; border-bottom: 1px solid var(--line); }
        .port-sub-body { padding: 12px 16px; }
        .sec-row { display: grid; grid-template-columns: 200px 1fr 60px; gap: 14px; align-items: center; padding: 6px 0; }
        .sec-l { font: 500 12px 'Figtree', sans-serif; color: var(--text-secondary); }
        .sec-bar { display: block; height: 6px; background: rgba(255,255,255,0.06); border-radius: 3px; overflow: hidden; }
        .sec-bar > span { display: block; height: 100%; border-radius: 3px; }
        .sec-v { font-size: 12px; color: #fff; text-align: right; }

        .act-row {
          display: grid; grid-template-columns: 60px 56px 70px 1fr auto;
          gap: 12px; align-items: center; padding: 6px 0; border-bottom: 1px solid rgba(255,255,255,0.04);
          font-size: 12px;
        }
        .act-row:last-child { border-bottom: 0; }
        .act-t { color: var(--text-tertiary); font-size: 11px; }
        .act-s { font: 700 9px 'Figtree', sans-serif; letter-spacing: 0.08em; padding: 2px 6px; border-radius: 3px; text-align: center; }
        .act-buy { background: var(--green-bg); color: var(--green); }
        .act-sell { background: var(--red-bg); color: var(--red); }
        .act-div { background: rgba(230,200,74,0.15); color: #e6c84a; }
        .act-sym { font: 700 12px 'Figtree', sans-serif; color: #fff; }
        .act-q { color: var(--text-secondary); }
        .act-v { color: #fff; font-weight: 600; }

        .add-pill {
          height: 24px; padding: 0 10px; border-radius: 4px;
          background: transparent; color: var(--text-secondary); border: 1px solid var(--line);
          font: 600 11px 'Figtree', sans-serif; cursor: pointer;
          display: inline-flex; align-items: center; gap: 4px;
        }
        .add-pill:hover { color: #fff; border-color: var(--line-strong); }
      `}</style>
    </div>
  );
}

window.PortfolioScreen = PortfolioScreen;
