// TILES — dashboard with consistent widget system.
// Every tile uses the same chrome: title row (label + meta), content, optional footer.

function Sparkline({ data, color, height = 40, fill = true }) {
  const w = 200, h = height;
  const min = Math.min(...data), max = Math.max(...data);
  const range = max - min || 1;
  const pts = data.map((v, i) => [(i / (data.length - 1)) * w, h - ((v - min) / range) * (h - 4) - 2]);
  const path = pts.map((p, i) => (i === 0 ? `M${p[0]},${p[1]}` : `L${p[0]},${p[1]}`)).join(' ');
  const area = path + ` L${w},${h} L0,${h} Z`;
  return (
    <svg viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none" style={{ width: '100%', height, display: 'block' }}>
      {fill && <path d={area} fill={color} opacity="0.18"/>}
      <path d={path} stroke={color} strokeWidth="1.6" fill="none" strokeLinecap="round" strokeLinejoin="round"/>
    </svg>
  );
}

// Unified widget shell — every dashboard card uses this
function Widget({ label, meta, children, span = 1, rowSpan = 1, accent }) {
  return (
    <div className="widget" style={{ gridColumn: `span ${span}`, gridRow: `span ${rowSpan}` }}>
      <div className="widget-head">
        <span className="widget-label">{label}</span>
        {meta && <span className="widget-meta">{meta}</span>}
      </div>
      <div className="widget-body">
        {children}
      </div>
      <style>{`
        .widget {
          background: var(--bg-elev-1);
          border-radius: 12px;
          padding: 16px 18px;
          display: flex; flex-direction: column;
          position: relative; overflow: hidden;
          min-height: 0;
        }
        .widget-head {
          display: flex; align-items: baseline; justify-content: space-between;
          gap: 12px;
          flex: 0 0 auto;
        }
        .widget-label {
          font: 600 11px 'Inter', sans-serif; letter-spacing: 0.04em;
          text-transform: uppercase; color: var(--text-tertiary);
        }
        .widget-meta {
          font: 500 11px 'Inter Tight', sans-serif; letter-spacing: -0.005em;
          color: var(--text-tertiary);
        }
        .widget-body { flex: 1; min-height: 0; display: flex; flex-direction: column; margin-top: 12px; }
      `}</style>
    </div>
  );
}

function SentimentDial({ value }) {
  const segs = 24;
  const filled = Math.round((value / 100) * segs);
  return (
    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', flex: 1, minHeight: 140 }}>
      <svg viewBox="-120 -120 240 240" style={{ width: '100%', maxWidth: 220, height: 'auto' }}>
        {Array.from({length: segs}).map((_, i) => {
          const a0 = -210 + (i * 240 / segs);
          const a1 = a0 + (240 / segs) - 3;
          const r0 = 78, r1 = 108;
          const rad = (d) => (d * Math.PI) / 180;
          const x0 = r0 * Math.cos(rad(a0)), y0 = r0 * Math.sin(rad(a0));
          const x1 = r1 * Math.cos(rad(a0)), y1 = r1 * Math.sin(rad(a0));
          const x2 = r1 * Math.cos(rad(a1)), y2 = r1 * Math.sin(rad(a1));
          const x3 = r0 * Math.cos(rad(a1)), y3 = r0 * Math.sin(rad(a1));
          const isOn = i < filled;
          const t = i / segs;
          const color = isOn
            ? (t < 0.33 ? '#f15e6c' : t < 0.66 ? '#e6c84a' : '#1ed760')
            : 'rgba(255,255,255,0.06)';
          return (
            <path key={i}
              d={`M${x0},${y0} L${x1},${y1} A${r1},${r1} 0 0 1 ${x2},${y2} L${x3},${y3} A${r0},${r0} 0 0 0 ${x0},${y0} Z`}
              fill={color}/>
          );
        })}
        <text x="0" y="6" textAnchor="middle" fill="#fff" style={{ font: '700 38px "Inter Tight"', letterSpacing: '-0.04em' }}>{value}</text>
        <text x="0" y="28" textAnchor="middle" fill="#b3b3b3" style={{ font: '600 10px "Inter"', letterSpacing: '0.06em' }}>GREEDY</text>
      </svg>
    </div>
  );
}

// Standardized "big number + change" content; optional inline action pill (ticket DNA)
function BigNumber({ value, change, changeLabel, color = '#fff', action }) {
  return (
    <div className={'big-num' + (action ? ' has-action' : '')}>
      <div className="bn-stack">
        <div className="bn-value display num" style={{ color }}>{value}</div>
        {change && <div className="bn-change num">{change}<span className="bn-clabel">{changeLabel}</span></div>}
        {!change && changeLabel && <div className="bn-change num"><span className="bn-clabel" style={{marginLeft:0}}>{changeLabel}</span></div>}
      </div>
      {action && (
        <button className={'bn-action ' + (action.tone || 'neutral')} onClick={action.onClick}>
          <span className="bn-action-l">{action.label}</span>
          <span className="bn-action-r">{action.glyph || '→'}</span>
        </button>
      )}
      <style>{`
        .big-num { display: flex; flex-direction: column; gap: 4px; }
        .big-num.has-action {
          flex-direction: row; align-items: stretch; gap: 10px;
        }
        .bn-stack { display: flex; flex-direction: column; gap: 4px; flex: 1; min-width: 0; }
        .bn-value { font: 700 36px 'Inter Tight', sans-serif; letter-spacing: -0.035em; line-height: 1; }
        .bn-change { font: 600 13px 'Inter Tight', sans-serif; letter-spacing: -0.01em; }
        .bn-clabel { color: var(--text-tertiary); font-weight: 500; margin-left: 6px; letter-spacing: -0.005em; }
        .bn-action {
          align-self: flex-end;
          display: inline-flex; align-items: center; gap: 6px;
          height: 30px; padding: 0 6px 0 12px;
          border: 0; cursor: pointer; border-radius: 999px;
          font: 700 11px 'Inter', sans-serif; letter-spacing: 0.01em;
          transition: transform .12s ease, background .12s ease, box-shadow .12s ease;
          white-space: nowrap; flex: 0 0 auto;
        }
        .bn-action:hover { transform: translateY(-1px); }
        .bn-action.neutral {
          background: rgba(255,255,255,0.08); color: #fff;
          box-shadow: inset 0 0 0 1px var(--line-strong);
        }
        .bn-action.neutral:hover { background: rgba(255,255,255,0.14); }
        .bn-action.green {
          background: var(--green); color: #000;
          box-shadow: 0 4px 14px color-mix(in oklch, var(--green) 24%, transparent), inset 0 1px 0 rgba(255,255,255,0.25);
        }
        .bn-action.green:hover { background: var(--green-hover); }
        .bn-action.red {
          background: var(--red); color: #000;
          box-shadow: 0 4px 14px color-mix(in oklch, var(--red) 24%, transparent), inset 0 1px 0 rgba(255,255,255,0.25);
        }
        .bn-action-l { font-family: 'Inter Tight', sans-serif; letter-spacing: -0.01em; }
        .bn-action-r {
          width: 18px; height: 18px;
          display: flex; align-items: center; justify-content: center;
          background: rgba(0,0,0,0.18); border-radius: 5px;
          font: 600 10px 'Inter', sans-serif;
        }
        .bn-action.neutral .bn-action-r {
          background: rgba(255,255,255,0.10); color: #fff;
        }
      `}</style>
    </div>
  );
}

// Sector heatmap — sized rectangles like tradingview/finviz
function SectorHeatmap() {
  // Pack into rows, scale w by allocation
  const total = SECTORS.reduce((s,x)=>s+x.w,0);
  return (
    <div className="hm">
      {SECTORS.map(s => {
        const intensity = Math.min(1, Math.abs(s.chg) / 2);
        const col = s.chg >= 0
          ? `rgba(30,215,96,${0.18 + intensity*0.55})`
          : `rgba(241,94,108,${0.18 + intensity*0.55})`;
        const flex = s.w / total;
        return (
          <div key={s.sym} className="hm-c" style={{ flex, background: col }}>
            <div className="hm-sym">{s.sym}</div>
            <div className={'hm-chg num ' + (s.chg>=0?'pos':'neg')}>
              {s.chg>=0?'+':'−'}{Math.abs(s.chg).toFixed(2)}%
            </div>
          </div>
        );
      })}
      <style>{`
        .hm { display: flex; gap: 2px; flex-wrap: wrap; flex: 1; min-height: 0; height: 100%; }
        .hm-c { min-width: 60px; min-height: 50px; padding: 8px 10px; border-radius: 4px; display:flex; flex-direction: column; justify-content: space-between; cursor: pointer; transition: transform .12s ease; }
        .hm-c:hover { transform: scale(1.02); }
        .hm-sym { font: 800 12px 'Inter Tight'; letter-spacing: -0.02em; color:#fff; }
        .hm-chg { font: 700 13px 'Inter Tight'; letter-spacing: -0.02em; color:#fff; }
      `}</style>
    </div>
  );
}

// Donut chart — allocation
function Donut({ data, size=140, hole=46 }) {
  const total = data.reduce((s,d)=>s+d.pct,0);
  let acc = 0;
  const r = size/2;
  return (
    <svg viewBox={`0 0 ${size} ${size}`} style={{ width: size, height: size }}>
      {data.map((d,i) => {
        const start = (acc/total) * Math.PI * 2 - Math.PI/2;
        acc += d.pct;
        const end = (acc/total) * Math.PI * 2 - Math.PI/2;
        const large = (end-start) > Math.PI ? 1 : 0;
        const x1 = r + r*Math.cos(start), y1 = r + r*Math.sin(start);
        const x2 = r + r*Math.cos(end),   y2 = r + r*Math.sin(end);
        const x3 = r + hole*Math.cos(end),y3 = r + hole*Math.sin(end);
        const x4 = r + hole*Math.cos(start), y4 = r + hole*Math.sin(start);
        return (
          <path key={i} d={`M${x1},${y1} A${r},${r} 0 ${large} 1 ${x2},${y2} L${x3},${y3} A${hole},${hole} 0 ${large} 0 ${x4},${y4} Z`} fill={d.color}/>
        );
      })}
    </svg>
  );
}

// Calendar heatmap — daily P&L
function PnlCalendar() {
  const max = Math.max(...PNL_DAILY.map(d=>Math.abs(d.pnl)));
  return (
    <div className="cal">
      <div className="cal-grid">
        {PNL_DAILY.map(d => {
          const i = Math.min(1, Math.abs(d.pnl)/max);
          const col = d.pnl>=0 ? `rgba(30,215,96,${0.18 + i*0.6})` : `rgba(241,94,108,${0.18 + i*0.6})`;
          return (
            <div key={d.d} className="cal-cell" style={{ background: col }} title={`Day ${d.d}: $${d.pnl}`}>
              <span className="cal-d">{d.d}</span>
            </div>
          );
        })}
      </div>
      <div className="cal-legend">
        <span>Loss</span>
        <span className="cal-scale">
          {[0.2,0.4,0.6,0.8,1].map(i=> <span key={i} style={{background: `rgba(241,94,108,${0.18 + i*0.6})`}}/>)}
          <span style={{background: 'rgba(255,255,255,0.05)'}}/>
          {[0.2,0.4,0.6,0.8,1].map(i=> <span key={i} style={{background: `rgba(30,215,96,${0.18 + i*0.6})`}}/>)}
        </span>
        <span>Profit</span>
      </div>
      <style>{`
        .cal { display:flex; flex-direction: column; gap: 8px; flex: 1; min-height: 0; justify-content: center; }
        .cal-grid { display: grid; grid-template-columns: repeat(10, 1fr); gap: 3px; }
        .cal-cell { aspect-ratio: 1; border-radius: 3px; display:flex; align-items:flex-start; justify-content:flex-end; padding: 2px 4px; }
        .cal-d { font: 600 8px 'Inter Tight'; color: rgba(255,255,255,0.5); }
        .cal-legend { display:flex; align-items:center; gap: 8px; font: 500 9px 'Inter'; letter-spacing: 0.06em; text-transform: uppercase; color: var(--text-tertiary); }
        .cal-scale { display:flex; gap: 2px; flex: 1; }
        .cal-scale > span { flex: 1; height: 8px; border-radius: 2px; }
      `}</style>
    </div>
  );
}

// Multi-line chart — index comparison
function MultiLine({ series, height=120 }) {
  const w = 320, h = height, pad = 6;
  const all = series.flatMap(s=>s.data);
  const min = Math.min(...all), max = Math.max(...all);
  const range = max - min || 1;
  return (
    <svg viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none" style={{ width:'100%', height, display:'block' }}>
      {[0,1,2,3].map(i => (
        <line key={i} x1="0" x2={w} y1={pad + (h-pad*2)*i/3} y2={pad + (h-pad*2)*i/3} stroke="rgba(255,255,255,0.04)"/>
      ))}
      {series.map((s, si) => {
        const pts = s.data.map((v,i)=>[(i/(s.data.length-1))*w, pad + (1-(v-min)/range)*(h-pad*2)]);
        return (
          <path key={si}
            d={pts.map((p,i)=>(i===0?'M':'L')+p[0]+','+p[1]).join(' ')}
            stroke={s.color} strokeWidth="1.6" fill="none" strokeLinecap="round" strokeLinejoin="round"/>
        );
      })}
    </svg>
  );
}

// Hex/octagon stacked dots — mood-inspired holdings visualization
function StackedHex({ pct, color='var(--green)' }) {
  const total = 30;
  const filled = Math.round((pct/100)*total);
  return (
    <div className="sh-grid">
      {Array.from({length: total}).map((_,i) => {
        const isOn = i < filled;
        return <span key={i} className="sh-dot" style={{ background: isOn ? color : 'rgba(255,255,255,0.06)' }}/>;
      })}
      <style>{`
        .sh-grid { display: grid; grid-template-columns: repeat(10, 1fr); gap: 4px; }
        .sh-dot { aspect-ratio: 1; clip-path: polygon(30% 0, 70% 0, 100% 30%, 100% 70%, 70% 100%, 30% 100%, 0 70%, 0 30%); }
      `}</style>
    </div>
  );
}

function TilesScreen() {
  const pnlSeries = [120, 110, 145, 162, 158, 180, 175, 198, 220, 240, 235, 268, 290];
  const spxSeries = [98, 99, 100, 102, 101, 103, 104, 106, 105, 107, 109, 110];
  const ndxSeries = [98, 100, 103, 105, 107, 106, 110, 113, 115, 117, 116, 119];
  const myPort   = [98, 102, 105, 108, 112, 115, 118, 120, 124, 128, 130, 134];

  return (
    <div className="tiles-screen">
      <div className="tiles-grid">
        {/* Hero: P&L */}
        <Widget label="Today's P&L" meta={<span><Ic.TrendUp size={11}/> Realized + unrealized</span>} span={6} rowSpan={2} accent="var(--green)">
          <BigNumber value="+$290,794" change={<span className="pos"><Ic.ArrowUp size={11}/> +1.93%</span>} changeLabel="vs yesterday" color="var(--green)" action={{label:'View trades', tone:'neutral'}}/>
          <div style={{ marginTop: 14 }}><Sparkline data={pnlSeries} color="#1ed760" height={70}/></div>
          <div className="t-stats">
            <Stat l="Realized" v="+$48,210"/>
            <Stat l="Unrealized" v="+$242,584"/>
            <Stat l="Win-rate" v="72%"/>
            <Stat l="Trades" v="38"/>
          </div>
        </Widget>

        {/* Top mover */}
        <Widget label="Top mover" meta="Today" span={3} rowSpan={2}>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            <span style={{ font: "700 30px 'Inter Tight'", letterSpacing: '-0.03em', color: '#fff' }}>NVDA</span>
            <BigNumber value="+2.26%" change={<span className="pos">+$28.40</span>} changeLabel="last $1,284.32" color="var(--green)" action={{label:'Trade', tone:'green', glyph:'↗'}}/>
          </div>
          <div style={{ marginTop: 'auto' }}>
            <Sparkline data={[1180,1190,1185,1210,1230,1215,1240,1255,1260,1278,1284]} color="#1ed760"/>
          </div>
        </Widget>

        {/* Market state */}
        <Widget label="NYSE · Open" meta="06h 42m elapsed" span={3}>
          <BigNumber value="23:21" changeLabel="time to close" color="#fff"/>
        </Widget>

        {/* Volume */}
        <Widget label="Volume · Today" meta="Last 24 bars" span={3}>
          <BigNumber value="14.2B" color="#fff"/>
          <div className="vol-bars">
            {Array.from({length: 24}).map((_, i) => (
              <div key={i} style={{
                height: `${20 + Math.abs(Math.sin(i*0.7)) * 70}%`,
                background: i > 16 ? 'var(--green)' : 'rgba(255,255,255,0.18)',
              }}/>
            ))}
          </div>
        </Widget>

        {/* Index strip — uniform mini cards */}
        <Widget label="S&P 500" span={3}>
          <BigNumber value="5,824.10" change={<span className="pos">+0.38%</span>} changeLabel="+22.04" color="#fff"/>
        </Widget>
        <Widget label="NASDAQ" span={3}>
          <BigNumber value="18,642.20" change={<span className="pos">+0.85%</span>} changeLabel="+157.21" color="#fff"/>
        </Widget>
        <Widget label="VIX" span={3}>
          <BigNumber value="14.82" change={<span className="neg">−3.21%</span>} changeLabel="−0.49" color="#fff"/>
        </Widget>
        <Widget label="10Y · TNX" span={3}>
          <BigNumber value="4.218%" change={<span className="neg">+4.1bp</span>} changeLabel="vs prior" color="#fff"/>
        </Widget>

        {/* Sentiment */}
        <Widget label={<span><Ic.Pulse size={11}/> Market sentiment</span>} meta="24h · n=18.4k" span={4} rowSpan={2}>
          <SentimentDial value={64}/>
          <div className="sent-scale">
            <span>FEAR</span>
            <span style={{ color: 'var(--green)' }}>GREEDY · 64</span>
            <span>EUPHORIC</span>
          </div>
        </Widget>

        {/* Sector heatmap */}
        <Widget label={<span><Ic.Grid size={11}/> Sector heatmap</span>} meta="S&P 500 · today" span={5} rowSpan={2}>
          <SectorHeatmap/>
        </Widget>

        {/* Risk */}
        <Widget label={<span><Ic.Shield size={11}/> Portfolio risk</span>} meta="VaR 95% · 1-day" span={3} rowSpan={2}>
          <BigNumber value="$48.2K" changeLabel="1.7% of NAV" color="#fff" action={{label:'Adjust', tone:'neutral'}}/>
          <div className="risk-stats">
            <Stat l="Beta" v="1.18"/>
            <Stat l="Vol" v="14.2"/>
            <Stat l="Sharpe" v="2.41"/>
          </div>
        </Widget>

        {/* Multi-line index comparison */}
        <Widget label={<span><Ic.Activity size={11}/> Performance</span>} meta="vs SPX · NDX · 30d" span={5} rowSpan={2}>
          <div style={{ display:'flex', gap: 16, marginBottom: 8 }}>
            <span className="legend-item"><span className="legend-dot" style={{background:'#1ed760'}}/>You <b className="num pos">+34%</b></span>
            <span className="legend-item"><span className="legend-dot" style={{background:'#7c3aed'}}/>NDX <b className="num pos">+19%</b></span>
            <span className="legend-item"><span className="legend-dot" style={{background:'#7a7a7a'}}/>SPX <b className="num pos">+10%</b></span>
          </div>
          <div style={{ flex: 1, minHeight: 0, display:'flex', alignItems:'center' }}>
            <MultiLine series={[
              {data: spxSeries, color:'#7a7a7a'},
              {data: ndxSeries, color:'#7c3aed'},
              {data: myPort,    color:'#1ed760'},
            ]} height={140}/>
          </div>
        </Widget>

        {/* Allocation donut */}
        <Widget label={<span><Ic.PieChart size={11}/> Allocation</span>} meta={<button className="rebalance-btn">Rebalance</button>} span={4} rowSpan={2}>
          <div style={{ display:'flex', gap: 14, alignItems:'center', flex:1, minHeight:0 }}>
            <Donut data={ALLOCATION} size={120} hole={42}/>
            <div style={{ flex: 1, display:'flex', flexDirection:'column', gap: 6 }}>
              {ALLOCATION.map(a => (
                <div key={a.label} className="al-item-mini">
                  <span className="al-dot" style={{ background: a.color }}/>
                  <span className="al-l">{a.label}</span>
                  <span className="al-v num">{a.pct}%</span>
                </div>
              ))}
            </div>
          </div>
        </Widget>

        {/* P&L calendar heatmap */}
        <Widget label={<span><Ic.Calendar size={11}/> 30-day P&L</span>} meta={<span className="pos"><Ic.Fire size={11}/> 7-day streak</span>} span={4} rowSpan={2}>
          <PnlCalendar/>
        </Widget>

        {/* Earnings on deck */}
        <Widget label={<span><Ic.Calendar size={11}/> Earnings on deck</span>} meta="Next 7 sessions" span={4} rowSpan={2}>
          <div className="earn-list">
            {EARNINGS.map((e,i) => (
              <div key={i} className="earn-row">
                <div className="earn-date">
                  <span className="earn-day">{e.date}</span>
                  <span className="earn-d num">{e.d}</span>
                </div>
                <div className="earn-meta">
                  <span className="earn-sym">{e.sym}</span>
                  <span className="earn-t">{e.t==='AMC'?<><Ic.Layers size={9}/>AMC</>:<><Ic.ArrowUp size={9}/>BMO</>}</span>
                </div>
                <div className="earn-num">
                  <span className="earn-l">EPS</span>
                  <span className="num">{e.est}</span>
                </div>
                <div className="earn-num">
                  <span className="earn-l">Implied</span>
                  <span className="num">{e.impl}</span>
                </div>
              </div>
            ))}
          </div>
        </Widget>

        {/* Hex-stacked: long/short ratio (mood reference) */}
        <Widget label={<span><Ic.Layers size={11}/> Long / short</span>} meta="Account exposure" span={4}>
          <div style={{ display:'grid', gridTemplateColumns:'1fr 1fr', gap: 16, alignItems:'center', flex:1, minHeight: 0 }}>
            <div>
              <div style={{ font: "800 36px 'Inter Tight'", letterSpacing: '-0.035em', color:'var(--green)', lineHeight: 1 }}>
                72<span style={{fontSize:'.55em', verticalAlign:'top'}}>%</span>
              </div>
              <div style={{ font:"600 11px 'Inter'", color: 'var(--text-tertiary)', letterSpacing:'0.04em', textTransform:'uppercase', marginTop: 4 }}>Long bias</div>
              <div style={{ marginTop: 10 }}><StackedHex pct={72} color="var(--green)"/></div>
            </div>
            <div>
              <div style={{ font: "800 36px 'Inter Tight'", letterSpacing: '-0.035em', color: '#fff', lineHeight: 1 }}>
                28<span style={{fontSize:'.55em', verticalAlign:'top'}}>%</span>
              </div>
              <div style={{ font:"600 11px 'Inter'", color: 'var(--text-tertiary)', letterSpacing:'0.04em', textTransform:'uppercase', marginTop: 4 }}>Short / hedge</div>
              <div style={{ marginTop: 10 }}><StackedHex pct={28} color="#fff"/></div>
            </div>
          </div>
        </Widget>

        {/* Top movers horizontal bars */}
        <Widget label={<span><Ic.BarChart size={11}/> Today's top movers</span>} meta="By % chg" span={4}>
          <div className="mov-list">
            {SCREENER.slice(0,6).map(r => {
              const up = r.chg>=0;
              const w = Math.min(100, Math.abs(r.chg)/10*100);
              return (
                <div key={r.sym} className="mov-row">
                  <span className="mov-sym">{r.sym}</span>
                  <div className="mov-bar-wrap">
                    <div className={'mov-bar ' + (up?'pos':'neg')} style={{ width: `${w}%` }}/>
                  </div>
                  <span className={'mov-chg num ' + (up?'pos':'neg')}>{up?'+':'−'}{Math.abs(r.chg).toFixed(2)}%</span>
                </div>
              );
            })}
          </div>
        </Widget>

        {/* News */}
        <Widget label={<span><Ic.News size={11}/> News</span>} meta={<span className="live-pill"><span className="live-dot"/>Streaming</span>} span={4}>
          <div className="news-list">
            {NEWS.slice(0,4).map((n, i) => (
              <div key={i} className="news-row">
                <span className="news-time num">{n.time}</span>
                <span className="news-src">{n.src}</span>
                <span className="news-head">{n.head}</span>
              </div>
            ))}
          </div>
        </Widget>
      </div>

      <style>{`
        .tiles-screen { padding: 12px; height: 100%; overflow-y: auto; flex: 1; }
        .tiles-grid {
          display: grid;
          grid-template-columns: repeat(12, 1fr);
          grid-auto-rows: 132px;
          gap: 12px;
        }

        .t-stats { display: grid; grid-template-columns: repeat(4, 1fr); gap: 14px; margin-top: auto; padding-top: 12px; border-top: 1px solid var(--line); }

        .vol-bars { display: flex; gap: 2px; align-items: flex-end; height: 40px; margin-top: auto; }
        .vol-bars > div { flex: 1; border-radius: 1px; min-height: 4px; }

        .sent-scale {
          display: flex; justify-content: space-between;
          font: 600 10px 'Inter', sans-serif; color: var(--text-tertiary);
          letter-spacing: 0.06em; padding-top: 8px; border-top: 1px solid var(--line);
        }

        .wl-rows { display: flex; flex-direction: column; flex: 1; gap: 0; }
        .wl-row {
          display: grid; grid-template-columns: 60px 1fr 86px 70px; gap: 12px;
          align-items: center; padding: 10px 0;
          border-bottom: 1px solid var(--line);
        }
        .wl-row:last-child { border-bottom: 0; }
        .wl-sym { font: 700 13px 'Inter', sans-serif; letter-spacing: -0.01em; color: #fff; }
        .wl-name { font: 500 11px 'Inter', sans-serif; color: var(--text-secondary); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
        .wl-px { font: 600 12px 'Inter Tight', sans-serif; color: var(--text-secondary); text-align: right; }
        .wl-chg { font: 600 12px 'Inter Tight', sans-serif; text-align: right; }

        .risk-stats {
          display: grid; grid-template-columns: repeat(3, 1fr); gap: 14px;
          margin-top: auto; padding-top: 12px; border-top: 1px solid var(--line);
        }

        .alloc-bar {
          height: 10px; border-radius: 999px; overflow: hidden;
          display: flex; gap: 2px; margin-top: 4px;
        }
        .alloc-bar > div { height: 100%; }
        .alloc-legend { display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px 18px; margin-top: 14px; }
        .al-item { display: flex; align-items: center; gap: 8px; min-width: 0; }
        .al-dot { width: 8px; height: 8px; border-radius: 2px; flex: 0 0 auto; }
        .al-l { font: 500 12px 'Inter', sans-serif; color: var(--text-secondary); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; flex: 1; letter-spacing: -0.005em; }
        .al-v { font: 600 12px 'Inter Tight', sans-serif; color: #fff; letter-spacing: -0.01em; }

        .rebalance-btn {
          height: 24px; padding: 0 10px; border-radius: 999px;
          background: rgba(255,255,255,0.06); color: #fff; border: 0;
          font: 600 11px 'Inter', sans-serif; cursor: pointer; letter-spacing: -0.005em;
        }
        .rebalance-btn:hover { background: rgba(255,255,255,0.12); }

        .live-pill {
          display: inline-flex; align-items: center; gap: 6px;
          font: 600 11px 'Inter', sans-serif; letter-spacing: -0.005em; color: var(--green);
        }
        .live-dot { width: 6px; height: 6px; border-radius: 50%; background: var(--green); animation: pulse 1.6s ease-in-out infinite; }
        @keyframes pulse { 0%,100% { opacity: 1; } 50% { opacity: 0.3; } }

        .news-list { display: flex; flex-direction: column; gap: 0; flex: 1; }
        .news-row {
          display: grid; grid-template-columns: 50px 60px 1fr; gap: 14px; align-items: baseline;
          padding: 8px 0; border-bottom: 1px solid var(--line); font-size: 12px;
        }
        .news-row:last-child { border-bottom: 0; }
        .news-time { color: var(--text-tertiary); font: 500 11px 'Inter Tight', sans-serif; letter-spacing: -0.01em; }
        .news-src {
          font: 600 9px 'Inter', sans-serif; letter-spacing: 0.06em; text-transform: uppercase;
          color: var(--text-secondary); padding: 2px 6px; border: 1px solid var(--line); border-radius: 4px;
          width: fit-content; align-self: center; text-align: center;
        }
        .news-head { color: var(--text-primary); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; font: 500 13px 'Inter', sans-serif; letter-spacing: -0.005em; }

        .legend-item { display: inline-flex; align-items: center; gap: 6px; font: 600 11px 'Inter'; color: var(--text-secondary); letter-spacing: -0.005em; }
        .legend-item b { font: 700 12px 'Inter Tight'; letter-spacing: -0.01em; margin-left: 2px; }
        .legend-dot { width: 8px; height: 8px; border-radius: 2px; }

        .al-item-mini { display: grid; grid-template-columns: 8px 1fr auto; gap: 8px; align-items: center; }
        .al-item-mini .al-dot { width: 8px; height: 8px; border-radius: 2px; }
        .al-item-mini .al-l { font: 500 11px 'Inter'; color: var(--text-secondary); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; letter-spacing: -0.005em; }
        .al-item-mini .al-v { font: 600 11px 'Inter Tight'; color: #fff; letter-spacing: -0.01em; }

        .earn-list { display: flex; flex-direction: column; gap: 1px; flex: 1; min-height: 0; }
        .earn-row { display: grid; grid-template-columns: 38px 1fr 70px 70px; gap: 10px; align-items: center; padding: 6px 0; border-bottom: 1px solid var(--line); }
        .earn-row:last-child { border-bottom: 0; }
        .earn-date { display: flex; flex-direction: column; align-items: center; padding: 4px 0; background: rgba(255,255,255,0.04); border-radius: 6px; }
        .earn-day { font: 700 8px 'Inter'; letter-spacing: 0.08em; text-transform: uppercase; color: var(--text-tertiary); }
        .earn-d { font: 700 14px 'Inter Tight'; letter-spacing: -0.02em; color: #fff; line-height: 1; }
        .earn-meta { display: flex; flex-direction: column; gap: 2px; }
        .earn-sym { font: 700 13px 'Inter Tight'; letter-spacing: -0.01em; color: #fff; }
        .earn-t { display: inline-flex; align-items: center; gap: 4px; font: 600 9px 'Inter'; letter-spacing: 0.06em; text-transform: uppercase; color: var(--text-tertiary); }
        .earn-num { display: flex; flex-direction: column; gap: 1px; align-items: flex-end; }
        .earn-l { font: 600 8px 'Inter'; letter-spacing: 0.08em; text-transform: uppercase; color: var(--text-tertiary); }
        .earn-num .num { font: 600 12px 'Inter Tight'; letter-spacing: -0.01em; color: #fff; }

        .mov-list { display: flex; flex-direction: column; gap: 6px; flex: 1; justify-content: center; }
        .mov-row { display: grid; grid-template-columns: 50px 1fr 70px; gap: 10px; align-items: center; }
        .mov-sym { font: 700 12px 'Inter Tight'; letter-spacing: -0.01em; color: #fff; }
        .mov-bar-wrap { height: 6px; border-radius: 3px; background: rgba(255,255,255,0.04); overflow: hidden; }
        .mov-bar { height: 100%; border-radius: 3px; }
        .mov-bar.pos { background: var(--green); }
        .mov-bar.neg { background: var(--red); }
        .mov-chg { text-align: right; font: 700 12px 'Inter Tight'; letter-spacing: -0.01em; }

        .widget-label { display: inline-flex; align-items: center; gap: 5px; }
        .widget-label svg { color: var(--text-secondary); }
        .widget-meta svg { vertical-align: -1px; margin-right: 2px; }
      `}</style>
    </div>
  );
}

window.TilesScreen = TilesScreen;
