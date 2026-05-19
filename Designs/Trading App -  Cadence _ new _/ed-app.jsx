// ed-app.jsx — main entry for the Editorial alternative

const { useState, useEffect, useMemo, useRef } = React;

const TWEAK_DEFAULTS_ED = /*EDITMODE-BEGIN*/{
  "screen": "dash",
  "theme": "meridien"
}/*EDITMODE-END*/;

function EditorialApp(){
  const [t, setTweak] = useTweaks(TWEAK_DEFAULTS_ED);
  const screen = t.screen;
  const setScreen = (s) => setTweak('screen', s);
  const [activeSym, setActiveSym] = useState('NVDA');

  useEffect(() => { applyEdTheme(t.theme); }, [t.theme]);

  return (
    <div className="paper">
      <TopBar screen={screen} setScreen={setScreen}/>
      <TickerBand/>

      <div className="page" style={{ overflow: screen === 'floor' ? 'hidden' : 'auto' }}>
        {screen === 'dash'     && <ScreenDashboard activeSym={activeSym} setActiveSym={setActiveSym}/>}
        {screen === 'dash-og'  && <ScreenDashboardOG activeSym={activeSym} setActiveSym={setActiveSym}/>}
        {screen === 'dash-alt' && <ScreenDashboardAlt activeSym={activeSym} setActiveSym={setActiveSym}/>}
        {screen === 'mkts'     && <ScreenMarkets activeSym={activeSym} setActiveSym={setActiveSym}/>}
        {screen === 'floor'    && <ScreenFloor activeSym={activeSym} setActiveSym={setActiveSym}/>}
        {screen === 'widgets'  && <ScreenWidgets activeSym={activeSym} setActiveSym={setActiveSym}/>}
        {screen === 'about'    && <ScreenAbout/>}
      </div>

      <Colophon screen={screen}/>

      <TweaksPanel title="Cadence · Tweaks">
        <TweakSection label="Theme"/>
        <TweakSelect label="Color scheme" value={t.theme}
          options={Object.entries(ED_THEMES).map(([k, th]) => ({ value: k, label: th.label }))}
          onChange={(v) => setTweak('theme', v)}/>
        <div style={{ display:'grid', gridTemplateColumns:'repeat(4, 1fr)', gap:6, padding:'4px 0 8px' }}>
          {Object.entries(ED_THEMES).map(([k, th]) => (
            <button key={k} onClick={() => setTweak('theme', k)}
                    title={th.label}
                    style={{
                      cursor:'pointer',
                      padding:0, border:0, background:'transparent',
                      display:'flex', flexDirection:'column', alignItems:'stretch', gap:4,
                    }}>
              <div style={{
                height:36, borderRadius:0, overflow:'hidden',
                display:'grid', gridTemplateColumns:'repeat(5, 1fr)',
                border: t.theme === k ? '2px solid var(--accent, #d99858)' : '1px solid #555',
              }}>
                {th.swatch.map((c, i) => <span key={i} style={{ background:c }}/>)}
              </div>
              <span style={{ fontSize:9, fontFamily:'ui-monospace, monospace', letterSpacing:'.06em',
                              textTransform:'uppercase', textAlign:'left',
                              opacity: t.theme === k ? 1 : .6 }}>
                {th.label.split('·')[0].trim()}
              </span>
            </button>
          ))}
        </div>
        <TweakSection label="Screen"/>
        <TweakSelect label="Active section" value={t.screen}
          options={[
            { value:'dash',     label:'I · Dashboard' },
            { value:'dash-og',  label:'Ia · Dashboard · OG' },
            { value:'dash-alt', label:'Ib · Dashboard · ALT' },
            { value:'mkts',     label:'II · Markets' },
            { value:'floor',    label:'III · Trading Floor' },
            { value:'widgets',  label:'IV · Widgets' },
            { value:'about',    label:'V · About' },
          ]}
          onChange={(v) => setTweak('screen', v)}/>
      </TweaksPanel>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')).render(<EditorialApp/>);
