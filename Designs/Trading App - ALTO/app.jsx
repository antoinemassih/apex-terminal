// App entry — top bar + ticker tape + main content row + sidebar
const TWEAK_DEFAULTS = /*EDITMODE-BEGIN*/{
  "screen": "apex",
  "showSidebar": true,
  "showTickerTape": true,
  "showDom": true,
  "showOrderTicket": true
}/*EDITMODE-END*/;

function App() {
  const [tweaks, setTweak] = useTweaks(TWEAK_DEFAULTS);
  const [activeSym, setActiveSym] = useState('NVDA');
  const [tf, setTf] = useState('5m');

  const Screen = {
    tiles:     TilesScreen,
    apex:      ApexScreen,
    portfolio: PortfolioScreen,
    plays:     PlaysScreen,
    screener:  ScreenerScreen,
    news:      NewsScreen,
    alerts:    AlertsScreen,
  }[tweaks.screen] || TilesScreen;

  const tools =
      tweaks.screen === 'apex' ? <ChartTools tf={tf} onTf={setTf}/>
    : tweaks.screen === 'portfolio' ? <>
        <ToolBtn active>All</ToolBtn><ToolBtn>Day</ToolBtn><ToolBtn>YTD</ToolBtn><ToolBtn>Inception</ToolBtn>
      </>
    : tweaks.screen === 'plays' ? <>
        <ToolBtn active><Ic.Layers size={11}/> All</ToolBtn>
        <ToolBtn><Ic.TrendUp size={11}/> Long</ToolBtn>
        <ToolBtn><Ic.TrendDown size={11}/> Short</ToolBtn>
        <ToolBtn><Ic.Sparkles size={11}/> Fresh</ToolBtn>
      </>
    : tweaks.screen === 'screener' ? <>
        <ToolBtn active>Movers</ToolBtn><ToolBtn>Squeeze</ToolBtn><ToolBtn>Earnings</ToolBtn><ToolBtn>52W high</ToolBtn>
      </>
    : tweaks.screen === 'news' ? <>
        <ToolBtn active>All</ToolBtn><ToolBtn>Macro</ToolBtn><ToolBtn>Earnings</ToolBtn><ToolBtn>Tech</ToolBtn>
      </>
    : tweaks.screen === 'alerts' ? <>
        <ToolBtn active>Armed</ToolBtn><ToolBtn>Triggered</ToolBtn><ToolBtn>Templates</ToolBtn>
      </>
    : <>
        <ToolBtn active>Overview</ToolBtn><ToolBtn>Risk</ToolBtn><ToolBtn>Macro</ToolBtn><ToolBtn>Heatmap</ToolBtn>
      </>;

  const showLeftDom = tweaks.screen === 'apex' && tweaks.showDom;
  const showFloatingTicket = tweaks.screen === 'apex' && tweaks.showOrderTicket;

  const labelMap = {
    tiles:'01 Dashboard', apex:'02 Markets', portfolio:'03 Portfolio',
    plays:'04 Plays', screener:'05 Screener', news:'06 News', alerts:'07 Alerts',
  };

  return (
    <div className="app">
      <TopBar
        active={tweaks.screen}
        onNav={(id) => setTweak('screen', id)}
        tools={tools}
        domOpen={tweaks.showDom}
        orderOpen={tweaks.showOrderTicket}
        onToggleDom={() => setTweak('showDom', !tweaks.showDom)}
        onToggleOrder={() => setTweak('showOrderTicket', !tweaks.showOrderTicket)}
      />
      {tweaks.showTickerTape && <TickerTape/>}

      <div className="app-main">
        {showLeftDom && <DomLadder activeSym={activeSym} onClose={() => setTweak('showDom', false)}/>}

        <main className="content" data-screen-label={labelMap[tweaks.screen] || tweaks.screen}>
          <div className="content-inner">
            <Screen activeSym={activeSym} onPickSym={setActiveSym}/>
          </div>
          {showFloatingTicket && (
            <OrderTicket activeSym={activeSym} onClose={() => setTweak('showOrderTicket', false)}/>
          )}
        </main>

        {tweaks.showSidebar && <Sidebar activeSym={activeSym} onPickSym={setActiveSym}/>}
      </div>

      <TweaksPanel title="Tweaks">
        <TweakSection title="View">
          <TweakSelect
            label="Screen"
            value={tweaks.screen}
            onChange={(v) => setTweak('screen', v)}
            options={[
              { value: 'tiles', label: 'Dashboard' },
              { value: 'apex', label: 'Markets' },
              { value: 'portfolio', label: 'Portfolio' },
              { value: 'plays', label: 'Plays' },
              { value: 'screener', label: 'Screener' },
              { value: 'news', label: 'News' },
              { value: 'alerts', label: 'Alerts' },
            ]}
          />
        </TweakSection>
        <TweakSection title="Layout">
          <TweakToggle label="Sidebar" value={tweaks.showSidebar} onChange={(v) => setTweak('showSidebar', v)}/>
          <TweakToggle label="Ticker tape" value={tweaks.showTickerTape} onChange={(v) => setTweak('showTickerTape', v)}/>
          <TweakToggle label="DOM ladder (Markets)" value={tweaks.showDom} onChange={(v) => setTweak('showDom', v)}/>
          <TweakToggle label="Order ticket (Markets)" value={tweaks.showOrderTicket} onChange={(v) => setTweak('showOrderTicket', v)}/>
        </TweakSection>
      </TweaksPanel>

      <style>{`
        .app { height: 100vh; display: flex; flex-direction: column; background: #000; }
        .app-main { flex: 1; min-height: 0; display: flex; padding: 0 0 8px 8px; }
        .content { flex: 1; min-width: 0; min-height: 0; padding: 8px; position: relative; display: flex; }
        .content-inner {
          flex: 1; min-width: 0; min-height: 0;
          background: var(--bg-elev-1); border-radius: 12px;
          overflow: hidden; display: flex;
        }
      `}</style>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')).render(<App/>);
