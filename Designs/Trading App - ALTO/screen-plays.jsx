// PLAYS — full-page card grid for trade ideas
function PlaysScreen() {
  const [filter, setFilter] = useState('All');
  const filters = [
    { id:'All',    label:'All',    icon: <Ic.Layers size={12}/> },
    { id:'LONG',   label:'Long',   icon: <Ic.TrendUp size={12}/> },
    { id:'SHORT',  label:'Short',  icon: <Ic.TrendDown size={12}/> },
    { id:'fresh',  label:'Fresh',  icon: <Ic.Sparkles size={12}/> },
    { id:'active', label:'Active', icon: <Ic.Bolt size={12}/> },
  ];
  const list = PLAYS.filter(p => filter==='All' ? true : p.side===filter || p.status===filter);

  return (
    <div className="plays-screen">
      <div className="plays-hero">
        <div className="plays-hero-l">
          <div className="plays-eyebrow"><Ic.Bolt size={11}/> CURATED PLAYS · MAY 10</div>
          <h1 className="plays-title display">Today's Plays</h1>
          <p className="plays-sub">Six high-conviction setups from the desk and your network. Built with entry, target, stop, and live R-multiple tracking.</p>
          <div className="plays-stats">
            <div><span className="ps-l">Active</span><span className="ps-v num">{PLAYS.filter(p=>p.status==='active').length}</span></div>
            <div><span className="ps-l">Fresh</span><span className="ps-v num">{PLAYS.filter(p=>p.status==='fresh').length}</span></div>
            <div><span className="ps-l">Avg R/R</span><span className="ps-v num">{(PLAYS.reduce((s,p)=>s+p.rr,0)/PLAYS.length).toFixed(1)}</span></div>
            <div><span className="ps-l">Win-rate (30d)</span><span className="ps-v num pos">68%</span></div>
          </div>
        </div>
        <div className="plays-hero-r">
          <button className="plays-cta">
            <Ic.Plus size={14}/> Compose play
          </button>
        </div>
      </div>

      <div className="plays-toolbar">
        <div className="plays-filters">
          {filters.map(f => (
            <button key={f.id} className={'pl-chip' + (filter===f.id?' active':'')} onClick={() => setFilter(f.id)}>
              {f.icon}{f.label}
            </button>
          ))}
        </div>
        <div className="plays-tools">
          <button className="pl-tool"><Ic.Filter size={13}/> More filters</button>
          <button className="pl-tool"><Ic.Grid size={13}/> Grid</button>
          <button className="pl-tool"><Ic.Download size={13}/> Export</button>
        </div>
      </div>

      <div className="plays-grid">
        {list.map(p => <PlayCardFull key={p.id} p={p}/>)}
      </div>

      <style>{`
        .plays-screen { padding: 20px 24px 24px; height: 100%; overflow-y: auto; flex: 1; display: flex; flex-direction: column; gap: 18px; }

        .plays-hero {
          display: grid; grid-template-columns: 1fr auto;
          gap: 24px; align-items: end;
          padding: 24px 28px;
          background: linear-gradient(135deg, rgba(30,215,96,0.18) 0%, rgba(124,58,237,0.10) 50%, rgba(0,0,0,0) 100%), var(--bg-elev-1);
          border-radius: 16px; position: relative; overflow: hidden;
        }
        .plays-hero::before {
          content:''; position:absolute; right:-40px; top:-40px;
          width: 240px; height: 240px; border-radius:50%;
          background: radial-gradient(circle, rgba(30,215,96,0.4), transparent 70%);
          filter: blur(40px);
        }
        .plays-eyebrow {
          display: inline-flex; align-items: center; gap: 6px;
          font: 700 10px 'Inter', sans-serif; letter-spacing: 0.12em;
          color: var(--green);
        }
        .plays-title {
          font: 800 44px 'Inter Tight', sans-serif; letter-spacing: -0.035em;
          color: #fff; margin: 8px 0 6px;
        }
        .plays-sub {
          font: 500 14px 'Inter', sans-serif; color: var(--text-secondary);
          max-width: 540px; margin: 0; letter-spacing: -0.005em;
        }
        .plays-stats { display: flex; gap: 36px; margin-top: 18px; }
        .plays-stats > div { display: flex; flex-direction: column; gap: 2px; }
        .plays-stats .ps-l { font: 600 10px 'Inter', sans-serif; letter-spacing: 0.06em; text-transform: uppercase; color: var(--text-tertiary); }
        .plays-stats .ps-v { font: 700 22px 'Inter Tight', sans-serif; letter-spacing: -0.025em; color: #fff; }

        .plays-cta {
          height: 44px; padding: 0 22px; border-radius: 999px;
          background: var(--green); color: #000; border: 0; cursor: pointer;
          font: 700 14px 'Inter', sans-serif; letter-spacing: -0.005em;
          display: inline-flex; align-items: center; gap: 8px;
          transition: transform .12s ease;
        }
        .plays-cta:hover { transform: scale(1.04); background: var(--green-hover); }

        .plays-toolbar { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
        .plays-filters, .plays-tools { display: flex; gap: 6px; }
        .pl-chip {
          height: 32px; padding: 0 14px; border-radius: 999px;
          background: rgba(255,255,255,0.06); border: 0; color: #fff; cursor: pointer;
          font: 600 12px 'Inter', sans-serif; letter-spacing: -0.005em;
          display: inline-flex; align-items: center; gap: 6px;
        }
        .pl-chip:hover { background: rgba(255,255,255,0.10); }
        .pl-chip.active { background: #fff; color: #000; }
        .pl-tool {
          height: 32px; padding: 0 12px; border-radius: 8px;
          background: transparent; border: 1px solid var(--line); color: var(--text-secondary); cursor: pointer;
          font: 600 12px 'Inter', sans-serif; letter-spacing: -0.005em;
          display: inline-flex; align-items: center; gap: 6px;
        }
        .pl-tool:hover { color: #fff; border-color: var(--line-strong); }

        .plays-grid {
          display: grid; grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
          gap: 14px;
        }
      `}</style>
    </div>
  );
}

function PlayCardFull({ p }) {
  const range = p.side==='LONG' ? p.target - p.stop : p.stop - p.target;
  const cur = p.series[p.series.length-1];
  const startPx = p.side==='LONG' ? p.stop : p.target;
  const progress = Math.max(0, Math.min(1, (cur - startPx) / range));

  return (
    <div className="pcf">
      <div className="pcf-head">
        <div className="pcf-head-l">
          <span className={'pc-side ' + p.side.toLowerCase()}>
            {p.side === 'LONG' ? <Ic.TrendUp size={11}/> : <Ic.TrendDown size={11}/>} {p.side}
          </span>
          <span className="pcf-status">{p.status==='active' ? <><Ic.Bolt size={10}/> Active</> : <><Ic.Sparkles size={10}/> Fresh</>}</span>
        </div>
        <button className="icon-btn-sm" aria-label="More"><Ic.Dots size={14}/></button>
      </div>

      <div className="pcf-title-row">
        <div className="pcf-sym-block">
          <span className="pcf-sym display">{p.sym}</span>
          <span className="pcf-name">{p.name}</span>
        </div>
        <div className="pcf-spark"><MiniSpark data={p.series} color={p.accent} w={86} h={32}/></div>
      </div>

      <p className="pcf-thesis">{p.thesis}</p>

      <div className="pcf-tags">
        {p.tags.map(t => <span key={t} className="pcf-tag">{t}</span>)}
      </div>

      <div className="pcf-levels">
        <div className="pcf-lvl">
          <span className="pcf-lvl-l"><Ic.Target size={10}/> Entry</span>
          <span className="pcf-lvl-v num">${p.entry.toFixed(2)}</span>
        </div>
        <div className="pcf-lvl">
          <span className="pcf-lvl-l pos"><Ic.TrendUp size={10}/> Target</span>
          <span className="pcf-lvl-v num pos">${p.target.toFixed(2)}</span>
        </div>
        <div className="pcf-lvl">
          <span className="pcf-lvl-l neg"><Ic.Shield size={10}/> Stop</span>
          <span className="pcf-lvl-v num neg">${p.stop.toFixed(2)}</span>
        </div>
      </div>

      <div className="pcf-progress-wrap">
        <div className="pcf-progress-bar">
          <span style={{ left: `${progress*100}%` }} className="pcf-marker"/>
        </div>
        <div className="pcf-progress-axis">
          <span>Stop</span>
          <span>Entry</span>
          <span>Target</span>
        </div>
      </div>

      <div className="pcf-foot">
        <div className="pcf-foot-l">
          <span className="pcf-rr">{p.rr.toFixed(1)}R</span>
          <Conviction n={p.conviction}/>
        </div>
        <div className="pcf-foot-r">
          <span className="pcf-author"><Ic.User size={10}/> {p.author}</span>
          <span className="pcf-time"><Ic.Clock size={10}/> {p.posted}</span>
        </div>
      </div>

      <div className="pcf-actions">
        <button className="pcf-act primary"><Ic.Bolt size={12}/> Take play</button>
        <button className="pcf-act"><Ic.Bookmark size={12}/></button>
        <button className="pcf-act"><Ic.Share size={12}/></button>
      </div>

      <style>{`
        .pcf {
          background: var(--bg-elev-1); border-radius: 14px;
          padding: 16px; display: flex; flex-direction: column; gap: 12px;
          position: relative; overflow: hidden;
          border: 1px solid var(--line);
          transition: transform .12s ease, border-color .12s ease;
        }
        .pcf:hover { border-color: var(--line-strong); }
        .pcf-head { display:flex; align-items:center; justify-content: space-between; }
        .pcf-head-l { display:flex; align-items:center; gap: 8px; }
        .pcf-status {
          display:inline-flex; align-items:center; gap:4px;
          font: 600 10px 'Inter', sans-serif; letter-spacing: 0.04em; text-transform: uppercase;
          color: var(--text-tertiary);
        }

        .pcf-title-row { display:flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
        .pcf-sym-block { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
        .pcf-sym { font: 800 28px 'Inter Tight', sans-serif; letter-spacing: -0.035em; color: #fff; line-height: 1; }
        .pcf-name { font: 600 13px 'Inter', sans-serif; color: var(--text-secondary); letter-spacing: -0.005em; }

        .pcf-thesis {
          font: 500 13px 'Inter', sans-serif; line-height: 1.5;
          color: var(--text-secondary); margin: 0; letter-spacing: -0.005em;
          text-wrap: pretty;
        }

        .pcf-tags { display: flex; gap: 6px; flex-wrap: wrap; }
        .pcf-tag {
          font: 600 10px 'Inter', sans-serif; letter-spacing: 0.04em; text-transform: uppercase;
          color: var(--text-secondary); padding: 3px 8px;
          background: rgba(255,255,255,0.04); border-radius: 4px;
        }

        .pcf-levels {
          display: grid; grid-template-columns: repeat(3, 1fr); gap: 1px;
          background: var(--line); border-radius: 10px; overflow: hidden;
        }
        .pcf-lvl {
          background: rgba(0,0,0,0.4); padding: 10px 12px;
          display:flex; flex-direction:column; gap: 4px;
        }
        .pcf-lvl-l {
          display:inline-flex; align-items:center; gap: 4px;
          font: 600 10px 'Inter', sans-serif; letter-spacing: 0.04em; text-transform: uppercase;
          color: var(--text-tertiary);
        }
        .pcf-lvl-v { font: 700 15px 'Inter Tight', sans-serif; letter-spacing: -0.02em; color: #fff; }

        .pcf-progress-wrap { display:flex; flex-direction: column; gap: 6px; }
        .pcf-progress-bar {
          position: relative; height: 4px; border-radius: 2px;
          background: linear-gradient(90deg, var(--red) 0%, var(--text-tertiary) 33%, var(--green) 100%);
        }
        .pcf-marker {
          position: absolute; top: 50%; transform: translate(-50%,-50%);
          width: 12px; height: 12px; border-radius: 50%; background: #fff;
          box-shadow: 0 0 0 3px rgba(0,0,0,0.6);
        }
        .pcf-progress-axis {
          display: flex; justify-content: space-between;
          font: 600 9px 'Inter', sans-serif; letter-spacing: 0.06em; text-transform: uppercase;
          color: var(--text-tertiary);
        }

        .pcf-foot { display:flex; align-items: center; justify-content: space-between; }
        .pcf-foot-l, .pcf-foot-r { display:flex; align-items: center; gap: 10px; }
        .pcf-rr {
          font: 700 12px 'Inter Tight', sans-serif; letter-spacing: -0.01em;
          color: var(--green); padding: 3px 8px; background: var(--green-bg); border-radius: 4px;
        }
        .pcf-author, .pcf-time {
          display:inline-flex; align-items:center; gap:4px;
          font: 500 11px 'Inter', sans-serif; color: var(--text-tertiary); letter-spacing: -0.005em;
        }

        .pcf-actions { display: grid; grid-template-columns: 1fr auto auto; gap: 6px; margin-top: 4px; }
        .pcf-act {
          height: 36px; padding: 0 14px; border-radius: 999px;
          background: rgba(255,255,255,0.06); color:#fff; border: 0; cursor: pointer;
          font: 700 12px 'Inter', sans-serif; letter-spacing: -0.005em;
          display: inline-flex; align-items: center; justify-content: center; gap: 6px;
        }
        .pcf-act:hover { background: rgba(255,255,255,0.12); }
        .pcf-act.primary { background: var(--green); color: #000; }
        .pcf-act.primary:hover { background: var(--green-hover); }
      `}</style>
    </div>
  );
}

window.PlaysScreen = PlaysScreen;
