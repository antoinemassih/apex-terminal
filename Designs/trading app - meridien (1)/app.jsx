// Main trading terminal app
const { useState: uS, useEffect: uE } = React;

const TWEAK_DEFAULTS = /*EDITMODE-BEGIN*/{
  "theme": "bauhaus",
  "accent": "amber",
  "density": "default",
  "borders": "default",
  "showVol": true,
  "showMA": true,
  "showBB": false,
  "dotmatrix": true,
  "tilesVar": "B",
  "tradeVar": "B",
  "layout": "editorial"
}/*EDITMODE-END*/;

function App() {
  const [tweaks, setTweak] = useTweaks(TWEAK_DEFAULTS);
  const [route, setRoute] = uS('dash');
  const [activeSym, setActiveSym] = uS('NVDA');
  const [cmdOpen, setCmdOpen] = uS(false);
  const [now, setNow] = uS(new Date());
  const [chainOpen, setChainOpen] = uS(false);
  const [newsOpen, setNewsOpen] = uS(false);
  const [socialOpen, setSocialOpen] = uS(false);
  const [portOpen, setPortOpen] = uS(false);
  const [settingsOpen, setSettingsOpen] = uS(false);
  const [orderOpen, setOrderOpen] = uS(false);
  const D = window.TERMINAL_DATA;

  uE(()=>{ const t=setInterval(()=>setNow(new Date()),1000); return ()=>clearInterval(t); }, []);

  uE(()=>{
    function k(e){
      if (e.target.tagName==='INPUT' || e.target.tagName==='TEXTAREA') return;
      if ((e.metaKey||e.ctrlKey) && e.key.toLowerCase()==='k'){ e.preventDefault(); setCmdOpen(v=>!v); }
      if (e.key === '/' && !cmdOpen) { e.preventDefault(); setCmdOpen(true); }
      if (e.key === 'n' && !cmdOpen) { setNewsOpen(v=>!v); }
      if (e.key === 'p' && !cmdOpen) { setPortOpen(v=>!v); }
      if ((e.metaKey||e.ctrlKey) && e.key.toLowerCase()==='o'){ e.preventDefault(); setChainOpen(v=>!v); }
      if ((e.metaKey||e.ctrlKey) && e.shiftKey && e.key.toLowerCase()==='s'){ e.preventDefault(); setSocialOpen(v=>!v); }
    }
    window.addEventListener('keydown', k);
    return ()=>window.removeEventListener('keydown', k);
  }, [cmdOpen]);

  uE(()=>{
    document.documentElement.setAttribute('data-theme', tweaks.theme);
    document.documentElement.setAttribute('data-accent', tweaks.accent);
    document.documentElement.setAttribute('data-density', tweaks.density);
    document.documentElement.setAttribute('data-borders', tweaks.borders || 'default');
  }, [tweaks]);

  const activeRow = D.watchlist.find(w => w.sym === activeSym) || D.watchlist[0];

  function handlePick(p){
    if (p.kind==='symbol') setActiveSym(p.sym);
    if (p.kind==='route')  {
      if (p.route==='settings') setSettingsOpen(true);
      else setRoute(p.route);
    }
    if (p.kind==='page') {
      if (p.page==='port') setRoute('port');
    }
  }

  return (
    <div className="shell">
      <TopBar route={route} setRoute={setRoute} now={now}
        onCmd={()=>setCmdOpen(true)}
        onChain={()=>setChainOpen(true)}
        onNews={()=>setNewsOpen(true)}
        onSocial={()=>setSocialOpen(true)}
        onPort={()=>setPortOpen(true)}
        onSettings={()=>setSettingsOpen(true)}/>
      <TickerTape items={D.indices} />

      {route==='dash'  && tweaks.layout!=='apex' && <Dashboard activeRow={activeRow} setActiveSym={setActiveSym} D={D} dotmatrix={tweaks.dotmatrix}
                                     showVol={tweaks.showVol} showMA={tweaks.showMA} showBB={tweaks.showBB}/>}
      {route==='dash'  && tweaks.layout==='apex' && <ApexLayout activeRow={activeRow} setActiveSym={setActiveSym} D={D} dotmatrix={tweaks.dotmatrix}
                                     showVol={tweaks.showVol} showMA={tweaks.showMA} showBB={tweaks.showBB}/>}
      {route==='apex'  && <ApexLayout activeRow={activeRow} setActiveSym={setActiveSym} D={D} dotmatrix={tweaks.dotmatrix}
                                     showVol={tweaks.showVol} showMA={tweaks.showMA} showBB={tweaks.showBB}/>}
      {route==='chart' && <ChartView activeRow={activeRow} D={D} showVol={tweaks.showVol} showMA={tweaks.showMA} showBB={tweaks.showBB} dotmatrix={tweaks.dotmatrix}/>}
      {route==='trade' && <TradePage activeRow={activeRow} D={D} variant={tweaks.tradeVar} showVol={tweaks.showVol} showMA={tweaks.showMA} showBB={tweaks.showBB} dotmatrix={tweaks.dotmatrix}/>}
      {route==='tiles' && <TilesPage D={D} variant={tweaks.tilesVar}/>}
      {route==='port'  && <PortfolioPage D={D}/>}
      {route==='research' && <ResearchPage D={D}/>}
      {route==='risk' && <RiskPage D={D}/>}
      {route==='options' && <OptionsPage D={D} activeRow={activeRow}/>}

      <StatusBar now={now} sym={activeSym} />

      <CommandPaletteV2 open={cmdOpen} onClose={()=>setCmdOpen(false)} onPick={handlePick}/>
      <SettingsModal open={settingsOpen} onClose={()=>setSettingsOpen(false)} tweaks={tweaks} setTweak={setTweak}/>

      <OptionsChainSidebar open={chainOpen} onClose={()=>setChainOpen(false)} sym={activeSym} last={activeRow.last}/>
      <NewsFeedSidebar open={newsOpen} onClose={()=>setNewsOpen(false)} items={D.news}/>
      <SocialFeedSidebar open={socialOpen} onClose={()=>setSocialOpen(false)}/>
      <PortfolioSidebar open={portOpen} onClose={()=>setPortOpen(false)} D={D}/>

      <TweaksPanel>
        <TweakSection label="Theme">
          <TweakSelect label="Palette" value={tweaks.theme} options={[
            {label:'Bauhaus · cream',value:'bauhaus'},
            {label:'Monokai',value:'monokai'},
            {label:'Nord',value:'nord'},
            {label:'Midnight',value:'midnight'},
            {label:'Solarized',value:'solarized'},
            {label:'Gruvbox',value:'gruvbox'},
            {label:'Rosé Pine',value:'rosepine'},
            {label:'Kanagawa',value:'kanagawa'},
          ]} onChange={v=>setTweak('theme', v)}/>
          <TweakSelect label="Accent" value={tweaks.accent} options={[{label:'Amber',value:'amber'},{label:'Cobalt',value:'cobalt'},{label:'Green',value:'green'},{label:'Mono',value:'black'}]} onChange={v=>setTweak('accent', v)}/>
          <TweakRadio label="Density" value={tweaks.density} options={[{label:'Compact',value:'compact'},{label:'Default',value:'default'},{label:'Comfy',value:'comfy'}]} onChange={v=>setTweak('density', v)}/>
          <TweakRadio label="Borders" value={tweaks.borders||'default'} options={[{label:'Off',value:'off'},{label:'Subtle',value:'subtle'},{label:'Default',value:'default'},{label:'Strong',value:'strong'}]} onChange={v=>setTweak('borders', v)}/>
        </TweakSection>
        <TweakSection label="Chart">
          <TweakToggle label="Volume" value={tweaks.showVol} onChange={v=>setTweak('showVol', v)}/>
          <TweakToggle label="Moving avgs" value={tweaks.showMA} onChange={v=>setTweak('showMA', v)}/>
          <TweakToggle label="Bollinger" value={tweaks.showBB} onChange={v=>setTweak('showBB', v)}/>
        </TweakSection>
        <TweakSection label="Editorial">
          <TweakToggle label="Data callouts" value={tweaks.dotmatrix} onChange={v=>setTweak('dotmatrix', v)}/>
        </TweakSection>
        <TweakSection label="Layout">
          <TweakRadio label="Dashboard layout" value={tweaks.layout} options={[{label:'Editorial',value:'editorial'},{label:'Apex',value:'apex'}]} onChange={v=>setTweak('layout', v)}/>
          <TweakSelect label="View" value={route} options={[
            {label:'Dashboard',value:'dash'},
            {label:'Apex Layout',value:'apex'},
            {label:'Chart · Analysis',value:'chart'},
            {label:'Trade · Floating',value:'trade'},
            {label:'Tiles · Bold',value:'tiles'},
            {label:'Portfolio',value:'port'},
            {label:'Research',value:'research'},
            {label:'Risk',value:'risk'},
            {label:'Options',value:'options'},
          ]} onChange={v=>setRoute(v)}/>
          <TweakRadio label="Tiles variant" value={tweaks.tilesVar} options={[{label:'A · Editorial',value:'A'},{label:'B · Burst',value:'B'},{label:'C · Mosaic',value:'C'}]} onChange={v=>setTweak('tilesVar', v)}/>
          <TweakRadio label="Trade widgets" value={tweaks.tradeVar} options={[{label:'A · Pills',value:'A'},{label:'B · Burst',value:'B'}]} onChange={v=>setTweak('tradeVar', v)}/>
        </TweakSection>
        <TweakSection label="Sidebars">
          <TweakButton label="Options chain" onClick={()=>setChainOpen(true)}/>
          <TweakButton label="News feed" onClick={()=>setNewsOpen(true)}/>
          <TweakButton label="Social feed" onClick={()=>setSocialOpen(true)}/>
          <TweakButton label="Portfolio" onClick={()=>setPortOpen(true)}/>
          <TweakButton label="Settings modal" onClick={()=>setSettingsOpen(true)}/>
          <TweakButton label="Command palette" onClick={()=>setCmdOpen(true)}/>
        </TweakSection>
      </TweaksPanel>
    </div>
  );
}

function TopBar({ route, setRoute, now, onCmd, onChain, onNews, onSocial, onPort, onSettings }){
  const t = now.toTimeString().slice(0,8);
  return (
    <div className="topbar" data-screen-label="Top Bar">
      <div className="brand">
        <div className="mark">M</div>
        <div>
          <div className="name">MERIDIAN</div>
          <div className="sub">Trading Terminal · v4.2</div>
        </div>
      </div>
      <div className="nav">
        <div className={'nav-item '+(route==='dash'?'on':'')} onClick={()=>setRoute('dash')}>Dashboard</div>
        <div className={'nav-item '+(route==='chart'?'on':'')} onClick={()=>setRoute('chart')}>Chart</div>
        <div className={'nav-item '+(route==='trade'?'on':'')} onClick={()=>setRoute('trade')}>Trade</div>
        <div className={'nav-item '+(route==='apex'?'on':'')} onClick={()=>setRoute('apex')}>Apex</div>
        <div className={'nav-item '+(route==='tiles'?'on':'')} onClick={()=>setRoute('tiles')}>Tiles</div>
        <div className={'nav-item '+(route==='port'?'on':'')} onClick={()=>setRoute('port')}>Portfolio</div>
        <div className={'nav-item '+(route==='research'?'on':'')} onClick={()=>setRoute('research')}>Research</div>
        <div className={'nav-item '+(route==='risk'?'on':'')} onClick={()=>setRoute('risk')}>Risk</div>
        <div className={'nav-item '+(route==='options'?'on':'')} onClick={()=>setRoute('options')}>Options</div>
      </div>
      <div className="topright">
        <div className="quick-actions">
          <div className="qa" onClick={onNews} title="News (N)">News</div>
          <div className="qa" onClick={onSocial} title="Social (⌘⇧S)">Social</div>
          <div className="qa" onClick={onChain} title="Chain (⌘O)">Chain</div>
          <div className="qa" onClick={onPort} title="Portfolio (P)">Portfolio</div>
          <div className="qa" onClick={onSettings} title="Settings">Settings</div>
        </div>
        <div className="kbd-hint" onClick={onCmd} style={{cursor:'pointer'}}>
          <span>Type a function</span>
          <kbd>⌘</kbd><kbd>K</kbd>
        </div>
        <div className="session">
          <div className="row">
            <span className="live"></span>
            <span className="label">NYSE OPEN</span>
            <span style={{marginLeft:'auto'}} className="mono" >{t} ET</span>
          </div>
          <div className="row" style={{marginTop:4, color:'var(--muted)'}}>
            <span className="label">USR</span>
            <span className="mono" style={{fontSize:11}}>j.morgan@meridian</span>
          </div>
        </div>
      </div>
    </div>
  );
}

function StatusBar({ now, sym }){
  return (
    <div className="statusbar">
      <div><span className="acc">●</span> Stream OK · 0ms</div>
      <div>Latency 4ms</div>
      <div>Region: NY-4</div>
      <div>Subscribed: {sym} · NASDAQ TotalView</div>
      <div>Buying power $5.83M</div>
      <div className="right">{now.toISOString().slice(0,19).replace('T',' ')} UTC</div>
    </div>
  );
}

function Dashboard({ activeRow, setActiveSym, D, dotmatrix, showVol, showMA, showBB }) {
  return (
    <div className="workspace" style={{
      gridTemplateColumns: '300px 1fr 360px',
      gridTemplateRows: 'auto minmax(0, 1.1fr) minmax(0, 1fr) minmax(0, 0.9fr)',
    }}>
      <div style={{gridColumn:'1 / -1'}} data-screen-label="01 Hero">
        <Hero row={activeRow} candles={D.candles}/>
      </div>

      <div style={{gridColumn:'1 / 2', gridRow:'2 / 3'}}>
        <Panel num="01" title="Watchlist" sub="Top 12">
          <Watchlist data={D.watchlist} active={activeRow.sym} onSelect={setActiveSym}/>
        </Panel>
      </div>

      <div style={{gridColumn:'2 / 3', gridRow:'2 / 3'}}>
        <Panel num="02" title={`${activeRow.sym} · 1D`} sub="Candles · 5m"
          noScroll
          actions={
            <div style={{display:'flex', gap:0, alignItems:'center'}}>
              {['1m','5m','15m','1H','1D','1W'].map((tf,i)=>(
                <span key={tf} className="mono" style={{
                  fontSize:10, padding:'2px 8px', color: tf==='5m'?'var(--ink)':'var(--muted)',
                  background: tf==='5m'?'var(--paper)':'transparent', cursor:'pointer'
                }}>{tf}</span>
              ))}
            </div>
          }
        >
          <div style={{position:'relative', width:'100%', height:'100%'}}>
            <Candles candles={D.candles} width={900} height={420}
                     showVol={showVol} showMA={showMA} showBB={showBB}/>
            {dotmatrix && (
              <div style={{
                position:'absolute', top:14, left:64, display:'flex', gap:14, alignItems:'center',
                fontFamily:'JetBrains Mono', fontSize:10, color:'var(--muted)', letterSpacing:'.1em', pointerEvents:'none'
              }}>
                <span>O <span style={{color:'var(--ink)'}}>{D.candles[D.candles.length-1].o.toFixed(2)}</span></span>
                <span>H <span style={{color:'var(--ink)'}}>{D.candles[D.candles.length-1].h.toFixed(2)}</span></span>
                <span>L <span style={{color:'var(--ink)'}}>{D.candles[D.candles.length-1].l.toFixed(2)}</span></span>
                <span>C <span style={{color:'var(--ink)'}}>{D.candles[D.candles.length-1].c.toFixed(2)}</span></span>
                {showMA && <span style={{color:'var(--muted)'}}>MA20 · <span style={{color:'var(--ink)'}}>1212.18</span></span>}
                {showMA && <span style={{color:'var(--muted)'}}>MA50 · <span className="acc">1184.62</span></span>}
              </div>
            )}
          </div>
        </Panel>
      </div>

      <div style={{gridColumn:'3 / 4', gridRow:'2 / 3'}}>
        <Panel num="03" title="Order Book" sub="Lvl 2 · 14 deep">
          <OrderBook ob={D.ob}/>
        </Panel>
      </div>

      <div style={{gridColumn:'1 / 2', gridRow:'3 / 4'}}>
        <Panel num="04" title="News" sub="Live">
          <News items={D.news.slice(0,7)}/>
        </Panel>
      </div>

      <div style={{gridColumn:'2 / 3', gridRow:'3 / 4', display:'grid', gridTemplateColumns:'1.4fr 1fr'}}>
        <div style={{borderRight:'1px solid var(--rule)'}}>
        <Panel num="05" title="Sector Heatmap" sub="S&P 500 · today" noScroll>
            <Heatmap data={D.heatmap}/>
          </Panel>
        </div>
        <div>
          <Panel num="06" title="Time & Sales" sub={activeRow.sym}>
            <Tape rows={D.tape}/>
          </Panel>
        </div>
      </div>

      <div style={{gridColumn:'3 / 4', gridRow:'3 / 4'}}>
        <Panel num="07" title="Order Ticket" sub={activeRow.sym}>
          <OrderTicket symbol={activeRow.sym} last={activeRow.last}/>
        </Panel>
      </div>

      <div style={{gridColumn:'1 / -1', gridRow:'4 / 5', borderTop:'1px solid var(--rule)'}}>
        <Panel num="08" title="Positions" sub="7 open · long-short">
          <Positions rows={D.positions}/>
        </Panel>
      </div>
    </div>
  );
}

function ChartView({ activeRow, D, showVol, showMA, showBB, dotmatrix }) {
  const last = D.candles[D.candles.length-1];
  return (
    <div className="analysis" data-screen-label="02 Chart Analysis">
      <div className="panel" style={{gridColumn:'1 / -1', borderRight:0, padding:'24px 28px 18px'}}>
        <div style={{display:'grid', gridTemplateColumns:'auto 1fr auto auto auto auto', gap:32, alignItems:'end'}}>
          <div>
            <div className="label">Equity · NASDAQ · USD</div>
            <div className="mono" style={{fontSize:14, fontWeight:600, marginTop:4}}>{activeRow.sym}</div>
            <div style={{color:'var(--muted)', fontSize:13, marginTop:2}}>{activeRow.name}</div>
          </div>
          <div>
            <div className="bignum mono" style={{fontSize:'clamp(56px,7vw,120px)'}}>{fmt(last.c,2)}</div>
            <div className={'mono '+(activeRow.chg>=0?'up':'down')} style={{fontSize:18, marginTop:2}}>
              {fmtSign(activeRow.chg)} · {fmtSign(activeRow.pct)}%
            </div>
          </div>
          {[
            {k:'OPEN', v: fmt(last.o)},
            {k:'HIGH', v: fmt(last.h)},
            {k:'LOW',  v: fmt(last.l)},
            {k:'VOL',  v: activeRow.vol},
          ].map((s,i)=>(
            <div key={i} style={{borderLeft:'1px solid var(--hair)', paddingLeft:18}}>
              <div className="label">{s.k}</div>
              <div className="midnum mono" style={{fontSize:28, marginTop:6}}>{s.v}</div>
            </div>
          ))}
        </div>
      </div>

      <div className="panel" style={{gridColumn:'1 / 2', gridRow:'2 / 3'}}>
        <div className="chart-toolbar">
          <div className="grp">
            {['1m','5m','15m','30m','1H','4H','1D','1W','1M'].map(tf => (
              <div key={tf} className={'tf '+(tf==='1D'?'on':'')}>{tf}</div>
            ))}
          </div>
          <div className="grp">
            {['Candle','Line','Area','Bars'].map(t => (
              <div key={t} className={'tf '+(t==='Candle'?'on':'')}>{t}</div>
            ))}
          </div>
          <div className="grp">
            <div className="tf">＋ Indicator</div>
            <div className="tf">⊞ Compare</div>
            <div className="tf">⤓ Export</div>
          </div>
        </div>
        <div className="chart-body">
          <div className="chart-tools">
            {['↖','│','╱','◯','▭','T','⌖','⊕','▾','⊘'].map((g,i)=>(
              <div key={i} className={'t '+(i===2?'on':'')}>{g}</div>
            ))}
          </div>
          <div className="chart-canvas">
            <Candles candles={D.candles} width={1100} height={520}
                     showVol={showVol} showMA={showMA} showBB={showBB}/>
            {dotmatrix && (
              <div style={{position:'absolute', top:12, left:18, display:'flex', gap:18, fontFamily:'JetBrains Mono', fontSize:11, color:'var(--muted)', letterSpacing:'.06em'}}>
                <span>{activeRow.sym} · NASDAQ · 1D · {fmt(last.c)}</span>
                {showMA && <span>MA(20) <span style={{color:'var(--ink)'}}>1212.18</span></span>}
                {showMA && <span>MA(50) <span className="acc">1184.62</span></span>}
                {showBB && <span>BB(20,2)</span>}
              </div>
            )}
          </div>
        </div>
      </div>

      <div className="panel" style={{gridColumn:'2 / 3', gridRow:'2 / 3', borderRight:0, display:'grid', gridTemplateRows:'auto auto 1fr'}}>
        <div style={{borderBottom:'1px solid var(--rule)'}}>
          <Panel num="A" title="Technicals" sub="Daily">
            <div style={{padding:'14px 18px', display:'grid', gridTemplateColumns:'1fr 1fr', gap:'14px 22px'}}>
              {[
                {k:'RSI(14)', v:'62.4', s:'NEUTRAL', c:'var(--muted)'},
                {k:'MACD',    v:'+8.42', s:'BULL', c:'var(--up)'},
                {k:'STOCH',   v:'78.1', s:'OVERBOUGHT', c:'var(--down)'},
                {k:'ADX',     v:'31.6', s:'TREND', c:'var(--ink)'},
                {k:'ATR(14)', v:'21.84',s:'HIGH', c:'var(--down)'},
                {k:'BETA',    v:'1.84', s:'HIGH', c:'var(--down)'},
              ].map((m,i)=>(
                <div key={i}>
                  <div className="label">{m.k}</div>
                  <div className="mono" style={{fontSize:18, marginTop:2}}>{m.v}</div>
                  <div className="mono" style={{fontSize:9, letterSpacing:'.12em', color:m.c}}>{m.s}</div>
                </div>
              ))}
            </div>
          </Panel>
        </div>
        <div style={{borderBottom:'1px solid var(--rule)'}}>
          <Panel num="B" title="Order Ticket" sub={activeRow.sym}>
            <OrderTicket symbol={activeRow.sym} last={activeRow.last}/>
          </Panel>
        </div>
        <Panel num="C" title="Options Chain" sub="May 15 · ATM ±5">
          <OptionsChain rows={D.optsChain} spot={activeRow.last}/>
        </Panel>
      </div>

      <div className="panel" style={{gridColumn:'1 / -1', gridRow:'3 / 4', display:'grid', gridTemplateColumns:'1.5fr 1fr 1fr', borderRight:0, borderBottom:0}}>
        <div style={{borderRight:'1px solid var(--rule)'}}>
          <Panel num="D" title="Time & Sales" sub={activeRow.sym}>
            <Tape rows={D.tape.slice(0,12)}/>
          </Panel>
        </div>
        <div style={{borderRight:'1px solid var(--rule)'}}>
          <Panel num="E" title="News" sub="Filter: NVDA + sector">
            <News items={D.news.slice(0,5)}/>
          </Panel>
        </div>
        <div>
          <Panel num="F" title="Order Book" sub="Lvl 2">
            <div style={{maxHeight: 360, overflow:'auto'}}>
              <OrderBook ob={D.ob}/>
            </div>
          </Panel>
        </div>
      </div>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')).render(<App/>);
