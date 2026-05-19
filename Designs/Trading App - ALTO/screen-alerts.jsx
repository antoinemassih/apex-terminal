// ALERTS — full-page alert manager with composer, armed, triggered history
function AlertsScreen() {
  const [tab, setTab] = useState('armed');
  const armed = ALERTS.filter(a=>a.status==='armed');
  const triggered = ALERTS.filter(a=>a.status==='triggered');
  const list = tab==='armed' ? armed : tab==='triggered' ? triggered : ALERTS;

  return (
    <div className="al-screen">
      <div className="al-head">
        <div>
          <div className="al-eyebrow"><Ic.Bell size={11}/> ALERTS · {armed.length} ARMED</div>
          <h1 className="al-title display">Alerts</h1>
        </div>
        <button className="al-cta"><Ic.Plus size={14}/> New alert</button>
      </div>

      <div className="al-grid">
        {/* Composer card — preview style */}
        <div className="al-composer">
          <div className="al-comp-head"><Ic.Sparkles size={12}/> QUICK COMPOSE</div>
          <div className="al-comp-body">
            <span className="al-comp-pill sym">NVDA</span>
            <span className="al-comp-text">price</span>
            <span className="al-comp-pill op">crosses above</span>
            <span className="al-comp-pill val num">$1,300.00</span>
            <span className="al-comp-text">notify</span>
            <span className="al-comp-pill ch">push + email</span>
          </div>
          <div className="al-comp-actions">
            <button className="al-comp-btn primary"><Ic.Bell size={12}/> Arm alert</button>
            <button className="al-comp-btn">Save template</button>
          </div>
        </div>

        {/* Tabs + list */}
        <div className="al-list-card">
          <div className="al-tabs">
            {[['armed','Armed',armed.length],['triggered','Triggered',triggered.length],['all','All',ALERTS.length]].map(([id,l,n]) => (
              <button key={id} className={'al-tab' + (tab===id?' active':'')} onClick={() => setTab(id)}>
                {l} <span className="al-tab-n">{n}</span>
              </button>
            ))}
            <div style={{ marginLeft:'auto', display:'flex', gap:6 }}>
              <button className="pl-tool"><Ic.Filter size={12}/> Filter</button>
              <button className="pl-tool"><Ic.Settings size={12}/></button>
            </div>
          </div>

          <div className="al-rows">
            {list.map(a => (
              <div key={a.id} className="al-row">
                <div className={'al-row-icon ' + a.status}>
                  {a.kind==='price' && <Ic.Target size={14}/>}
                  {a.kind==='volume' && <Ic.Activity size={14}/>}
                  {a.kind==='indicator' && <Ic.Pulse size={14}/>}
                  {a.kind==='news' && <Ic.News size={14}/>}
                  {a.kind==='event' && <Ic.Calendar size={14}/>}
                </div>
                <div className="al-row-sym">{a.sym}</div>
                <div className="al-row-cond">
                  <span className="al-row-kind">{a.kind}</span>
                  <span className="al-row-text">{a.cond} <b className="num">{a.val}</b></span>
                </div>
                <div className={'al-row-status ' + a.status}>
                  {a.status==='armed' ? <><span className="al-pulse"/>Armed</> : <><Ic.Bolt size={11}/>Triggered</>}
                </div>
                <div className="al-row-time"><Ic.Clock size={11}/> {a.set}</div>
                <div className="al-row-actions">
                  <button className="al-act"><Ic.Eye size={12}/></button>
                  <button className="al-act"><Ic.Settings size={12}/></button>
                  <button className="al-act"><Ic.X size={12}/></button>
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Channels card */}
        <div className="al-channels">
          <div className="al-comp-head"><Ic.Bell size={12}/> NOTIFICATION CHANNELS</div>
          <div className="al-ch-list">
            {[
              {l:'Push (mobile)', s:'on', sub:'iPhone · iPad'},
              {l:'Email',         s:'on', sub:'jm@cadence.app'},
              {l:'Slack',         s:'on', sub:'#trading-desk'},
              {l:'SMS',           s:'off',sub:'+1 (•••) ••• 4218'},
              {l:'Webhook',       s:'off',sub:'POST · 2 endpoints'},
            ].map(c => (
              <div key={c.l} className="al-ch">
                <div>
                  <div className="al-ch-l">{c.l}</div>
                  <div className="al-ch-s">{c.sub}</div>
                </div>
                <span className={'al-toggle ' + c.s}><span/></span>
              </div>
            ))}
          </div>
        </div>
      </div>

      <style>{`
        .al-screen { padding: 18px 22px 22px; height:100%; overflow-y:auto; flex:1; display:flex; flex-direction:column; gap:14px; }
        .al-head { display:flex; align-items:end; justify-content:space-between; }
        .al-eyebrow { display:inline-flex; align-items:center; gap:6px; font: 700 10px 'Inter'; letter-spacing: 0.12em; color: var(--text-secondary); }
        .al-title { font: 800 32px 'Inter Tight'; letter-spacing: -0.03em; color:#fff; margin: 4px 0 0; }
        .al-cta {
          height: 40px; padding: 0 18px; border-radius: 999px;
          background: var(--green); color:#000; border:0; cursor:pointer;
          font: 700 13px 'Inter'; letter-spacing: -0.005em;
          display:inline-flex; align-items:center; gap:6px;
        }
        .al-cta:hover { background: var(--green-hover); transform: scale(1.04); }

        .al-grid { display: grid; grid-template-columns: 1fr 320px; gap: 14px; }
        .al-composer, .al-channels {
          background: var(--bg-elev-1); border-radius: 14px; padding: 16px;
          display: flex; flex-direction: column; gap: 10px;
        }
        .al-composer { grid-column: 1 / 2; }
        .al-comp-head { font: 700 10px 'Inter'; letter-spacing: 0.12em; color: var(--text-tertiary); display:inline-flex; align-items:center; gap:6px; }
        .al-comp-body {
          display:flex; align-items:center; gap:8px; flex-wrap: wrap;
          padding: 14px 0;
        }
        .al-comp-text { font: 500 14px 'Inter'; color: var(--text-secondary); letter-spacing: -0.005em; }
        .al-comp-pill {
          height: 36px; padding: 0 14px; border-radius: 8px;
          display:inline-flex; align-items:center; gap:6px;
          font: 600 13px 'Inter'; letter-spacing: -0.005em;
        }
        .al-comp-pill.sym { background: var(--green-bg); color: var(--green); font-weight: 700; }
        .al-comp-pill.op  { background: rgba(255,255,255,0.06); color:#fff; }
        .al-comp-pill.val { background: rgba(255,255,255,0.08); color:#fff; font: 700 14px 'Inter Tight'; letter-spacing: -0.01em; }
        .al-comp-pill.ch  { background: rgba(124,58,237,0.18); color: #c4a8ff; }
        .al-comp-actions { display:flex; gap:8px; padding-top: 4px; border-top: 1px solid var(--line); margin-top: 4px; padding-top: 14px; }
        .al-comp-btn {
          height: 38px; padding: 0 16px; border-radius: 999px;
          background: rgba(255,255,255,0.06); color:#fff; border: 0; cursor: pointer;
          font: 700 12px 'Inter'; letter-spacing: -0.005em;
          display:inline-flex; align-items:center; gap:6px;
        }
        .al-comp-btn.primary { background: var(--green); color:#000; }
        .al-comp-btn:hover { transform: translateY(-1px); }

        .al-list-card { grid-column: 1 / 2; background: var(--bg-elev-1); border-radius: 14px; overflow: hidden; }
        .al-tabs { display:flex; align-items:center; gap: 4px; padding: 12px; border-bottom: 1px solid var(--line); }
        .al-tab {
          height: 32px; padding: 0 14px; border-radius: 999px;
          background: transparent; border: 0; color: var(--text-secondary); cursor: pointer;
          font: 600 12px 'Inter'; letter-spacing: -0.005em;
          display: inline-flex; align-items: center; gap: 6px;
        }
        .al-tab:hover { color:#fff; background: rgba(255,255,255,0.04); }
        .al-tab.active { color:#fff; background: rgba(255,255,255,0.10); }
        .al-tab-n { font: 700 10px 'Inter Tight'; padding: 1px 6px; border-radius: 999px; background: rgba(255,255,255,0.10); color:#fff; }
        .al-tab.active .al-tab-n { background: var(--green); color:#000; }

        .al-rows { display:flex; flex-direction: column; }
        .al-row {
          display: grid;
          grid-template-columns: 36px 70px 1fr 110px 90px 100px;
          gap: 14px; align-items: center;
          padding: 12px 16px; border-bottom: 1px solid rgba(255,255,255,0.04);
        }
        .al-row:last-child { border-bottom: 0; }
        .al-row:hover { background: rgba(255,255,255,0.03); }
        .al-row-icon {
          width:36px; height:36px; border-radius:10px;
          display:inline-flex; align-items:center; justify-content:center;
          background: rgba(255,255,255,0.06); color: var(--text-secondary);
        }
        .al-row-icon.triggered { background: var(--green-bg); color: var(--green); }
        .al-row-sym { font: 700 14px 'Inter Tight'; letter-spacing: -0.02em; color:#fff; }
        .al-row-cond { display:flex; flex-direction:column; gap:2px; min-width:0; }
        .al-row-kind { font: 700 9px 'Inter'; letter-spacing: 0.08em; text-transform: uppercase; color: var(--text-tertiary); }
        .al-row-text { font: 500 13px 'Inter'; color: var(--text-secondary); letter-spacing: -0.005em; }
        .al-row-text b { color: #fff; font-weight: 600; }
        .al-row-status {
          display:inline-flex; align-items:center; gap:5px;
          font: 600 11px 'Inter'; letter-spacing: -0.005em;
          padding: 4px 10px; border-radius: 999px;
        }
        .al-row-status.armed { color: var(--text-secondary); background: rgba(255,255,255,0.06); }
        .al-row-status.triggered { color: var(--green); background: var(--green-bg); }
        .al-pulse { width: 6px; height: 6px; border-radius: 50%; background: var(--green); animation: pulse 1.6s ease-in-out infinite; }
        .al-row-time { display:inline-flex; align-items:center; gap:5px; font: 500 11px 'Inter Tight'; color: var(--text-tertiary); letter-spacing: -0.005em; }
        .al-row-actions { display:flex; gap:4px; justify-content:flex-end; }
        .al-act {
          width: 28px; height: 28px; border-radius: 6px;
          background: rgba(255,255,255,0.04); color: var(--text-secondary); border: 0; cursor: pointer;
          display:inline-flex; align-items:center; justify-content:center;
        }
        .al-act:hover { background: rgba(255,255,255,0.10); color:#fff; }

        .al-channels { grid-column: 2 / 3; grid-row: 1 / 3; }
        .al-ch-list { display: flex; flex-direction: column; gap: 4px; padding-top: 6px; }
        .al-ch {
          display:flex; align-items:center; justify-content:space-between;
          padding: 10px 12px; background: rgba(255,255,255,0.02); border-radius: 8px;
          border: 1px solid var(--line);
        }
        .al-ch-l { font: 600 13px 'Inter'; color:#fff; letter-spacing: -0.005em; }
        .al-ch-s { font: 500 11px 'Inter'; color: var(--text-tertiary); margin-top: 2px; letter-spacing: -0.005em; }
        .al-toggle {
          width: 36px; height: 20px; border-radius: 999px;
          background: rgba(255,255,255,0.10); position: relative; cursor: pointer;
        }
        .al-toggle > span {
          position:absolute; top:2px; left:2px; width:16px; height:16px; border-radius:50%; background:#fff;
          transition: left .15s ease;
        }
        .al-toggle.on { background: var(--green); }
        .al-toggle.on > span { left: 18px; background:#000; }
      `}</style>
    </div>
  );
}

window.AlertsScreen = AlertsScreen;
