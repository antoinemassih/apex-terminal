// app.jsx — main shell, screen switcher, tweaks

const { useState, useEffect, useMemo, useRef } = React;

const TWEAK_DEFAULTS = /*EDITMODE-BEGIN*/{
  "theme": "alto",
  "screen": "terminal"
}/*EDITMODE-END*/;

function Titlebar({ screen, setScreen }){
  const tabs = [
    { id:'dense',        label:'Chart Desk',  idx:'01' },
    { id:'dashboard',    label:'Live Tiles',  idx:'02' },
    { id:'terminal',     label:'Terminal',    idx:'03' },
    { id:'dense-te',     label:'Chart · TE',  idx:'04' },
    { id:'dashboard-te', label:'Tiles · TE',  idx:'05' },
    { id:'terminal-te',  label:'Term · TE',   idx:'06' },
  ];
  const now = new Date();
  const t = `${String(now.getHours()).padStart(2,'0')}:${String(now.getMinutes()).padStart(2,'0')}:${String(now.getSeconds()).padStart(2,'0')}`;
  return (
    <div className="titlebar">
      <div className="tb-brand">
        <span className="dot pulse"/>
        <span>Alto</span>
        <span style={{ color:'var(--ink-3)', fontWeight:400, letterSpacing:'.02em', textTransform:'none', fontFamily:'var(--mono)', fontSize:10, marginLeft:4 }}>v4.2</span>
      </div>
      <div style={{ width:1, height:18, background:'#0c0906', boxShadow:'1px 0 0 var(--hi-1)' }}/>
      <div className="tb-tabs">
        {tabs.map(t => (
          <button key={t.id} className={`tb-tab ${screen===t.id ? 'on':''}`} onClick={() => setScreen(t.id)}>
            <span className="idx">{t.idx}</span>
            <span>{t.label}</span>
          </button>
        ))}
      </div>
      <div className="tb-spacer"/>
      <div className="tb-status">
        <span><span className="pip ok"/>NYSE OPEN</span>
        <span style={{ color:'var(--ink-3)' }}>·</span>
        <span>NY-4</span>
        <span style={{ color:'var(--ink-3)' }}>·</span>
        <span>L1+L2</span>
        <span style={{ color:'var(--ink-3)' }}>·</span>
        <span>4ms</span>
        <span style={{ color:'var(--ink-3)' }}>·</span>
        <span style={{ color:'var(--ink-0)' }}>{t}</span>
        <span style={{ color:'var(--ink-3)' }}>ET</span>
      </div>
    </div>
  );
}

function TickerStrip(){
  return (
    <div className="ticker-strip">
      {TAPE.concat(TAPE).map((t, i) => (
        <div key={i} className="ticker-cell">
          <span className="sym">{t.sym}</span>
          <span className="px">{t.px}</span>
          <span className={`ch ${t.ch>=0?'t-up':'t-dn'}`}>{fmtCh(t.ch)}</span>
        </div>
      ))}
    </div>
  );
}

function StatusBar({ screen }){
  return (
    <div className="statusbar">
      <span className="item"><span className="pip"/> Stream OK · 4ms</span>
      <span className="item">NY-4</span>
      <span className="item">Subscribed: SPY · NVDA · NASDAQ TotalView</span>
      <span className="item">Buying Power $5.83M</span>
      <span style={{ flex:1 }}/>
      <span className="item" style={{ color:'var(--ink-2)' }}>{screen.toUpperCase()}</span>
      <span className="item">2026-05-10 15:58:16</span>
    </div>
  );
}

function App(){
  const [t, setTweak] = useTweaks(TWEAK_DEFAULTS);
  useEffect(() => { applyTheme(t.theme); }, [t.theme]);

  const screen = t.screen;
  const setScreen = (s) => setTweak('screen', s);

  return (
    <div className="app">
      <Titlebar screen={screen} setScreen={setScreen}/>
      <div className="screen">
        {screen === 'dense'        && <ScreenDense/>}
        {screen === 'dashboard'    && <ScreenDashboard/>}
        {screen === 'terminal'     && <ScreenTerminal/>}
        {screen === 'dense-te'     && <ScreenDenseTE/>}
        {screen === 'dashboard-te' && <ScreenDashboardTE/>}
        {screen === 'terminal-te'  && <ScreenTerminalTE/>}
      </div>
      <StatusBar screen={screen}/>

      <TweaksPanel title="Alto · Tweaks">
        <TweakSection label="Theme"/>
        <TweakSelect label="Color theme" value={t.theme}
          options={Object.entries(THEMES).map(([k, th]) => ({ value: k, label: th.label }))}
          onChange={(v) => setTweak('theme', v)}/>
        <div style={{ display:'grid', gridTemplateColumns:'repeat(4, 1fr)', gap:6, padding:'4px 0 8px' }}>
          {Object.entries(THEMES).map(([k, th]) => (
            <button key={k} onClick={() => setTweak('theme', k)}
                    title={th.label}
                    style={{
                      cursor:'pointer',
                      padding:0, border:0, background:'transparent',
                      display:'flex', flexDirection:'column', alignItems:'stretch', gap:4,
                    }}>
              <div style={{
                height:36, borderRadius:4, overflow:'hidden',
                display:'grid', gridTemplateColumns:'repeat(5, 1fr)',
                border: t.theme === k ? '2px solid var(--accent)' : '1px solid var(--line-2)',
                boxShadow: t.theme === k ? '0 0 0 1px var(--accent-soft)' : 'inset 0 1px 0 var(--hi-1)'
              }}>
                {th.swatch.map((c, i) => (
                  <span key={i} style={{ background:c }}/>
                ))}
              </div>
              <span style={{ fontSize:9, fontFamily:'var(--mono)', letterSpacing:'.06em',
                              color: t.theme === k ? 'var(--ink-0)' : 'var(--ink-3)',
                              textTransform:'uppercase', textAlign:'left' }}>
                {th.label.split('·')[0].trim()}
              </span>
            </button>
          ))}
        </div>
        <TweakSection label="Screen"/>
        <TweakSelect label="Active screen" value={t.screen}
          options={[
            { value:'dense', label:'01 · Chart Desk' },
            { value:'dashboard', label:'02 · Live Tiles' },
            { value:'terminal', label:'03 · Terminal' },
            { value:'dense-te', label:'04 · Chart Desk · TE' },
            { value:'dashboard-te', label:'05 · Live Tiles · TE' },
            { value:'terminal-te', label:'06 · Terminal · TE' },
          ]}
          onChange={(v) => setTweak('screen', v)}/>
      </TweaksPanel>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')).render(<App/>);
