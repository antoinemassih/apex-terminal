// SCREENER — filter pills + sortable results table
function ScreenerScreen({ onPickSym }) {
  const [side, setSide] = useState('all');
  const [signal, setSignal] = useState('All');
  const signals = ['All','Breakout','Reversal','Volume spike','52W high','52W low','Squeeze','Earnings'];
  const list = SCREENER.filter(r => {
    if (side==='gain' && r.chg<0) return false;
    if (side==='loss' && r.chg>0) return false;
    if (signal!=='All' && r.signal!==signal) return false;
    return true;
  });

  return (
    <div className="scr-screen">
      <div className="scr-head">
        <div>
          <div className="scr-eyebrow"><Ic.Filter size={11}/> SCREENER · LIVE</div>
          <h1 className="scr-title display">Movers &amp; signals</h1>
        </div>
        <div className="scr-summary">
          <div><span className="scr-l">Matches</span><span className="scr-v num">{list.length}</span></div>
          <div><span className="scr-l">Avg chg</span><span className={'scr-v num ' + ((list.reduce((s,r)=>s+r.chg,0)/list.length>=0)?'pos':'neg')}>
            {(list.reduce((s,r)=>s+r.chg,0)/list.length).toFixed(2)}%
          </span></div>
          <div><span className="scr-l">Updated</span><span className="scr-v num">14:32:08</span></div>
        </div>
      </div>

      <div className="scr-criteria">
        <div className="scr-cri-grp">
          <span className="scr-cri-l">Side</span>
          <div className="scr-seg">
            {[['all','All'],['gain','Gainers'],['loss','Losers']].map(([id,l]) => (
              <button key={id} className={'scr-seg-btn' + (side===id?' active':'')} onClick={() => setSide(id)}>{l}</button>
            ))}
          </div>
        </div>
        <div className="scr-cri-grp">
          <span className="scr-cri-l">Signal</span>
          <div className="scr-pills">
            {signals.map(s => (
              <button key={s} className={'scr-pill' + (signal===s?' active':'')} onClick={() => setSignal(s)}>
                {s}
              </button>
            ))}
          </div>
        </div>
        <div className="scr-cri-grp">
          <span className="scr-cri-l">Saved</span>
          <button className="scr-pill"><Ic.Bookmark size={11}/> AI breakouts</button>
          <button className="scr-pill"><Ic.Bookmark size={11}/> Mega-cap dip</button>
          <button className="scr-pill"><Ic.Plus size={11}/> Save current</button>
        </div>
      </div>

      <div className="scr-table-card">
        <div className="scr-cols">
          <span>#</span>
          <span>Symbol</span>
          <span>Sector</span>
          <span>Signal</span>
          <span style={{textAlign:'right'}}>Last</span>
          <span style={{textAlign:'right'}}>Chg %</span>
          <span style={{textAlign:'right'}}>Volume</span>
          <span style={{textAlign:'right'}}>Mkt Cap</span>
          <span style={{textAlign:'right'}}>P/E</span>
          <span style={{textAlign:'center'}}>Action</span>
        </div>
        <div className="scr-rows">
          {list.map((r, i) => {
            const up = r.chg >= 0;
            return (
              <div key={r.sym} className="scr-row" onClick={() => onPickSym && onPickSym(r.sym)}>
                <span className="scr-i num">{i+1}</span>
                <span className="scr-sym-cell">
                  <span className="scr-sym-tile">{r.sym.slice(0,2)}</span>
                  <span>
                    <span className="scr-sym">{r.sym}</span>
                    <span className="scr-name">{r.name}</span>
                  </span>
                </span>
                <span className="scr-sec">{r.sec}</span>
                <span className="scr-sig">
                  {r.signal==='Breakout' && <Ic.TrendUp size={11}/>}
                  {r.signal==='Reversal' && <Ic.TrendDown size={11}/>}
                  {r.signal==='Volume spike' && <Ic.Activity size={11}/>}
                  {r.signal==='52W high' && <Ic.Star size={11}/>}
                  {r.signal==='52W low' && <Ic.ArrowDown size={11}/>}
                  {r.signal==='Squeeze' && <Ic.Bolt size={11}/>}
                  {r.signal==='Earnings' && <Ic.Calendar size={11}/>}
                  {r.signal==='High R/R' && <Ic.Diamond size={11}/>}
                  {r.signal}
                </span>
                <span className="scr-num num">{r.px.toFixed(2)}</span>
                <span className={'scr-chg num ' + (up?'pos':'neg')}>
                  {up? <Ic.ArrowUp size={9}/> : <Ic.ArrowDown size={9}/>}
                  {(up?'+':'−')}{Math.abs(r.chg).toFixed(2)}%
                </span>
                <span className="scr-num num" style={{ color:'var(--text-secondary)' }}>{r.vol}</span>
                <span className="scr-num num" style={{ color:'var(--text-secondary)' }}>{r.mc}</span>
                <span className="scr-num num" style={{ color:'var(--text-secondary)' }}>{r.pe}</span>
                <span className="scr-actions">
                  <button className="scr-action" aria-label="Watch"><Ic.Eye size={12}/></button>
                  <button className="scr-action" aria-label="Buy"><Ic.Plus size={12}/></button>
                </span>
              </div>
            );
          })}
        </div>
      </div>

      <style>{`
        .scr-screen { padding: 18px 22px 22px; height: 100%; overflow-y: auto; flex:1; display:flex; flex-direction: column; gap: 14px; }

        .scr-head { display:flex; align-items: end; justify-content: space-between; gap: 24px; }
        .scr-eyebrow {
          display:inline-flex; align-items:center; gap:6px;
          font: 700 10px 'Inter', sans-serif; letter-spacing: 0.12em;
          color: var(--text-secondary);
        }
        .scr-title { font: 800 32px 'Inter Tight', sans-serif; letter-spacing: -0.03em; color:#fff; margin: 4px 0 0; }
        .scr-summary { display:flex; gap: 28px; }
        .scr-summary > div { display:flex; flex-direction: column; gap:1px; align-items:flex-end; }
        .scr-l { font: 600 10px 'Inter', sans-serif; letter-spacing: 0.06em; text-transform: uppercase; color: var(--text-tertiary); }
        .scr-v { font: 700 18px 'Inter Tight', sans-serif; letter-spacing: -0.02em; color:#fff; }

        .scr-criteria {
          background: var(--bg-elev-1); border-radius: 12px;
          padding: 14px 16px; display: flex; flex-direction: column; gap: 12px;
        }
        .scr-cri-grp { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
        .scr-cri-l {
          font: 600 10px 'Inter', sans-serif; letter-spacing: 0.06em; text-transform: uppercase;
          color: var(--text-tertiary); width: 60px; flex: 0 0 60px;
        }
        .scr-seg {
          display: flex; gap: 2px; padding: 3px;
          background: rgba(255,255,255,0.04); border-radius: 999px;
        }
        .scr-seg-btn {
          height: 28px; padding: 0 14px; border-radius: 999px;
          background: transparent; border: 0; color: var(--text-secondary); cursor: pointer;
          font: 600 12px 'Inter', sans-serif; letter-spacing: -0.005em;
        }
        .scr-seg-btn:hover { color:#fff; }
        .scr-seg-btn.active { background: #fff; color: #000; }

        .scr-pills { display:flex; gap: 6px; flex-wrap: wrap; }
        .scr-pill {
          height: 28px; padding: 0 12px; border-radius: 999px;
          background: rgba(255,255,255,0.06); border: 0; color:#fff; cursor: pointer;
          font: 600 12px 'Inter', sans-serif; letter-spacing: -0.005em;
          display:inline-flex; align-items:center; gap: 5px;
        }
        .scr-pill:hover { background: rgba(255,255,255,0.10); }
        .scr-pill.active { background: var(--green); color: #000; }

        .scr-table-card { background: var(--bg-elev-1); border-radius: 12px; overflow: hidden; flex: 1; min-height: 0; }
        .scr-cols, .scr-row {
          display: grid;
          grid-template-columns: 36px 1.4fr 110px 150px 90px 90px 90px 90px 60px 80px;
          gap: 12px; align-items: center; padding: 10px 16px;
        }
        .scr-cols {
          font: 700 9px 'Inter', sans-serif; letter-spacing: 0.08em; text-transform: uppercase;
          color: var(--text-tertiary); border-bottom: 1px solid var(--line);
        }
        .scr-rows { display:flex; flex-direction:column; }
        .scr-row {
          font-size: 12px; cursor: pointer; border-bottom: 1px solid rgba(255,255,255,0.04);
        }
        .scr-row:hover { background: rgba(255,255,255,0.04); }
        .scr-i { color: var(--text-tertiary); font: 500 11px 'Inter Tight', sans-serif; }
        .scr-sym-cell { display:flex; align-items:center; gap:10px; min-width:0; }
        .scr-sym-tile {
          width: 28px; height: 28px; border-radius: 6px;
          display: inline-flex; align-items: center; justify-content: center;
          background: rgba(255,255,255,0.06);
          font: 700 10px 'Inter Tight', sans-serif; letter-spacing: 0; color:#fff;
        }
        .scr-sym { display:block; font: 700 13px 'Inter Tight', sans-serif; letter-spacing: -0.01em; color:#fff; }
        .scr-name { display:block; font: 500 11px 'Inter', sans-serif; color: var(--text-secondary); white-space:nowrap; overflow:hidden; text-overflow:ellipsis; max-width: 180px; }
        .scr-sec {
          font: 600 10px 'Inter', sans-serif; letter-spacing: 0.04em; text-transform: uppercase;
          color: var(--text-secondary); padding: 3px 8px; background: rgba(255,255,255,0.04);
          border-radius: 4px; width: fit-content;
        }
        .scr-sig {
          display:inline-flex; align-items:center; gap: 5px;
          font: 600 11px 'Inter', sans-serif; letter-spacing: -0.005em;
          color: #fff; padding: 4px 10px;
          background: rgba(30,215,96,0.10); border-radius: 999px;
          width: fit-content;
        }
        .scr-num { text-align: right; font: 600 12px 'Inter Tight', sans-serif; letter-spacing: -0.01em; color: #fff; }
        .scr-chg { display:inline-flex; align-items:center; justify-content:flex-end; gap: 3px; text-align: right; font: 600 12px 'Inter Tight', sans-serif; letter-spacing: -0.01em; }
        .scr-actions { display:flex; gap: 4px; justify-content: center; }
        .scr-action {
          width: 26px; height: 26px; border-radius: 6px;
          background: rgba(255,255,255,0.04); color: var(--text-secondary); border: 0; cursor: pointer;
          display:inline-flex; align-items:center; justify-content:center;
        }
        .scr-action:hover { background: var(--green); color: #000; }
      `}</style>
    </div>
  );
}

window.ScreenerScreen = ScreenerScreen;
