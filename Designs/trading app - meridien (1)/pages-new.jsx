// Portfolio, Research, Risk, Settings, Options Chain, and Order Ticket modal
const { useState: nS, useEffect: nE } = React;

// ---------- Helpers ----------
function fSign(n, d=2){ const s=n>=0?'+':''; return s+n.toLocaleString('en-US',{minimumFractionDigits:d, maximumFractionDigits:d}); }
function fNum(n, d=2){ return n.toLocaleString('en-US',{minimumFractionDigits:d, maximumFractionDigits:d}); }

// =========================================================
// PORTFOLIO PAGE
// =========================================================
function PortfolioPage({ D }){
  const E = window.TERMINAL_DATA_EXTRA;
  const totMv = D.positions.reduce((a,b)=>a+b.mv,0);
  const totPl = D.positions.reduce((a,b)=>a+b.pl,0);
  return (
    <div style={{flex:1, minHeight:0, overflow:'auto', background:'var(--bg)'}} data-screen-label="05 Portfolio">
      {/* Hero strip */}
      <div style={{display:'grid', gridTemplateColumns:'1.4fr 1fr 1fr 1fr', borderBottom:'1px solid var(--rule)'}}>
        <div style={{padding:'24px 28px', borderRight:'1px solid var(--rule)'}}>
          <div className="label">Total equity · Main account</div>
          <div className="bignum mono" style={{fontSize:'clamp(56px,7vw,128px)', marginTop:8}}>$5.83M</div>
          <div className="mono up" style={{fontSize:18, marginTop:6}}>+$98,420 · +1.72% today</div>
        </div>
        {[
          {k:'Net P&L · YTD', v:'+$842,180', s:'+16.8%', c:'var(--up)'},
          {k:'Realized · MTD', v:'+$184,200', s:'+3.2%',  c:'var(--up)'},
          {k:'Unrealized',     v:'+$290,794', s:'+5.1%',  c:'var(--up)'},
        ].map((s,i)=>(
          <div key={i} style={{padding:'24px 28px', borderRight:i<2?'1px solid var(--rule)':0}}>
            <div className="label">{s.k}</div>
            <div className="midnum mono" style={{fontSize:42, marginTop:8}}>{s.v}</div>
            <div className="mono" style={{fontSize:13, color:s.c, marginTop:4}}>{s.s}</div>
          </div>
        ))}
      </div>

      {/* Body grid */}
      <div style={{display:'grid', gridTemplateColumns:'1.6fr 1fr', borderBottom:'1px solid var(--rule)'}}>
        {/* Performance chart */}
        <div style={{borderRight:'1px solid var(--rule)', padding:'18px 22px'}}>
          <div style={{display:'flex', justifyContent:'space-between', alignItems:'baseline', marginBottom:14}}>
            <div>
              <div className="label">Performance · 252 day</div>
              <div style={{fontSize:18, fontWeight:600, marginTop:2}}>Account vs S&P 500</div>
            </div>
            <div style={{display:'flex', gap:0, border:'1px solid var(--hair)'}}>
              {['1W','1M','3M','YTD','1Y','ALL'].map((p,i)=>(
                <div key={p} className="mono" style={{
                  padding:'6px 12px', fontSize:10, letterSpacing:'.1em',
                  color: p==='1Y'?'var(--ink)':'var(--muted)',
                  background: p==='1Y'?'var(--paper)':'transparent',
                  borderRight: i<5?'1px solid var(--hair)':0, cursor:'pointer'
                }}>{p}</div>
              ))}
            </div>
          </div>
          <PerfChart data={E.perfHistory}/>
        </div>

        {/* Sector allocation donut */}
        <div style={{padding:'18px 22px'}}>
          <div className="label">Sector allocation</div>
          <div style={{fontSize:18, fontWeight:600, marginTop:2, marginBottom:14}}>By weight</div>
          <div style={{display:'grid', gridTemplateColumns:'180px 1fr', gap:18, alignItems:'center'}}>
            <Donut data={E.allocSector}/>
            <div style={{display:'flex', flexDirection:'column', gap:6}}>
              {E.allocSector.map(s=>(
                <div key={s.k} style={{display:'grid', gridTemplateColumns:'10px 1fr auto auto', gap:10, alignItems:'center', fontFamily:'JetBrains Mono', fontSize:11}}>
                  <div style={{width:8, height:8, background:s.c}}/>
                  <div>{s.k}</div>
                  <div style={{color:'var(--muted)'}}>{s.w}%</div>
                  <div className={s.pl>=0?'up':'down'} style={{minWidth:46, textAlign:'right'}}>{fSign(s.pl,1)}</div>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>

      {/* Positions table */}
      <Positions rows={D.positions}/>
    </div>
  );
}

function PerfChart({ data }){
  const W=900, H=240, pad=20;
  const minP=Math.min(...data.map(d=>Math.min(d.p,d.b)));
  const maxP=Math.max(...data.map(d=>Math.max(d.p,d.b)));
  const x=i=> pad + i/(data.length-1) * (W-pad*2);
  const y=v=> H-pad - (v-minP)/(maxP-minP) * (H-pad*2);
  const path=(key)=> data.map((d,i)=>(i?'L':'M')+x(i).toFixed(1)+' '+y(d[key]).toFixed(1)).join(' ');
  return (
    <svg viewBox={`0 0 ${W} ${H}`} style={{width:'100%', height:240, display:'block'}}>
      {[0,1,2,3,4].map(i=><line key={i} x1={pad} x2={W-pad} y1={pad+(H-pad*2)*i/4} y2={pad+(H-pad*2)*i/4} stroke="var(--hair-2)"/>)}
      <path d={path('b')} fill="none" stroke="var(--muted)" strokeWidth="1" strokeDasharray="3 3"/>
      <path d={path('p')} fill="none" stroke="var(--accent)" strokeWidth="1.6"/>
      <text x={W-pad} y={y(data[data.length-1].p)-6} fontSize="10" fontFamily="JetBrains Mono" fill="var(--accent)" textAnchor="end">ACCT {data[data.length-1].p.toFixed(1)}</text>
      <text x={W-pad} y={y(data[data.length-1].b)+12} fontSize="10" fontFamily="JetBrains Mono" fill="var(--muted)" textAnchor="end">SPX {data[data.length-1].b.toFixed(1)}</text>
    </svg>
  );
}

function Donut({ data }){
  const total = data.reduce((a,b)=>a+b.w,0);
  const cx=90, cy=90, r=70, ir=42;
  let acc=0;
  return (
    <svg viewBox="0 0 180 180" style={{width:180, height:180}}>
      {data.map((d,i)=>{
        const a0 = acc/total * Math.PI*2 - Math.PI/2;
        acc += d.w;
        const a1 = acc/total * Math.PI*2 - Math.PI/2;
        const large = (a1-a0) > Math.PI ? 1 : 0;
        const x0=cx+r*Math.cos(a0), y0=cy+r*Math.sin(a0);
        const x1=cx+r*Math.cos(a1), y1=cy+r*Math.sin(a1);
        const ix0=cx+ir*Math.cos(a1), iy0=cy+ir*Math.sin(a1);
        const ix1=cx+ir*Math.cos(a0), iy1=cy+ir*Math.sin(a0);
        return <path key={i} d={`M${x0} ${y0} A${r} ${r} 0 ${large} 1 ${x1} ${y1} L${ix0} ${iy0} A${ir} ${ir} 0 ${large} 0 ${ix1} ${iy1} Z`} fill={d.c} stroke="var(--bg)" strokeWidth="1.5"/>;
      })}
      <text x={cx} y={cy-2} textAnchor="middle" fontSize="20" fontWeight="600" fontFamily="Inter Tight">$5.83M</text>
      <text x={cx} y={cy+14} textAnchor="middle" fontSize="9" fill="var(--muted)" fontFamily="JetBrains Mono" letterSpacing="2">EQUITY</text>
    </svg>
  );
}

// =========================================================
// RESEARCH PAGE
// =========================================================
function ResearchPage({ D }){
  const E = window.TERMINAL_DATA_EXTRA;
  return (
    <div style={{flex:1, minHeight:0, overflow:'auto', background:'var(--bg)'}} data-screen-label="06 Research">
      <div style={{padding:'24px 28px', borderBottom:'1px solid var(--rule)'}}>
        <div className="label">Research desk · Equity</div>
        <div style={{fontSize:'clamp(36px,4.5vw,72px)', fontWeight:500, letterSpacing:'-0.03em', lineHeight:.95, marginTop:8}}>
          The morning brief.
        </div>
        <div style={{fontSize:14, color:'var(--muted)', marginTop:8, maxWidth:680, lineHeight:1.4}}>
          Curated analyst notes, ratings, earnings calendar and macro monitor — synced 09:30 ET.
        </div>
      </div>

      <div style={{display:'grid', gridTemplateColumns:'1.3fr 1fr', borderBottom:'1px solid var(--rule)'}}>
        {/* Ratings table */}
        <div style={{borderRight:'1px solid var(--rule)'}}>
          <div className="panel-head">
            <div className="ttl"><span className="num">A1</span><span className="nm">Ratings actions · today</span></div>
            <div className="actions"><span className="mono" style={{fontSize:10, color:'var(--muted)'}}>6 NEW</span></div>
          </div>
          <div style={{display:'grid', gridTemplateColumns:'70px 1fr 110px 90px 110px 70px', fontFamily:'JetBrains Mono', fontSize:11}}>
            {['SYM','FIRM','ACTION','PT','PREV PT','DATE'].map(h=>(
              <div key={h} style={{padding:'10px 12px', fontSize:9, letterSpacing:'.1em', color:'var(--muted)', borderBottom:'1px solid var(--hair)'}}>{h}</div>
            ))}
            {E.ratings.map((r,i)=>(
              <React.Fragment key={i}>
                <div style={{padding:'12px', fontWeight:600, borderBottom:'1px solid var(--hair-2)'}}>{r.sym}</div>
                <div style={{padding:'12px', borderBottom:'1px solid var(--hair-2)'}}>{r.firm}</div>
                <div style={{padding:'12px', borderBottom:'1px solid var(--hair-2)', color: r.act==='Buy'||r.act==='Outperform'?'var(--up)':r.act==='Underweight'?'var(--down)':'var(--muted)'}}>{r.act}</div>
                <div style={{padding:'12px', borderBottom:'1px solid var(--hair-2)', textAlign:'right'}}>${r.pt}</div>
                <div style={{padding:'12px', borderBottom:'1px solid var(--hair-2)', textAlign:'right', color:'var(--muted)'}}>${r.prev}</div>
                <div style={{padding:'12px', borderBottom:'1px solid var(--hair-2)', color:'var(--muted)'}}>{r.date}</div>
              </React.Fragment>
            ))}
          </div>
        </div>

        {/* Earnings */}
        <div>
          <div className="panel-head">
            <div className="ttl"><span className="num">A2</span><span className="nm">Earnings calendar</span></div>
          </div>
          <div style={{padding:'8px 0'}}>
            {E.earnings.map((e,i)=>(
              <div key={i} style={{display:'grid', gridTemplateColumns:'70px 100px 1fr 80px', padding:'14px 18px', borderBottom:'1px solid var(--hair-2)', alignItems:'center'}}>
                <div className="mono" style={{fontWeight:600}}>{e.sym}</div>
                <div className="mono" style={{fontSize:11, color:'var(--muted)'}}>{e.date}</div>
                <div className="mono" style={{fontSize:11}}>EPS <span style={{color:'var(--muted)'}}>{e.ept}</span> → <span>{e.actl}</span> · REV {e.rev}</div>
                <div className={'mono '+(e.surp>=0?'up':'down')} style={{textAlign:'right', fontSize:13, fontWeight:600}}>{fSign(e.surp,1)}%</div>
              </div>
            ))}
          </div>
        </div>
      </div>

      <div style={{display:'grid', gridTemplateColumns:'1fr 1fr 1fr'}}>
        <div style={{borderRight:'1px solid var(--rule)', padding:'18px 22px'}}>
          <div className="label">Macro monitor</div>
          <div style={{fontSize:16, fontWeight:600, marginTop:4, marginBottom:14}}>Fed-watch · CPI · jobs</div>
          {[
            {k:'Fed Dec rate cut prob.', v:'62%', d:'Up 4pts wk/wk'},
            {k:'10Y / 2Y spread',          v:'+0.18',d:'Steepening'},
            {k:'Real yield (10Y)',         v:'1.86%',d:'Flat'},
            {k:'CPI nowcast (Cleveland)',  v:'+2.4%',d:'Cooling'},
            {k:'NFP estimate',             v:'+185K',d:'Consensus'},
          ].map((m,i)=>(
            <div key={i} style={{display:'grid', gridTemplateColumns:'1fr auto', padding:'10px 0', borderBottom:'1px dashed var(--hair)'}}>
              <div>
                <div style={{fontSize:12}}>{m.k}</div>
                <div className="mono" style={{fontSize:10, color:'var(--muted)', marginTop:2}}>{m.d}</div>
              </div>
              <div className="mono" style={{fontSize:18, fontWeight:600}}>{m.v}</div>
            </div>
          ))}
        </div>

        <div style={{borderRight:'1px solid var(--rule)', padding:'18px 22px'}}>
          <div className="label">Top movers · S&P</div>
          <div style={{fontSize:16, fontWeight:600, marginTop:4, marginBottom:14}}>Gainers / losers</div>
          <div style={{display:'grid', gridTemplateColumns:'1fr 1fr', gap:18}}>
            <div>
              <div className="label" style={{color:'var(--up)'}}>↑ GAINERS</div>
              {[['SMCI','+8.2%'],['PLTR','+5.1%'],['CRWD','+4.4%'],['MU','+3.9%'],['AMD','+2.6%']].map(([s,p])=>(
                <div key={s} style={{display:'flex', justifyContent:'space-between', padding:'6px 0', borderBottom:'1px solid var(--hair-2)', fontFamily:'JetBrains Mono', fontSize:11}}>
                  <span style={{fontWeight:600}}>{s}</span><span className="up">{p}</span>
                </div>
              ))}
            </div>
            <div>
              <div className="label" style={{color:'var(--down)'}}>↓ LOSERS</div>
              {[['TSLA','-3.9%'],['BA','-2.2%'],['DIS','-1.2%'],['LLY','-1.0%'],['AAPL','-0.8%']].map(([s,p])=>(
                <div key={s} style={{display:'flex', justifyContent:'space-between', padding:'6px 0', borderBottom:'1px solid var(--hair-2)', fontFamily:'JetBrains Mono', fontSize:11}}>
                  <span style={{fontWeight:600}}>{s}</span><span className="down">{p}</span>
                </div>
              ))}
            </div>
          </div>
        </div>

        <div style={{padding:'18px 22px'}}>
          <div className="label">Editor's notes</div>
          <div style={{fontSize:16, fontWeight:600, marginTop:4, marginBottom:14}}>Desk view</div>
          <div style={{fontSize:13, lineHeight:1.55, color:'var(--ink-2)'}}>
            Semis lead the tape on durable AI cap-ex commentary; Blackwell ramp credibly de-risks H2.
            We stay constructive on <b>NVDA</b>, lean cautious into <b>TSLA</b> deliveries, and watch
            <b> AVGO</b> for follow-through after the custom-silicon win.
          </div>
          <div style={{marginTop:18, display:'flex', gap:8, flexWrap:'wrap'}}>
            {['#semis','#ai-capex','#fed-watch','#china-cars','#hedge'].map(t=>(
              <div key={t} className="mono" style={{padding:'4px 10px', border:'1px solid var(--hair)', fontSize:10, letterSpacing:'.08em', color:'var(--muted)'}}>{t}</div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

// =========================================================
// RISK PAGE
// =========================================================
function RiskPage({ D }){
  const E = window.TERMINAL_DATA_EXTRA;
  return (
    <div style={{flex:1, minHeight:0, overflow:'auto', background:'var(--bg)'}} data-screen-label="07 Risk">
      <div style={{display:'grid', gridTemplateColumns:'1fr 1fr 1fr 1fr', borderBottom:'1px solid var(--rule)'}}>
        {[
          {k:'VaR (95%, 1d)', v:'$184,400', s:'3.16% of equity', c:'var(--down)'},
          {k:'CVaR (97.5%)',  v:'$284,200', s:'4.87% of equity', c:'var(--down)'},
          {k:'Vol · annualized', v:'18.4%',   s:'30d realized', c:'var(--ink)'},
          {k:'Sharpe · YTD',  v:'2.41',    s:'Benchmark 1.18', c:'var(--up)'},
        ].map((s,i)=>(
          <div key={i} style={{padding:'24px 28px', borderRight:i<3?'1px solid var(--rule)':0}}>
            <div className="label">{s.k}</div>
            <div className="bignum mono" style={{fontSize:'clamp(40px,4.4vw,76px)', marginTop:8, color:s.c}}>{s.v}</div>
            <div className="mono" style={{fontSize:11, color:'var(--muted)', marginTop:6}}>{s.s}</div>
          </div>
        ))}
      </div>

      <div style={{display:'grid', gridTemplateColumns:'1.2fr 1fr', borderBottom:'1px solid var(--rule)'}}>
        {/* Stress tests */}
        <div style={{borderRight:'1px solid var(--rule)'}}>
          <div className="panel-head">
            <div className="ttl"><span className="num">R1</span><span className="nm">Stress scenarios</span></div>
            <div className="actions"><span className="mono" style={{fontSize:10, color:'var(--muted)'}}>8 SCENARIOS</span></div>
          </div>
          <div style={{padding:'14px 22px'}}>
            {E.riskScenarios.map((s,i)=>{
              const w = Math.min(100, Math.abs(s.pct)/15*100);
              return (
                <div key={i} style={{display:'grid', gridTemplateColumns:'160px 1fr 130px 80px', alignItems:'center', gap:14, padding:'12px 0', borderBottom:'1px dashed var(--hair)'}}>
                  <div style={{fontSize:13}}>{s.k}</div>
                  <div style={{position:'relative', height:14, background:'var(--paper)', border:'1px solid var(--hair)'}}>
                    <div style={{position:'absolute', left:`${100-w}%`, width:`${w}%`, top:0, bottom:0, background:'var(--down)', opacity:.7}}/>
                  </div>
                  <div className="mono down" style={{textAlign:'right', fontSize:14, fontWeight:600}}>{fSign(s.pl,0)}</div>
                  <div className="mono down" style={{textAlign:'right', fontSize:14}}>{fSign(s.pct,1)}%</div>
                </div>
              );
            })}
          </div>
        </div>

        {/* Greeks + exposure */}
        <div>
          <div className="panel-head">
            <div className="ttl"><span className="num">R2</span><span className="nm">Portfolio greeks</span></div>
          </div>
          <div style={{padding:'18px 22px', display:'grid', gridTemplateColumns:'1fr 1fr 1fr', gap:18}}>
            {E.riskGreeks.map((g,i)=>(
              <div key={i}>
                <div className="label">{g.k}</div>
                <div className="midnum mono" style={{fontSize:28, marginTop:4}}>{g.v}</div>
                <div className="mono" style={{fontSize:9, color:'var(--muted)', marginTop:2}}>{g.n}</div>
              </div>
            ))}
          </div>
          <div className="panel-head" style={{borderTop:'1px solid var(--rule)'}}>
            <div className="ttl"><span className="num">R3</span><span className="nm">Net exposure · long-short</span></div>
          </div>
          <div style={{padding:'14px 22px'}}>
            {E.exposureBySym.map((p,i)=>{
              const wL = Math.abs(p.long)/2.0*100;
              const wS = Math.abs(p.short)/2.0*100;
              return (
                <div key={i} style={{display:'grid', gridTemplateColumns:'70px 1fr 1fr 90px', gap:10, alignItems:'center', padding:'8px 0', borderBottom:'1px solid var(--hair-2)', fontFamily:'JetBrains Mono', fontSize:11}}>
                  <div style={{fontWeight:600}}>{p.sym}</div>
                  <div style={{display:'flex', justifyContent:'flex-end'}}>
                    <div style={{width:`${wS}%`, height:10, background:'var(--down)', opacity:.7}}/>
                  </div>
                  <div style={{display:'flex'}}>
                    <div style={{width:`${wL}%`, height:10, background:'var(--up)', opacity:.7}}/>
                  </div>
                  <div style={{textAlign:'right'}} className={p.net>=0?'up':'down'}>{fSign(p.net,2)}M</div>
                </div>
              );
            })}
          </div>
        </div>
      </div>

      <div style={{padding:'24px 28px', display:'grid', gridTemplateColumns:'1fr 1fr 1fr 1fr', gap:18}}>
        {[
          {k:'MAX DRAWDOWN · YTD', v:'-4.21%', d:'Mar 14 → Mar 21', c:'var(--down)'},
          {k:'CORRELATION · SPX', v:'0.74',    d:'30d rolling',     c:'var(--ink)'},
          {k:'CONCENTRATION',     v:'42%',     d:'Top 3 positions', c:'var(--accent)'},
          {k:'LIQUIDITY · ADV',   v:'4.2%',    d:'Days to liquidate', c:'var(--ink)'},
        ].map((s,i)=>(
          <div key={i} style={{padding:'18px 22px', border:'1px solid var(--hair)'}}>
            <div className="label">{s.k}</div>
            <div className="midnum mono" style={{fontSize:36, marginTop:8, color:s.c}}>{s.v}</div>
            <div className="mono" style={{fontSize:10, color:'var(--muted)', marginTop:6}}>{s.d}</div>
          </div>
        ))}
      </div>
    </div>
  );
}

// =========================================================
// SETTINGS PAGE
// =========================================================
function SettingsPage({ tweaks, setTweak, setRoute }){
  const [tab, setTab] = nS('appearance');
  const tabs = [
    {k:'appearance', n:'Appearance'},
    {k:'data',       n:'Data & feeds'},
    {k:'orders',     n:'Order routing'},
    {k:'alerts',     n:'Alerts'},
    {k:'keys',       n:'Hotkeys'},
    {k:'account',    n:'Account & billing'},
  ];
  return (
    <div style={{flex:1, minHeight:0, overflow:'hidden', display:'grid', gridTemplateColumns:'240px 1fr', background:'var(--bg)'}} data-screen-label="08 Settings">
      <div style={{borderRight:'1px solid var(--rule)', padding:'18px 0', overflow:'auto'}}>
        <div style={{padding:'0 22px 14px'}}>
          <div className="label">Settings</div>
          <div style={{fontSize:18, fontWeight:600, marginTop:4}}>Workspace</div>
        </div>
        {tabs.map(t=>(
          <div key={t.k} onClick={()=>setTab(t.k)} className="mono" style={{
            padding:'12px 22px', fontSize:11, letterSpacing:'.1em', textTransform:'uppercase',
            cursor:'pointer',
            color: t.k===tab?'var(--ink)':'var(--muted)',
            background: t.k===tab?'var(--paper)':'transparent',
            borderLeft: t.k===tab?'3px solid var(--accent)':'3px solid transparent',
          }}>{t.n}</div>
        ))}
      </div>
      <div style={{overflow:'auto'}}>
        <div style={{padding:'24px 32px', borderBottom:'1px solid var(--rule)'}}>
          <div className="label">{tabs.find(t=>t.k===tab).n}</div>
          <div style={{fontSize:'clamp(28px,3.4vw,52px)', fontWeight:500, letterSpacing:'-0.03em', marginTop:6}}>
            {tab==='appearance'&&'Theme & layout.'}
            {tab==='data'&&'Market data feeds.'}
            {tab==='orders'&&'Order routing rules.'}
            {tab==='alerts'&&'Triggers & notifications.'}
            {tab==='keys'&&'Keyboard shortcuts.'}
            {tab==='account'&&'Account & billing.'}
          </div>
        </div>
        <div style={{padding:'28px 32px'}}>
          {tab==='appearance' && (
            <div style={{display:'grid', gap:24, maxWidth:720}}>
              <SettingRow label="Theme" desc="Bauhaus cream, or one of seven editor palettes.">
                <SegPick value={tweaks.theme} onChange={v=>setTweak('theme',v)} options={[
                  {v:'bauhaus',l:'Bauhaus'},
                  {v:'monokai',l:'Monokai'},
                  {v:'nord',l:'Nord'},
                  {v:'midnight',l:'Midnight'},
                  {v:'solarized',l:'Solarized'},
                  {v:'gruvbox',l:'Gruvbox'},
                  {v:'rosepine',l:'Rosé Pine'},
                  {v:'kanagawa',l:'Kanagawa'},
                ]}/>
              </SettingRow>
              <SettingRow label="Accent" desc="Used for the highlighter, key data, focus.">
                <SegPick value={tweaks.accent} onChange={v=>setTweak('accent',v)} options={[{v:'amber',l:'Amber'},{v:'cobalt',l:'Cobalt'},{v:'green',l:'Green'},{v:'black',l:'Mono'}]}/>
              </SettingRow>
              <SettingRow label="Density" desc="Row height across watchlists, books and tables.">
                <SegPick value={tweaks.density} onChange={v=>setTweak('density',v)} options={[{v:'compact',l:'Compact'},{v:'default',l:'Default'},{v:'comfy',l:'Comfy'}]}/>
              </SettingRow>
              <SettingRow label="Editorial callouts" desc="Show OHLC + indicator labels overlaid on the chart.">
                <Toggle value={tweaks.dotmatrix} onChange={v=>setTweak('dotmatrix',v)}/>
              </SettingRow>
              <SettingRow label="Default chart" desc="Indicators shown by default on every chart.">
                <div style={{display:'flex', gap:14, flexWrap:'wrap'}}>
                  <Check label="Volume"  value={tweaks.showVol} onChange={v=>setTweak('showVol',v)}/>
                  <Check label="Moving avgs"  value={tweaks.showMA} onChange={v=>setTweak('showMA',v)}/>
                  <Check label="Bollinger" value={tweaks.showBB} onChange={v=>setTweak('showBB',v)}/>
                </div>
              </SettingRow>
            </div>
          )}
          {tab==='data' && (
            <div style={{display:'grid', gap:0, maxWidth:780}}>
              {[
                {n:'NASDAQ TotalView',      l:'Subscribed', s:'$72/mo', c:'var(--up)'},
                {n:'NYSE OpenBook · Lvl 2', l:'Subscribed', s:'$56/mo', c:'var(--up)'},
                {n:'CBOE One',              l:'Subscribed', s:'$24/mo', c:'var(--up)'},
                {n:'OPRA · Options',        l:'Subscribed', s:'$95/mo', c:'var(--up)'},
                {n:'CME · Futures',         l:'Trial',      s:'14 days', c:'var(--accent)'},
                {n:'ICE · Bonds',           l:'Off',        s:'$48/mo', c:'var(--muted)'},
              ].map((d,i)=>(
                <div key={i} style={{display:'grid', gridTemplateColumns:'1fr 120px 100px 80px', alignItems:'center', padding:'14px 0', borderBottom:'1px solid var(--hair)'}}>
                  <div>
                    <div style={{fontSize:14, fontWeight:500}}>{d.n}</div>
                    <div className="mono" style={{fontSize:10, color:'var(--muted)', marginTop:2}}>Latency 4ms · Region NY-4</div>
                  </div>
                  <div className="mono" style={{fontSize:11, color:d.c, letterSpacing:'.08em'}}>{d.l.toUpperCase()}</div>
                  <div className="mono" style={{fontSize:11, color:'var(--muted)'}}>{d.s}</div>
                  <div className="mono" style={{fontSize:10, padding:'6px 10px', border:'1px solid var(--rule)', textAlign:'center', cursor:'pointer'}}>MANAGE</div>
                </div>
              ))}
            </div>
          )}
          {tab==='orders' && (
            <div style={{display:'grid', gap:24, maxWidth:720}}>
              <SettingRow label="Default route" desc="SOR will fall back if direct route rejects.">
                <SegPick value="SOR" onChange={()=>{}} options={[{v:'SOR',l:'Smart'},{v:'NSDQ',l:'NASDAQ'},{v:'ARCA',l:'ARCA'},{v:'IEX',l:'IEX'}]}/>
              </SettingRow>
              <SettingRow label="Default order type" desc="Used when ticket opens fresh.">
                <SegPick value="LMT" onChange={()=>{}} options={[{v:'MKT',l:'Market'},{v:'LMT',l:'Limit'},{v:'STP',l:'Stop'},{v:'STL',l:'Stop-Limit'}]}/>
              </SettingRow>
              <SettingRow label="Default TIF" desc="Time in force for new orders.">
                <SegPick value="DAY" onChange={()=>{}} options={[{v:'DAY',l:'Day'},{v:'GTC',l:'GTC'},{v:'IOC',l:'IOC'},{v:'FOK',l:'FOK'}]}/>
              </SettingRow>
              <SettingRow label="Confirm before submit" desc="Show review modal for any non-IOC order.">
                <Toggle value={true} onChange={()=>{}}/>
              </SettingRow>
              <SettingRow label="Auto-stop on fill" desc="Trigger stop-loss order automatically after entry fills.">
                <Toggle value={false} onChange={()=>{}}/>
              </SettingRow>
            </div>
          )}
          {tab==='alerts' && (
            <div style={{maxWidth:720}}>
              <div className="panel-head" style={{paddingLeft:0, paddingRight:0}}>
                <div className="ttl"><span className="num">AL</span><span className="nm">Active alerts</span></div>
                <div className="mono" style={{fontSize:10, color:'var(--accent)'}}>+ NEW</div>
              </div>
              {[
                {n:'NVDA crosses $1,300', t:'price > 1,300', s:'ON'},
                {n:'TSLA stop-loss', t:'price < 250', s:'ARMED'},
                {n:'Daily P&L > +$250K', t:'pnl > 250000', s:'ON'},
                {n:'VIX < 15', t:'vix < 15.0', s:'ON'},
                {n:'CPI release', t:'event @ 08:30', s:'ON'},
              ].map((a,i)=>(
                <div key={i} style={{display:'grid', gridTemplateColumns:'1fr 240px 80px 60px', alignItems:'center', padding:'14px 0', borderBottom:'1px solid var(--hair)'}}>
                  <div style={{fontSize:13}}>{a.n}</div>
                  <div className="mono" style={{fontSize:11, color:'var(--muted)'}}>{a.t}</div>
                  <div className="mono" style={{fontSize:10, padding:'4px 8px', background:'var(--ink)', color:'var(--bg)', textAlign:'center', letterSpacing:'.1em'}}>{a.s}</div>
                  <div style={{textAlign:'right', color:'var(--muted)', cursor:'pointer'}}>···</div>
                </div>
              ))}
            </div>
          )}
          {tab==='keys' && (
            <div style={{display:'grid', gridTemplateColumns:'1fr 1fr', gap:32, maxWidth:780}}>
              {[
                ['Open palette',         '⌘ K'],
                ['Quick search',         '/'],
                ['Buy ticket',           'B'],
                ['Sell ticket',          'S'],
                ['Cancel orders',        '⌘ ⇧ X'],
                ['Flatten symbol',       '⌘ ⇧ F'],
                ['Flatten all',          '⌘ ⌥ F'],
                ['Toggle dotmatrix',     'D'],
                ['Cycle timeframe',      'T'],
                ['Toggle theme',         '⌘ J'],
                ['Toggle MA',            'M'],
                ['Toggle Bollinger',     'V'],
              ].map(([n,k])=>(
                <div key={n} style={{display:'flex', justifyContent:'space-between', alignItems:'center', padding:'10px 0', borderBottom:'1px dashed var(--hair)'}}>
                  <div style={{fontSize:13}}>{n}</div>
                  <div className="mono" style={{fontSize:11, padding:'4px 8px', border:'1px solid var(--hair)', background:'var(--paper)'}}>{k}</div>
                </div>
              ))}
            </div>
          )}
          {tab==='account' && (
            <div style={{display:'grid', gap:24, maxWidth:720}}>
              <div style={{display:'grid', gridTemplateColumns:'1fr 1fr', gap:18}}>
                {[
                  {k:'Plan',     v:'Meridian Pro',  s:'Renews 2026-12-01'},
                  {k:'Seat',     v:'1 of 1',        s:'Add seat $48/mo'},
                  {k:'Data fees',v:'$247/mo',       s:'Pass-through'},
                  {k:'Storage',  v:'4.2GB / 50GB',  s:'Replays + workspaces'},
                ].map((s,i)=>(
                  <div key={i} style={{padding:'18px 22px', border:'1px solid var(--hair)'}}>
                    <div className="label">{s.k}</div>
                    <div className="midnum mono" style={{fontSize:32, marginTop:6}}>{s.v}</div>
                    <div className="mono" style={{fontSize:10, color:'var(--muted)', marginTop:6}}>{s.s}</div>
                  </div>
                ))}
              </div>
              <SettingRow label="Email" desc="Notifications, statements and trade confirms.">
                <input className="mono" defaultValue="j.morgan@meridian.co" style={{padding:'10px 12px', fontSize:13, border:'1px solid var(--rule)', background:'var(--paper)', color:'var(--ink)', width:280, fontFamily:'JetBrains Mono'}}/>
              </SettingRow>
              <SettingRow label="Sign out" desc="Ends this terminal session.">
                <div className="mono" style={{padding:'10px 18px', background:'var(--ink)', color:'var(--bg)', fontSize:11, letterSpacing:'.12em', cursor:'pointer'}}>SIGN OUT</div>
              </SettingRow>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function SettingRow({label, desc, children}){
  return (
    <div style={{display:'grid', gridTemplateColumns:'260px 1fr', gap:24, paddingBottom:18, borderBottom:'1px solid var(--hair)'}}>
      <div>
        <div style={{fontSize:14, fontWeight:500}}>{label}</div>
        <div style={{fontSize:11, color:'var(--muted)', marginTop:4, lineHeight:1.4}}>{desc}</div>
      </div>
      <div>{children}</div>
    </div>
  );
}
function SegPick({value, options, onChange}){
  return (
    <div style={{display:'inline-flex', border:'1px solid var(--rule)'}}>
      {options.map((o,i)=>(
        <div key={o.v} className="mono" onClick={()=>onChange(o.v)} style={{
          padding:'8px 14px', fontSize:11, letterSpacing:'.1em', textTransform:'uppercase', cursor:'pointer',
          background: o.v===value?'var(--ink)':'transparent',
          color: o.v===value?'var(--bg)':'var(--muted)',
          borderRight: i<options.length-1?'1px solid var(--rule)':0,
        }}>{o.l}</div>
      ))}
    </div>
  );
}
function Toggle({value, onChange}){
  return (
    <div onClick={()=>onChange(!value)} style={{
      width:48, height:24, borderRadius:0, background: value?'var(--ink)':'var(--paper)', border:'1px solid var(--rule)',
      position:'relative', cursor:'pointer'
    }}>
      <div style={{
        position:'absolute', top:1, left: value?23:1, width:20, height:20,
        background: value?'var(--bg)':'var(--ink)', transition:'left .15s'
      }}/>
    </div>
  );
}
function Check({label, value, onChange}){
  return (
    <div onClick={()=>onChange(!value)} style={{display:'flex', gap:8, alignItems:'center', cursor:'pointer'}}>
      <div style={{width:16, height:16, border:'1px solid var(--rule)', background: value?'var(--ink)':'transparent', display:'grid', placeItems:'center', color:'var(--bg)', fontSize:10}}>{value?'✓':''}</div>
      <span style={{fontSize:12}}>{label}</span>
    </div>
  );
}

// =========================================================
// OPTIONS CHAIN PAGE
// =========================================================
function OptionsPage({ activeRow, D }){
  const [exp, setExp] = nS('May 15');
  const [strikes, setStrikes] = nS('ATM');
  const expiries = ['Apr 26','May 02','May 09','May 15','May 30','Jun 20','Jul 18','Sep 19','Dec 19','Jan 16 \'27'];
  return (
    <div style={{flex:1, minHeight:0, overflow:'auto', background:'var(--bg)'}} data-screen-label="09 Options">
      <div style={{display:'grid', gridTemplateColumns:'1.4fr 1fr 1fr 1fr', borderBottom:'1px solid var(--rule)'}}>
        <div style={{padding:'24px 28px', borderRight:'1px solid var(--rule)'}}>
          <div className="label">Underlying</div>
          <div style={{display:'flex', alignItems:'baseline', gap:14, marginTop:4}}>
            <div className="mono" style={{fontSize:24, fontWeight:600}}>{activeRow.sym}</div>
            <div style={{color:'var(--muted)', fontSize:13}}>{activeRow.name}</div>
          </div>
          <div className="bignum mono" style={{fontSize:'clamp(48px,5.6vw,96px)', marginTop:8}}>{fNum(activeRow.last,2)}</div>
          <div className={'mono '+(activeRow.chg>=0?'up':'down')} style={{fontSize:16, marginTop:4}}>{fSign(activeRow.chg,2)} · {fSign(activeRow.pct)}%</div>
        </div>
        {[
          {k:'IV (30d)',  v:'42.8%', s:'+1.4 vs prev'},
          {k:'IV RANK',   v:'68',    s:'52w'},
          {k:'PUT/CALL',  v:'0.74',  s:'OI ratio'},
        ].map((s,i)=>(
          <div key={i} style={{padding:'24px 28px', borderRight:i<2?'1px solid var(--rule)':0}}>
            <div className="label">{s.k}</div>
            <div className="bignum mono" style={{fontSize:'clamp(40px,4.4vw,72px)', marginTop:8}}>{s.v}</div>
            <div className="mono" style={{fontSize:11, color:'var(--muted)', marginTop:6}}>{s.s}</div>
          </div>
        ))}
      </div>

      {/* Toolbar */}
      <div style={{display:'flex', alignItems:'center', borderBottom:'1px solid var(--rule)', padding:'10px 22px', gap:18, flexWrap:'wrap'}}>
        <div>
          <div className="label" style={{marginBottom:4}}>Expiry</div>
          <div style={{display:'flex', overflow:'auto', gap:0, border:'1px solid var(--hair)'}}>
            {expiries.map(e=>(
              <div key={e} onClick={()=>setExp(e)} className="mono" style={{
                padding:'6px 12px', fontSize:11, letterSpacing:'.06em', cursor:'pointer',
                background: e===exp?'var(--ink)':'transparent',
                color: e===exp?'var(--bg)':'var(--muted)',
                borderRight:'1px solid var(--hair)', whiteSpace:'nowrap'
              }}>{e}</div>
            ))}
          </div>
        </div>
        <div>
          <div className="label" style={{marginBottom:4}}>Range</div>
          <SegPick value={strikes} onChange={setStrikes} options={[{v:'ATM',l:'ATM ±5'},{v:'NEAR',l:'±10'},{v:'ALL',l:'All'}]}/>
        </div>
        <div>
          <div className="label" style={{marginBottom:4}}>View</div>
          <SegPick value="GREEKS" onChange={()=>{}} options={[{v:'PRICE',l:'Price'},{v:'GREEKS',l:'Greeks'},{v:'OI',l:'OI'},{v:'VOL',l:'Vol'}]}/>
        </div>
        <div style={{marginLeft:'auto', display:'flex', gap:8, alignItems:'center'}}>
          <span className="label">Strategy</span>
          <Drop value="Vertical" options={['Single','Vertical','Iron Condor','Butterfly','Calendar','Strangle']}/>
        </div>
      </div>

      {/* Chain table */}
      <div style={{padding:0, borderBottom:'1px solid var(--rule)'}}>
        <FullChain rows={D.optsChain} spot={activeRow.last}/>
      </div>

      {/* IV smile + skew */}
      <div style={{display:'grid', gridTemplateColumns:'1fr 1fr', borderBottom:'1px solid var(--rule)'}}>
        <div style={{borderRight:'1px solid var(--rule)', padding:'18px 22px'}}>
          <div className="label">IV smile · {exp}</div>
          <div style={{fontSize:16, fontWeight:600, marginBottom:14, marginTop:4}}>Implied vol by strike</div>
          <IVSmile rows={D.optsChain} spot={activeRow.last}/>
        </div>
        <div style={{padding:'18px 22px'}}>
          <div className="label">Term structure</div>
          <div style={{fontSize:16, fontWeight:600, marginBottom:14, marginTop:4}}>ATM IV across expiries</div>
          <TermStruct/>
        </div>
      </div>
    </div>
  );
}

function Drop({value, options}){
  const [open,setOpen]=nS(false);
  return (
    <div style={{position:'relative'}}>
      <div onClick={()=>setOpen(o=>!o)} className="mono" style={{padding:'6px 12px', border:'1px solid var(--rule)', background:'var(--paper)', fontSize:11, cursor:'pointer', display:'flex', gap:8, alignItems:'center'}}>
        {value} <span style={{color:'var(--muted)'}}>▾</span>
      </div>
      {open && (
        <div style={{position:'absolute', top:'100%', right:0, marginTop:2, background:'var(--bg)', border:'1px solid var(--rule)', minWidth:160, zIndex:50}}>
          {options.map(o=>(
            <div key={o} onClick={()=>setOpen(false)} className="mono" style={{padding:'8px 12px', fontSize:11, cursor:'pointer', borderBottom:'1px solid var(--hair-2)', background:o===value?'var(--paper)':'transparent'}}>{o}</div>
          ))}
        </div>
      )}
    </div>
  );
}

function FullChain({rows, spot}){
  return (
    <div style={{fontFamily:'JetBrains Mono', fontSize:11}}>
      <div style={{display:'grid', gridTemplateColumns:'1fr 1fr 1fr 1fr 1fr 1fr 1fr 90px 1fr 1fr 1fr 1fr 1fr 1fr 1fr', borderBottom:'1px solid var(--rule)'}}>
        {['IV','Δ','OI','VOL','LAST','BID','ASK','STRIKE','BID','ASK','LAST','VOL','OI','Δ','IV'].map((h,i)=>(
          <div key={i} style={{padding:'8px 6px', fontSize:9, letterSpacing:'.08em', color:'var(--muted)', textAlign: i===7?'center':'right', background:'var(--paper)'}}>{h}</div>
        ))}
      </div>
      <div style={{padding:'4px 0', textAlign:'center', fontSize:9, letterSpacing:'.12em', color:'var(--muted)', borderBottom:'1px solid var(--hair)'}}>
        ◀  CALLS · in-the-money shaded ·  PUTS  ▶
      </div>
      {rows.map(r=>{
        const itmC = r.k < spot, itmP = r.k > spot;
        const cLast = ((parseFloat(r.cBid)+parseFloat(r.cAsk))/2).toFixed(2);
        const pLast = ((parseFloat(r.pBid)+parseFloat(r.pAsk))/2).toFixed(2);
        return (
          <div key={r.k} style={{display:'grid', gridTemplateColumns:'1fr 1fr 1fr 1fr 1fr 1fr 1fr 90px 1fr 1fr 1fr 1fr 1fr 1fr 1fr', borderBottom:'1px solid var(--hair-2)'}}>
            <div style={{padding:'6px 6px', textAlign:'right', background: itmC?'color-mix(in srgb, var(--up) 8%, transparent)':'transparent'}}>{r.iv}</div>
            <div style={{padding:'6px 6px', textAlign:'right', background: itmC?'color-mix(in srgb, var(--up) 8%, transparent)':'transparent', color:'var(--muted)'}}>{(0.5+ (r.k<spot? 0.2 : -0.2)).toFixed(2)}</div>
            <div style={{padding:'6px 6px', textAlign:'right', color:'var(--muted)', background: itmC?'color-mix(in srgb, var(--up) 8%, transparent)':'transparent'}}>{r.cOI.toLocaleString()}</div>
            <div style={{padding:'6px 6px', textAlign:'right', color:'var(--muted)', background: itmC?'color-mix(in srgb, var(--up) 8%, transparent)':'transparent'}}>{r.cVol.toLocaleString()}</div>
            <div style={{padding:'6px 6px', textAlign:'right', background: itmC?'color-mix(in srgb, var(--up) 12%, transparent)':'transparent'}}>{cLast}</div>
            <div className="up" style={{padding:'6px 6px', textAlign:'right', background: itmC?'color-mix(in srgb, var(--up) 16%, transparent)':'transparent', fontWeight:600}}>{r.cBid}</div>
            <div style={{padding:'6px 6px', textAlign:'right', background: itmC?'color-mix(in srgb, var(--up) 16%, transparent)':'transparent'}}>{r.cAsk}</div>
            <div style={{padding:'6px 6px', textAlign:'center', fontWeight:600, background:'var(--paper)', borderInline:'1px solid var(--hair)'}}>{r.k}</div>
            <div className="down" style={{padding:'6px 6px', textAlign:'right', background: itmP?'color-mix(in srgb, var(--down) 16%, transparent)':'transparent', fontWeight:600}}>{r.pBid}</div>
            <div style={{padding:'6px 6px', textAlign:'right', background: itmP?'color-mix(in srgb, var(--down) 16%, transparent)':'transparent'}}>{r.pAsk}</div>
            <div style={{padding:'6px 6px', textAlign:'right', background: itmP?'color-mix(in srgb, var(--down) 12%, transparent)':'transparent'}}>{pLast}</div>
            <div style={{padding:'6px 6px', textAlign:'right', color:'var(--muted)', background: itmP?'color-mix(in srgb, var(--down) 8%, transparent)':'transparent'}}>{r.pVol.toLocaleString()}</div>
            <div style={{padding:'6px 6px', textAlign:'right', color:'var(--muted)', background: itmP?'color-mix(in srgb, var(--down) 8%, transparent)':'transparent'}}>{r.pOI.toLocaleString()}</div>
            <div style={{padding:'6px 6px', textAlign:'right', color:'var(--muted)', background: itmP?'color-mix(in srgb, var(--down) 8%, transparent)':'transparent'}}>{(0.5- (r.k<spot? 0.2 : -0.2)).toFixed(2)}</div>
            <div style={{padding:'6px 6px', textAlign:'right', background: itmP?'color-mix(in srgb, var(--down) 8%, transparent)':'transparent'}}>{r.iv}</div>
          </div>
        );
      })}
    </div>
  );
}

function IVSmile({rows, spot}){
  const W=540, H=200, pad=22;
  const ivs = rows.map(r=>parseFloat(r.iv));
  const min=Math.min(...ivs)-1, max=Math.max(...ivs)+1;
  const x=i=> pad + i/(rows.length-1) * (W-pad*2);
  const y=v=> H-pad - (v-min)/(max-min) * (H-pad*2);
  const path = rows.map((r,i)=>(i?'L':'M')+x(i).toFixed(1)+' '+y(parseFloat(r.iv)).toFixed(1)).join(' ');
  return (
    <svg viewBox={`0 0 ${W} ${H}`} style={{width:'100%', height:200, display:'block'}}>
      {[0,1,2,3,4].map(i=><line key={i} x1={pad} x2={W-pad} y1={pad+(H-pad*2)*i/4} y2={pad+(H-pad*2)*i/4} stroke="var(--hair-2)"/>)}
      <path d={path} fill="none" stroke="var(--accent)" strokeWidth="1.6"/>
      {rows.map((r,i)=>{
        const cx=x(i), cy=y(parseFloat(r.iv));
        const atm = Math.abs(r.k-spot)<6;
        return <circle key={i} cx={cx} cy={cy} r={atm?4:2.4} fill={atm?'var(--accent)':'var(--ink)'}/>;
      })}
      {rows.map((r,i)=>(
        <text key={i} x={x(i)} y={H-4} fontSize="9" fontFamily="JetBrains Mono" textAnchor="middle" fill="var(--muted)">{r.k}</text>
      ))}
    </svg>
  );
}

function TermStruct(){
  const data = [
    {t:'1W', v:36.4},{t:'2W', v:38.2},{t:'1M', v:42.8},{t:'2M', v:44.1},{t:'3M', v:43.4},{t:'6M', v:41.8},{t:'9M', v:40.9},{t:'1Y', v:39.4},{t:'2Y', v:38.1},
  ];
  const W=540, H=200, pad=22;
  const min=34, max=46;
  const x=i=> pad + i/(data.length-1) * (W-pad*2);
  const y=v=> H-pad - (v-min)/(max-min) * (H-pad*2);
  return (
    <svg viewBox={`0 0 ${W} ${H}`} style={{width:'100%', height:200, display:'block'}}>
      {[0,1,2,3,4].map(i=><line key={i} x1={pad} x2={W-pad} y1={pad+(H-pad*2)*i/4} y2={pad+(H-pad*2)*i/4} stroke="var(--hair-2)"/>)}
      {data.map((d,i)=>(
        <rect key={i} x={x(i)-12} y={y(d.v)} width="24" height={H-pad-y(d.v)} fill="var(--ink)"/>
      ))}
      {data.map((d,i)=>(
        <React.Fragment key={i}>
          <text x={x(i)} y={y(d.v)-6} fontSize="10" fontFamily="JetBrains Mono" textAnchor="middle" fill="var(--ink)">{d.v}</text>
          <text x={x(i)} y={H-4} fontSize="9" fontFamily="JetBrains Mono" textAnchor="middle" fill="var(--muted)">{d.t}</text>
        </React.Fragment>
      ))}
    </svg>
  );
}

// =========================================================
// ORDER TICKET MODAL
// =========================================================
function OrderTicketModal({ open, onClose, sym='NVDA', last=1284.32 }){
  const [side,setSide] = nS('buy');
  const [type,setType] = nS('LMT');
  const [tif,setTif]   = nS('DAY');
  const [route,setRoute] = nS('SOR');
  const [qty,setQty]   = nS(100);
  const [px,setPx]     = nS(last.toFixed(2));
  const [stop,setStop] = nS((last*0.98).toFixed(2));
  const [tp,setTp]     = nS((last*1.05).toFixed(2));
  if (!open) return null;
  const notional = qty * parseFloat(px||0);
  return (
    <div onClick={onClose} style={{
      position:'fixed', inset:0, zIndex:200, background:'rgba(20,20,15,.5)',
      display:'grid', placeItems:'center', backdropFilter:'blur(2px)',
    }}>
      <div onClick={e=>e.stopPropagation()} style={{
        width:'min(880px, 92vw)', background:'var(--bg)', border:'1px solid var(--rule)',
        boxShadow:'0 24px 80px rgba(0,0,0,.3)', display:'grid', gridTemplateColumns:'1fr 320px', maxHeight:'88vh', overflow:'hidden'
      }}>
        <div style={{padding:'24px 28px', display:'flex', flexDirection:'column', gap:18, overflow:'auto'}}>
          <div style={{display:'flex', justifyContent:'space-between', alignItems:'baseline'}}>
            <div>
              <div className="label">Order ticket</div>
              <div style={{fontSize:30, fontWeight:600, letterSpacing:'-0.02em', marginTop:2}}>{sym} <span style={{color:'var(--muted)', fontWeight:400, fontSize:14, marginLeft:8}}>· NASDAQ · USD</span></div>
            </div>
            <div onClick={onClose} className="mono" style={{padding:'4px 8px', fontSize:10, color:'var(--muted)', cursor:'pointer'}}>ESC</div>
          </div>

          <div style={{display:'grid', gridTemplateColumns:'1fr 1fr', border:'1px solid var(--rule)'}}>
            <div onClick={()=>setSide('buy')} className="mono" style={{padding:14, textAlign:'center', fontSize:13, letterSpacing:'.14em', cursor:'pointer',
              background: side==='buy'?'var(--up)':'transparent',
              color: side==='buy'?'var(--bg)':'var(--muted)',
              borderRight:'1px solid var(--rule)', fontWeight:600}}>BUY · LONG</div>
            <div onClick={()=>setSide('sell')} className="mono" style={{padding:14, textAlign:'center', fontSize:13, letterSpacing:'.14em', cursor:'pointer',
              background: side==='sell'?'var(--down)':'transparent',
              color: side==='sell'?'var(--bg)':'var(--muted)', fontWeight:600}}>SELL · SHORT</div>
          </div>

          <div style={{display:'grid', gridTemplateColumns:'1fr 1fr', gap:14}}>
            <div className="field">
              <label>Type</label>
              <FieldSelect value={type} onChange={setType} options={['MKT','LMT','STP','STP-LMT','TRAIL','MOC','OCO','BRACKET']}/>
            </div>
            <div className="field">
              <label>TIF</label>
              <FieldSelect value={tif} onChange={setTif} options={['DAY','GTC','IOC','FOK','GTD']}/>
            </div>
          </div>

          <div className="field">
            <label>Quantity</label>
            <div className="input">
              <input value={qty} onChange={e=>setQty(parseInt(e.target.value)||0)} className="mono" style={{flex:1, border:0, outline:0, background:'transparent', fontSize:18, color:'var(--ink)'}}/>
              <div className="stepper">
                {[100,500,1000].map(s=>(
                  <span key={s} onClick={()=>setQty(s)} style={{padding:'4px 10px', fontFamily:'JetBrains Mono', fontSize:10, color:'var(--muted)'}}>{s}</span>
                ))}
                <span onClick={()=>setQty(Math.max(0,qty-100))}>−</span>
                <span onClick={()=>setQty(qty+100)}>+</span>
              </div>
            </div>
          </div>

          {(type==='LMT' || type==='STP-LMT' || type==='BRACKET') && (
            <div className="field">
              <label>Limit price</label>
              <div className="input">
                <input value={px} onChange={e=>setPx(e.target.value)} className="mono" style={{flex:1, border:0, outline:0, background:'transparent', fontSize:18, color:'var(--ink)'}}/>
                <div className="stepper">
                  <span onClick={()=>setPx((parseFloat(px)-0.05).toFixed(2))}>−</span>
                  <span onClick={()=>setPx((parseFloat(px)+0.05).toFixed(2))}>+</span>
                </div>
              </div>
            </div>
          )}

          {type==='BRACKET' && (
            <div style={{display:'grid', gridTemplateColumns:'1fr 1fr', gap:14}}>
              <div className="field">
                <label>Stop loss</label>
                <div className="input">
                  <input value={stop} onChange={e=>setStop(e.target.value)} className="mono" style={{flex:1, border:0, outline:0, background:'transparent', fontSize:16, color:'var(--down)'}}/>
                </div>
              </div>
              <div className="field">
                <label>Take profit</label>
                <div className="input">
                  <input value={tp} onChange={e=>setTp(e.target.value)} className="mono" style={{flex:1, border:0, outline:0, background:'transparent', fontSize:16, color:'var(--up)'}}/>
                </div>
              </div>
            </div>
          )}

          <div className="field">
            <label>Route</label>
            <FieldSelect value={route} onChange={setRoute} options={['SOR · Smart','NSDQ','NYSE ARCA','IEX','BATS','EDGX','LIT only','Dark only']}/>
          </div>

          <div style={{display:'flex', gap:10}}>
            {[
              ['POSTONLY','Maker only'],
              ['ALLORNONE','AON'],
              ['DISCRET','Discret. ±0.02'],
              ['REDUCEONLY','Reduce only'],
            ].map(([k,l])=>(
              <div key={k} className="mono" style={{padding:'8px 12px', border:'1px solid var(--hair)', fontSize:10, letterSpacing:'.08em', color:'var(--muted)', cursor:'pointer'}}>{l}</div>
            ))}
          </div>
        </div>

        <div style={{borderLeft:'1px solid var(--rule)', background:'var(--paper)', padding:'24px 22px', display:'flex', flexDirection:'column', gap:18}}>
          <div>
            <div className="label">Last trade</div>
            <div className="bignum mono" style={{fontSize:48, marginTop:4}}>{last.toFixed(2)}</div>
            <div className="mono up" style={{fontSize:13, marginTop:2}}>+28.40 · +2.26%</div>
          </div>
          <div style={{display:'grid', gridTemplateColumns:'1fr 1fr', gap:8, fontFamily:'JetBrains Mono', fontSize:11}}>
            <div><span style={{color:'var(--muted)'}}>BID</span> <span className="up">{(last-0.05).toFixed(2)}</span></div>
            <div><span style={{color:'var(--muted)'}}>ASK</span> <span className="down">{(last+0.05).toFixed(2)}</span></div>
            <div><span style={{color:'var(--muted)'}}>SPRD</span> 0.05</div>
            <div><span style={{color:'var(--muted)'}}>BIDSZ</span> 124</div>
          </div>
          <div className="summary" style={{borderTop:'1px solid var(--hair)', paddingTop:14, gap:8}}>
            <div className="k">Notional</div><div className="mono" style={{fontWeight:600}}>${fNum(notional)}</div>
            <div className="k">Buying power</div><div className="mono">$5.83M</div>
            <div className="k">Margin req.</div><div className="mono">${fNum(notional*0.5)}</div>
            <div className="k">Commission</div><div className="mono">$0.00</div>
            <div className="k">Slippage est.</div><div className="mono" style={{color:'var(--muted)'}}>0.8 bp</div>
            <div className="k">Daily VaR Δ</div><div className="mono down">+$3,184</div>
          </div>
          <button className={`btn-submit ${side}`} style={{padding:'18px', fontSize:13}}>
            {side==='buy'?'REVIEW BUY':'REVIEW SELL'} · {qty} @ {type==='MKT'?'MKT':px}
          </button>
          <div className="mono" style={{fontSize:10, color:'var(--muted)', textAlign:'center', letterSpacing:'.08em'}}>
            REVIEW · CONFIRM ENABLED
          </div>
        </div>
      </div>
    </div>
  );
}
function FieldSelect({value, options, onChange}){
  const [open,setOpen]=nS(false);
  return (
    <div style={{position:'relative'}}>
      <div className="input" onClick={()=>setOpen(o=>!o)} style={{cursor:'pointer'}}>
        <span>{value}</span>
        <span style={{color:'var(--muted)', fontSize:11}}>▾</span>
      </div>
      {open && (
        <div style={{position:'absolute', top:'calc(100% + 2px)', left:0, right:0, background:'var(--bg)', border:'1px solid var(--rule)', maxHeight:240, overflow:'auto', zIndex:60, boxShadow:'0 8px 24px rgba(0,0,0,.18)'}}>
          {options.map(o=>(
            <div key={o} onClick={()=>{onChange(o); setOpen(false);}} className="mono" style={{
              padding:'10px 14px', fontSize:12, cursor:'pointer', borderBottom:'1px solid var(--hair-2)',
              background:o===value?'var(--paper)':'transparent'
            }}>{o}</div>
          ))}
        </div>
      )}
    </div>
  );
}

// =========================================================
// TOPBAR DROPDOWNS — Account / Alerts / Profile
// =========================================================
function AccountDropdown(){
  const [open,setOpen]=nS(false);
  const A = window.TERMINAL_DATA_EXTRA;
  const cur = A.accounts[0];
  return (
    <div style={{position:'relative', borderLeft:'1px solid var(--rule)'}}>
      <div onClick={()=>setOpen(o=>!o)} style={{padding:'10px 16px', cursor:'pointer', display:'flex', flexDirection:'column', minWidth:160}}>
        <span className="mono" style={{fontSize:9, color:'var(--muted)', letterSpacing:'.12em'}}>ACCOUNT ▾</span>
        <span className="mono" style={{fontSize:12, fontWeight:600, marginTop:2}}>{cur.id}</span>
        <span className="mono" style={{fontSize:10, color:'var(--muted)', marginTop:1}}>{cur.value} · <span className="up">{cur.flat}</span></span>
      </div>
      {open && (
        <div style={{position:'absolute', top:'100%', right:0, background:'var(--bg)', border:'1px solid var(--rule)', minWidth:280, zIndex:80, boxShadow:'0 12px 40px rgba(0,0,0,.24)'}}>
          <div style={{padding:'10px 14px', borderBottom:'1px solid var(--hair)'}} className="mono">
            <div className="label">Switch account</div>
          </div>
          {A.accounts.map(a=>(
            <div key={a.id} onClick={()=>setOpen(false)} style={{padding:'12px 14px', borderBottom:'1px solid var(--hair-2)', cursor:'pointer', display:'grid', gridTemplateColumns:'1fr auto', gap:10}}>
              <div>
                <div className="mono" style={{fontSize:12, fontWeight:600}}>{a.id}</div>
                <div className="mono" style={{fontSize:10, color:'var(--muted)', marginTop:2}}>{a.name}</div>
              </div>
              <div style={{textAlign:'right'}}>
                <div className="mono" style={{fontSize:12, fontWeight:600}}>{a.value}</div>
                <div className="mono up" style={{fontSize:10, marginTop:2}}>{a.flat}</div>
              </div>
            </div>
          ))}
          <div className="mono" style={{padding:'10px 14px', fontSize:10, color:'var(--accent)', letterSpacing:'.1em', cursor:'pointer'}}>+ ADD ACCOUNT</div>
        </div>
      )}
    </div>
  );
}

function AlertsDropdown(){
  const [open,setOpen]=nS(false);
  const A = window.TERMINAL_DATA_EXTRA;
  return (
    <div style={{position:'relative', borderLeft:'1px solid var(--rule)'}}>
      <div onClick={()=>setOpen(o=>!o)} style={{padding:'12px 14px', cursor:'pointer', display:'flex', alignItems:'center', gap:8, height:'100%'}}>
        <div style={{position:'relative', width:18, height:18, display:'grid', placeItems:'center'}}>
          <span style={{fontSize:14}}>◐</span>
          <span style={{position:'absolute', top:-2, right:-4, width:14, height:14, borderRadius:'50%', background:'var(--accent)', color:'#fff', fontSize:8, display:'grid', placeItems:'center', fontFamily:'JetBrains Mono', fontWeight:600}}>{A.alerts.length}</span>
        </div>
      </div>
      {open && (
        <div style={{position:'absolute', top:'100%', right:0, background:'var(--bg)', border:'1px solid var(--rule)', width:360, zIndex:80, boxShadow:'0 12px 40px rgba(0,0,0,.24)'}}>
          <div style={{padding:'12px 14px', borderBottom:'1px solid var(--hair)', display:'flex', justifyContent:'space-between'}}>
            <span className="mono label">ALERTS · {A.alerts.length} NEW</span>
            <span className="mono" style={{fontSize:10, color:'var(--accent)', cursor:'pointer'}}>MARK ALL READ</span>
          </div>
          {A.alerts.map((a,i)=>(
            <div key={i} style={{padding:'12px 14px', borderBottom:'1px solid var(--hair-2)', display:'grid', gridTemplateColumns:'auto 1fr auto', gap:10, alignItems:'baseline', borderLeft: a.tone==='pos'?'2px solid var(--up)':a.tone==='neg'?'2px solid var(--down)':'2px solid var(--hair)'}}>
              <span className="mono" style={{fontSize:9, color:'var(--muted)', letterSpacing:'.1em'}}>{a.t}</span>
              <span style={{fontSize:12, lineHeight:1.35}}><span className="mono" style={{fontWeight:600}}>{a.sym}</span> · {a.msg}</span>
              <span style={{fontSize:10, color:'var(--muted)'}}>···</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function ProfileDropdown({onNavSettings}){
  const [open,setOpen]=nS(false);
  return (
    <div style={{position:'relative', borderLeft:'1px solid var(--rule)'}}>
      <div onClick={()=>setOpen(o=>!o)} style={{padding:'10px 14px', cursor:'pointer', display:'flex', alignItems:'center', gap:10}}>
        <div style={{width:28, height:28, background:'var(--ink)', color:'var(--bg)', display:'grid', placeItems:'center', fontFamily:'JetBrains Mono', fontWeight:600, fontSize:11}}>JM</div>
        <span className="mono" style={{fontSize:11, color:'var(--muted)'}}>▾</span>
      </div>
      {open && (
        <div style={{position:'absolute', top:'100%', right:0, background:'var(--bg)', border:'1px solid var(--rule)', minWidth:240, zIndex:80, boxShadow:'0 12px 40px rgba(0,0,0,.24)'}}>
          <div style={{padding:'14px', borderBottom:'1px solid var(--hair)'}}>
            <div style={{fontSize:13, fontWeight:600}}>Jordan Morgan</div>
            <div className="mono" style={{fontSize:10, color:'var(--muted)', marginTop:2}}>j.morgan@meridian.co</div>
            <div className="mono" style={{fontSize:9, color:'var(--accent)', marginTop:6, letterSpacing:'.12em'}}>MERIDIAN PRO</div>
          </div>
          {[
            {l:'Settings',          a: onNavSettings, icon:'◐'},
            {l:'Workspaces',        icon:'⊞'},
            {l:'Trade history',     icon:'≣'},
            {l:'Statements',        icon:'▤'},
            {l:'Help & docs',       icon:'?'},
            {l:'Sign out',          icon:'⎋', danger:true},
          ].map(it=>(
            <div key={it.l} onClick={()=>{ if(it.a) it.a(); setOpen(false); }} style={{padding:'10px 14px', borderBottom:'1px solid var(--hair-2)', cursor:'pointer', display:'flex', gap:10, alignItems:'center'}}>
              <span className="mono" style={{fontSize:11, color:'var(--muted)', width:14}}>{it.icon}</span>
              <span style={{fontSize:12, color: it.danger?'var(--down)':'var(--ink)'}}>{it.l}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

Object.assign(window, {
  PortfolioPage, ResearchPage, RiskPage, SettingsPage, OptionsPage, OrderTicketModal,
  AccountDropdown, AlertsDropdown, ProfileDropdown,
});
