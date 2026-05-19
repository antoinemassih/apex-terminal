// ed-shell.jsx — Compact app chrome (replaces masthead) + ticker band

function TopBar({ screen, setScreen }){
  const tabs = [
    { id:'dash',    rn:'I',    label:'Dashboard' },
    { id:'dash-og', rn:'Ia',   label:'Dashboard · OG' },
    { id:'dash-alt',rn:'Ib',   label:'Dashboard · ALT' },
    { id:'mkts',    rn:'II',   label:'Markets'    },
    { id:'floor',   rn:'III',  label:'Trading Floor' },
    { id:'widgets', rn:'IV',   label:'Widgets' },
    { id:'about',   rn:'V',    label:'About' },
  ];
  const now = new Date();
  const t = `${String(now.getHours()).padStart(2,'0')}:${String(now.getMinutes()).padStart(2,'0')}:${String(now.getSeconds()).padStart(2,'0')}`;

  return (
    <div className="topbar">
      <div className="tb-brand">
        Cadence<span className="co">&amp;Co.</span>
      </div>
      <div className="tb-sep"/>
      <div className="tb-tabs">
        {tabs.map(t => (
          <button key={t.id}
                  className={`tb-tab ${screen === t.id ? 'on' : ''}`}
                  onClick={() => setScreen(t.id)}>
            <span className="rn">{t.rn}.</span>{t.label}
          </button>
        ))}
      </div>
      <div className="tb-spacer"/>
      <div className="tb-status">
        <span><span className="pip"/>NYSE Open</span>
        <span className="sep">·</span>
        <span>4 ms</span>
        <span className="sep">·</span>
        <span>BP <span className="strong">$5.83M</span></span>
        <span className="sep">·</span>
        <span className="strong">{t}</span>
        <span>ET</span>
      </div>
    </div>
  );
}

function TickerBand(){
  return (
    <div className="tickerband">
      {TAPE.concat(TAPE.slice(0,4)).map((t, i) => (
        <div key={i} className="item">
          <span className="sym">{t.sym}</span>
          <span className="px">{t.px}</span>
          <span className={`ch ${t.ch>=0?'up':'dn'}`}>{fmtCh(t.ch)}</span>
        </div>
      ))}
    </div>
  );
}

function Colophon({ screen }){
  const label = { dash:'Dashboard', mkts:'Markets', floor:'Trading Floor', about:'About' }[screen] || screen;
  return (
    <div className="colophon">
      <span>© Alto &amp; Co. 2026</span>
      <span>Section IV · {label}</span>
      <span>terminal@alto.co</span>
    </div>
  );
}

window.TopBar = TopBar;
window.TickerBand = TickerBand;
window.Colophon = Colophon;
