// ed-dashboard.jsx — Screen I: Dashboard, newspaper-layout edition
// Reverts the tile-bento back to column rules + ruled section heads.

function VBarsChart({ count = 30, seed = 7, vol = 0.6, tint = false, height = 56 }){
  const r = mulberry32(seed);
  const data = Array.from({ length: count }, (_, i) => ({
    v: 0.35 + Math.abs(Math.sin(i * 0.4 + r()*2)) * 0.6 + (i/(count*3)),
    up: r() > 0.42
  }));
  return (
    <div className="vbars" style={{ height }}>
      {data.map((d, i) => (
        <i key={i}
           className={tint ? (d.up ? 'up' : 'dn') : ''}
           style={{ height: `${Math.min(100, d.v*100)}%` }}/>
      ))}
    </div>
  );
}

function Donut({ value = 0.62, size = 100, stroke = 8 }){
  const r = (size - stroke) / 2;
  const c = 2 * Math.PI * r;
  return (
    <svg width={size} height={size}>
      <circle cx={size/2} cy={size/2} r={r} fill="none"
              stroke="var(--rule-2)" strokeWidth={stroke}/>
      <circle cx={size/2} cy={size/2} r={r} fill="none"
              stroke="var(--ink-0)" strokeWidth={stroke}
              strokeDasharray={`${c*value} ${c}`} strokeLinecap="butt"
              transform={`rotate(-90 ${size/2} ${size/2})`}/>
    </svg>
  );
}

function DashWatchTable({ syms, activeSym, setActiveSym }){
  const groups = [
    { label:'Megacaps · Tech',  range:[0, 5] },
    { label:'Cyclical · Auto', range:[5, 9] },
    { label:'Index · ETF',     range:[9, 12] },
  ];

  return (
    <div>
      <div className="search" style={{ marginBottom: 10 }}>
        <input placeholder="Find a security…"/>
        <span className="kbd">⌘K</span>
      </div>

      <table className="table dense">
        <thead>
          <tr>
            <th>Issue</th>
            <th>30d</th>
            <th className="r">Last</th>
            <th className="r">Day</th>
          </tr>
        </thead>
        <tbody>
        {groups.map(g => (
          <React.Fragment key={g.label}>
            <tr className="subhead"><td colSpan={4}>{g.label}</td></tr>
            {syms.slice(g.range[0], g.range[1]).map((s, i) => {
              const spark = makeArea(20, s.sym.length*17 + i, s.px*0.95, s.px*0.008);
              const on = activeSym === s.sym;
              return (
                <tr key={s.sym} className={on ? 'on' : ''}
                    onClick={() => setActiveSym && setActiveSym(s.sym)}>
                  <td>
                    <div className="watch-cell-sym">
                      <span className="ticker">{s.sym}</span>
                      <span className="desc">{s.name.slice(0, 14)}</span>
                    </div>
                  </td>
                  <td><EditorialSpark data={spark} w={56} h={18} fill/></td>
                  <td className="r px">{fmtNum(s.px)}</td>
                  <td className={`r ${s.ch>=0?'up':'dn'}`}>{fmtCh(s.ch)}</td>
                </tr>
              );
            })}
          </React.Fragment>
        ))}
        </tbody>
      </table>
    </div>
  );
}

function StatPair({ label, value, sub, tone }){
  return (
    <div className="pair">
      <dt>{label}</dt>
      <dd style={{ color: tone === 'up' ? 'var(--buy)' : tone === 'dn' ? 'var(--sell)' : null }}>
        {value}{sub && <span className="pct">{sub}</span>}
      </dd>
    </div>
  );
}

// ── Day Pulse — intraday timeline with annotated session events ──
function DayPulse(){
  const { points, events, lo, hi } = useMemo(() => {
    const r = mulberry32(83);
    const n = 78;  // 09:30 → 16:00, 5-min bars = 78 bars
    let v = 3.198;
    const points = [];
    for (let i = 0; i < n; i++){
      v += (r() - 0.48) * 0.0042;
      // Inject a few inflection events
      if (i === 12) v += 0.022;   // 10:30 — Powell
      if (i === 24) v -= 0.018;   // 11:30 — soft auction
      if (i === 48) v += 0.034;   // 13:30 — Nvidia headline
      if (i === 66) v -= 0.012;   // 15:00 — late tape softness
      points.push(v);
    }
    const events = [
      { idx: 12, t:'10:30', kicker:'POWELL · Q&A', body:'Reiterates patience; tape lifts.' },
      { idx: 24, t:'11:30', kicker:'3Y AUCTION',   body:'1.4bp tail; risk fades briefly.' },
      { idx: 48, t:'13:30', kicker:'NVDA NEWS',    body:'Blackwell shipments accelerate.' },
      { idx: 66, t:'15:00', kicker:'CLOSE TONE',   body:'Mild profit-take into the bell.' },
    ];
    return { points, events, lo: Math.min(...points) - 0.005, hi: Math.max(...points) + 0.005 };
  }, []);

  const W = 1080, H = 140;
  const padL = 8, padR = 56, padT = 10, padB = 26;
  const w = W - padL - padR;
  const h = H - padT - padB;
  const x = (i) => padL + (i / (points.length - 1)) * w;
  const y = (v) => padT + (1 - (v - lo) / (hi - lo)) * h;
  const open = points[0];
  let path = `M${x(0).toFixed(1)} ${y(points[0]).toFixed(1)}`;
  for (let i = 1; i < points.length; i++) path += ` L${x(i).toFixed(1)} ${y(points[i]).toFixed(1)}`;
  const last = points[points.length - 1];
  // X-axis hour ticks (each 12 bars = 1h)
  const hours = ['09:30','10:30','11:30','12:30','13:30','14:30','15:30','16:00'];

  return (
    <div style={{ position:'relative', width:'100%' }}>
      <svg className="ed-chart" viewBox={`0 0 ${W} ${H}`}
            preserveAspectRatio="xMidYMid meet"
            style={{ display:'block', width:'100%', height:'auto' }}>
        {/* Open reference (dashed) */}
        <line className="grid-line" x1={padL} x2={padL + w} y1={y(open)} y2={y(open)}
              strokeDasharray="3 3"/>
        <text className="axis-text" x={padL + w + 4} y={y(open) + 3.5}>open · {open.toFixed(3)}</text>

        {/* Pulse line */}
        <path className="price-line" d={path}/>
        <path d={`${path} L${(padL + w).toFixed(1)} ${(padT + h).toFixed(1)} L${padL.toFixed(1)} ${(padT + h).toFixed(1)} Z`}
              fill="var(--paper-3)" opacity=".55"/>

        {/* Event annotations */}
        {events.map((e, i) => (
          <g key={i}>
            <line className="annotation" x1={x(e.idx)} x2={x(e.idx)}
                  y1={padT} y2={padT + h}/>
            <circle className="last-dot" cx={x(e.idx)} cy={y(points[e.idx])} r="2.5"/>
            <text x={x(e.idx) + 4} y={padT + 11}
                  fontFamily="var(--sans)" fontSize="9"
                  letterSpacing=".14em" fill="var(--claret)"
                  fontWeight="600">{e.kicker}</text>
          </g>
        ))}

        {/* X-axis labels */}
        {hours.map((t, i) => {
          const xx = padL + (i / (hours.length - 1)) * w;
          return (
            <g key={t}>
              <line x1={xx} x2={xx} y1={padT + h} y2={padT + h + 3} stroke="var(--rule-2)" strokeWidth=".5"/>
              <text className="axis-text" x={xx} y={H - 12} textAnchor="middle">{t}</text>
            </g>
          );
        })}

        {/* Last price marker */}
        <circle className="last-dot" cx={x(points.length - 1)} cy={y(last)} r="3"/>
        <text className="axis-text" x={padL + w + 4} y={y(last) + 3.5}
              style={{ fontWeight:600, fill:'var(--claret)' }}>{last.toFixed(3)}</text>
      </svg>

      {/* Event chips beneath chart */}
      <div style={{
        display:'grid', gridTemplateColumns:'repeat(4, 1fr)', gap: 0,
        marginTop: 6, borderTop:'.5px solid var(--rule-2)',
      }}>
        {events.map((e, i) => (
          <div key={i} style={{
            padding:'8px 12px',
            borderRight: i < events.length - 1 ? '.5px solid var(--rule-2)' : 'none',
          }}>
            <div style={{ display:'flex', alignItems:'baseline', gap: 6 }}>
              <span style={{
                fontFamily:'var(--mono)', fontSize: 10, color:'var(--ink-2)',
              }}>{e.t}</span>
              <span style={{
                fontFamily:'var(--sans)', fontSize: 9, letterSpacing:'.14em',
                color:'var(--claret)', fontWeight: 600, textTransform:'uppercase',
              }}>{e.kicker}</span>
            </div>
            <div style={{
              fontFamily:'var(--serif)', fontSize: 12, fontStyle:'italic',
              color:'var(--ink-1)', marginTop: 2, lineHeight: 1.35,
            }}>{e.body}</div>
          </div>
        ))}
      </div>
    </div>
  );
}

// ── Performance attribution — source-of-return table ──
function PerfAttribution(){
  const rows = [
    { label:'Long beta',         pnl:+18420, w: 0.86, tone:'up' },
    { label:'Stock selection',   pnl:+22640, w: 1.00, tone:'up' },
    { label:'Sector tilt',       pnl: +6840, w: 0.32, tone:'up' },
    { label:'Hedges (SPY puts)', pnl: -3120, w: 0.18, tone:'dn' },
    { label:'Theta · options',   pnl: -1840, w: 0.10, tone:'dn' },
    { label:'Slippage & fees',   pnl:  -822, w: 0.05, tone:'dn' },
  ];
  const max = Math.max(...rows.map(r => Math.abs(r.pnl)));
  return (
    <table className="table dense" style={{ width:'100%' }}>
      <thead>
        <tr>
          <th>Source</th>
          <th>Contribution</th>
          <th className="r">$</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((r, i) => {
          const w = (Math.abs(r.pnl) / max) * 100;
          return (
            <tr key={i}>
              <td className="sector" style={{ fontFamily:'var(--serif)', fontStyle:'italic' }}>
                {r.label}
              </td>
              <td>
                <span style={{
                  display:'inline-block', height: 8,
                  width: `${w}%`,
                  background: r.tone === 'up' ? 'var(--buy)' : 'var(--sell)',
                  opacity: .75,
                }}/>
              </td>
              <td className={`r ${r.tone === 'up' ? 'up-strong' : 'dn-strong'}`}>
                {r.pnl >= 0 ? '+' : '−'}${Math.abs(r.pnl).toLocaleString()}
              </td>
            </tr>
          );
        })}
        <tr style={{ borderTop:'2px solid var(--ink-0)' }}>
          <td colSpan={2} style={{
            fontFamily:'var(--sans)', fontSize: 10, fontWeight: 700,
            letterSpacing:'.14em', textTransform:'uppercase', color:'var(--ink-0)',
            paddingTop: 8,
          }}>Total day P&amp;L</td>
          <td className="r up-strong" style={{
            fontFamily:'var(--serif)', fontWeight: 700, fontSize: 16, paddingTop: 8,
          }}>+$42,118</td>
        </tr>
      </tbody>
    </table>
  );
}

// ── Today's executions blotter (compact) ──
function DashBlotter(){
  const trades = useMemo(() => {
    const r = mulberry32(37);
    const syms = ['NVDA','SPY','AAPL','TSLA','META','AMD','MSFT','GOOGL'];
    return syms.map((sym, i) => {
      const ref = SYMS.find(s => s.sym === sym);
      const side = r() > 0.45 ? 'BUY' : 'SELL';
      const qty  = (Math.floor(r() * 5) + 1) * 50;
      const px   = ref ? ref.px + (r() - 0.5) * ref.px * 0.005 : 100;
      const hh   = 9 + Math.floor(r() * 7);
      const mm   = Math.floor(r() * 60);
      const t    = `${String(hh).padStart(2,'0')}:${String(mm).padStart(2,'0')}`;
      return { sym, side, qty, px, t };
    });
  }, []);

  return (
    <table className="table dense" style={{ width:'100%' }}>
      <thead>
        <tr>
          <th>Time</th>
          <th>Issue</th>
          <th>Side</th>
          <th className="r">Qty</th>
          <th className="r">Fill</th>
        </tr>
      </thead>
      <tbody>
        {trades.map((t, i) => (
          <tr key={i}>
            <td style={{ color:'var(--ink-2)' }}>{t.t}</td>
            <td className="sym">{t.sym}</td>
            <td className={t.side === 'BUY' ? 'up-strong' : 'dn-strong'}
                style={{ fontFamily:'var(--sans)', fontSize:9.5,
                          letterSpacing:'.14em', fontWeight: 700 }}>
              {t.side}
            </td>
            <td className="r">{t.qty}</td>
            <td className="r px">{fmtNum(t.px)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

// ── Risk register — limit / current / utilization ──
function RiskRegister(){
  const rows = [
    { label:'Gross exposure',   v:'213%',  lim:'250%', pct: 85, tone:'ok' },
    { label:'Net delta',        v:'+62',   lim:'±100', pct: 62, tone:'ok' },
    { label:'Single name',      v:'18.2%', lim:'25%',  pct: 73, tone:'ok' },
    { label:'Sector concentration', v:'38%', lim:'45%', pct: 84, tone:'warn' },
    { label:'VaR · 95% · 1d',   v:'$84k',  lim:'$120k',pct: 70, tone:'ok' },
    { label:'Drawdown · MTD',   v:'−1.8%', lim:'−5%',  pct: 36, tone:'ok' },
  ];
  return (
    <table className="table dense" style={{ width:'100%' }}>
      <thead>
        <tr>
          <th>Limit</th>
          <th className="r">Current</th>
          <th className="r">Cap</th>
          <th>Utilisation</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((r, i) => {
          const col = r.tone === 'warn' ? 'var(--claret)' : 'var(--ink-0)';
          return (
            <tr key={i}>
              <td className="sector" style={{ fontFamily:'var(--serif)', fontStyle:'italic' }}>
                {r.label}
              </td>
              <td className="r px" style={{ color: col }}>{r.v}</td>
              <td className="r" style={{ color:'var(--ink-2)' }}>{r.lim}</td>
              <td>
                <div style={{
                  position:'relative', height: 8,
                  background:'var(--paper-3)',
                  border:'.5px solid var(--rule-2)',
                }}>
                  <span style={{
                    position:'absolute', left: 0, top: 0, bottom: 0,
                    width: `${r.pct}%`,
                    background: r.tone === 'warn' ? 'var(--claret)' : 'var(--ink-0)',
                    opacity: .75,
                  }}/>
                </div>
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}


function ScreenDashboard({ activeSym, setActiveSym }){
  const positions = SYMS.filter(s => s.qty);
  const longCount  = positions.filter(p => p.qty > 0).length;
  const shortCount = positions.filter(p => p.qty < 0).length;
  const navSeries  = useMemo(() => makeArea(45, 17, 280, 6).map(v => 2.96 + v * 0.001), []);
  const topGain    = [...SYMS].sort((a,b)=>b.ch-a.ch)[0];
  const topLose    = [...SYMS].sort((a,b)=>a.ch-b.ch)[0];

  return (
    <div className="page-inner">

      {/* ── Letter from the desk — editorial header strip ─── */}
      <div className="grid-12" style={{ paddingTop: 16, paddingBottom: 18,
                                          borderBottom: '2px solid var(--ink-0)' }}>
        <div className="col-7">
          <div className="eyebrow">Letter from the Desk · 17 May</div>
          <h1 className="headline h2" style={{ marginTop: 2 }}>
            Book runs to a fresh high as the rally widens beyond the megacaps.
          </h1>
          <p className="dek" style={{ fontSize: 14 }}>
            Net liquidation crests <span style={{ color:'var(--ink-0)', fontStyle:'normal',
              fontFamily:'var(--mono)', fontWeight:600 }}>$3.24m</span> · risk holds in
            check despite a softer Treasury bid.
          </p>
        </div>
        <div className="col-5 vrule">
          <div className="eyebrow muted">Session at a glance · 16:00 ET</div>
          <div style={{ display:'grid', gridTemplateColumns:'repeat(3, 1fr)', gap:0,
                          marginTop: 6 }}>
            {[
              { l:'Day P&L',   v:'+$42,118', tone:'up' },
              { l:'NAV',       v:'$3.24M' },
              { l:'YTD',       v:'+18.4%', tone:'up' },
            ].map((s, i) => (
              <div key={i} style={{
                padding:'2px 12px',
                borderRight: i<2 ? '1px solid var(--rule-2)' : 'none'
              }}>
                <div style={{ fontFamily:'var(--sans)', fontSize:9,
                                letterSpacing:'.14em', textTransform:'uppercase',
                                color:'var(--ink-2)', fontWeight:600 }}>{s.l}</div>
                <div className="num" style={{ fontFamily:'var(--serif)', fontWeight:700,
                                                fontSize: 22, marginTop: 2,
                                                color: s.tone === 'up' ? 'var(--buy-ink)' : 'var(--ink-0)' }}>
                  {s.v}
                </div>
              </div>
            ))}
          </div>
          <div className="byline" style={{ marginTop: 6 }}>
            <span className="dateline">New York —</span> by
            <span className="name"> The Desk</span> · 16:14 ET
          </div>
        </div>
      </div>

      {/* ── Top zone ───────────────────────────────────── */}
      <div className="grid-12" style={{ paddingTop: 18 }}>

        {/* col-5 · NAV hero */}
        <div className="col-5">
          <div className="sec-head">
            <span className="ttl">Net Liquidation</span>
            <span className="sub">USD · live · YTD +18.4%</span>
          </div>

          <div className="hero-quote">
            <span className="px">3,240<span className="frac">,194</span></span>
            <span className="ch up">+$42,118</span>
            <span className="ch up">+1.32%</span>
            <span className="pct">today vs. previous close</span>
          </div>

          <div style={{ marginTop: 14, width:'100%' }}>
            <EditorialLine data={navSeries} width={560} height={150}
                            padding={{ t:6, r:34, b:14, l:6 }} lastLabel/>
          </div>
          <div className="figcap" style={{ borderTop:'.5px solid var(--rule-1)', paddingTop: 5 }}>
            Figure I — Account NAV, 45 sessions. Last printed value highlighted in claret.
          </div>

          <dl className="dl-stats" style={{ marginTop: 14 }}>
            <StatPair label="Realised" value="+$48,200" sub="+1.78%" tone="up"/>
            <StatPair label="Unrealised" value="+$242,594" sub="+8.16%" tone="up"/>
            <StatPair label="Cash" value="$2.42M"/>
            <StatPair label="Buying Power" value="$5.83M"/>
          </dl>

          {/* House view — pull quote */}
          <div className="pullquote" style={{ marginTop: 18, fontSize: 18 }}>
            "Edge is what survives a quiet tape."
            <span className="attrib">House view · weekly memo · 12 May</span>
          </div>
        </div>

        {/* col-4 · Performance summaries */}
        <div className="col-4 vrule">
          {/* P&L 30d */}
          <div className="sec-head">
            <span className="ttl">P&amp;L · 30 sessions</span>
            <span className="sub">Real / est.</span>
          </div>
          <div className="bignum" style={{ fontSize: 36, color:'var(--buy-ink)' }}>
            +17.43<span className="pct">%</span>
          </div>
          <div style={{ marginTop: 8, color:'var(--ink-0)' }}>
            <VBarsChart count={30} seed={11} tint height={50}/>
          </div>
          <div style={{ display:'flex', justifyContent:'space-between', marginTop:6,
                        fontFamily:'var(--sans)', fontSize:9.5, letterSpacing:'.12em',
                        textTransform:'uppercase', color:'var(--ink-2)' }}>
            <span>real <span className="num up">+18.56%</span></span>
            <span>est <span className="num up">+15.78%</span></span>
          </div>

          {/* Sharpe + Top Mover */}
          <div className="grid-12 hrule" style={{ marginTop: 16, gap: 14 }}>
            <div className="col-6">
              <div className="sec-head">
                <span className="ttl">Sharpe · YTD</span>
              </div>
              <div style={{ display:'flex', gap:10, alignItems:'center' }}>
                <Donut value={0.62} size={72} stroke={5}/>
                <div>
                  <div className="bignum" style={{ fontSize: 28 }}>2.41</div>
                  <div style={{ fontFamily:'var(--sans)', fontSize:9,
                                letterSpacing:'.14em', textTransform:'uppercase',
                                color:'var(--ink-2)', marginTop:3 }}>
                    Sortino 3.02
                  </div>
                </div>
              </div>
            </div>
            <div className="col-6" style={{ borderLeft:'1px solid var(--rule-2)', paddingLeft:14 }}>
              <div className="sec-head">
                <span className="ttl">Top mover</span>
              </div>
              <div className="bignum" style={{ fontSize: 26 }}>TSLA</div>
              <div className="num dn"
                    style={{ fontSize:14, fontWeight:600, marginTop:2 }}>−3.88%</div>
              <div style={{ fontFamily:'var(--serif)', fontSize:11.5, fontStyle:'italic',
                            color:'var(--ink-2)', marginTop:6, lineHeight:1.4 }}>
                Q3 deliveries missed by 8.2k units.
              </div>
            </div>
          </div>

          {/* Today on the wire — 3 compact dispatches */}
          <div className="hrule" style={{ marginTop: 18 }}>
            <div className="sec-head">
              <span className="ttl">Today on the wire</span>
              <span className="sub">3 of 14 · live</span>
            </div>
            {[
              { k:'Earnings', src:'BBG',  city:'SAN JOSE', t:'15:42',
                h:'Nvidia surges past $3.16tn as Blackwell shipments accelerate' },
              { k:'Autos',    src:'RTRS', city:'AUSTIN',   t:'15:38',
                h:'Tesla Q3 deliveries miss estimates as China demand softens' },
              { k:'Macro',    src:'BBG',  city:'NY',       t:'15:17',
                h:'JPMorgan lifts S&P 500 year-end target to 6,200 on AI capex' },
            ].map((w, i) => (
              <article key={i} className="news-article" style={{ padding:'8px 0' }}>
                <div className="kicker" style={{ fontSize:9, marginBottom:2 }}>{w.k}</div>
                <h3 className="hed" style={{ fontSize:13.5, lineHeight:1.2 }}>{w.h}</h3>
                <div className="meta" style={{ marginTop:4 }}>
                  <span className="src">{w.src}</span> · {w.city} · {w.t} ET
                </div>
              </article>
            ))}
          </div>
        </div>

        {/* col-3 · Watch list */}
        <div className="col-3 vrule">
          <div className="sec-head">
            <span className="ttl">Watch list</span>
            <span className="sub">{SYMS.length} · live</span>
          </div>
          <DashWatchTable syms={SYMS}
                            activeSym={activeSym}
                            setActiveSym={setActiveSym}/>

          {/* Movers note */}
          <div className="hrule" style={{ marginTop: 16 }}>
            <div className="eyebrow">Movers · today</div>
            <div style={{ fontFamily:'var(--serif)', fontSize:13, lineHeight:1.45,
                            color:'var(--ink-1)', marginTop:4 }}>
              <strong style={{ color:'var(--ink-0)' }}>{topGain.sym}</strong>
              {' '}leads the watch list at{' '}
              <span className="num up">{fmtCh(topGain.ch)}</span>; offsetting weakness from{' '}
              <strong style={{ color:'var(--ink-0)' }}>{topLose.sym}</strong> at{' '}
              <span className="num dn">{fmtCh(topLose.ch)}</span>.
            </div>
          </div>
        </div>
      </div>

      {/* ── Mid band · Day pulse, attribution, executions, risk ───── */}
      <div className="hrule-thick" style={{ marginTop: 22, paddingTop: 14 }}>
        <div className="sec-head">
          <span className="ttl">Day Pulse · intraday narrative</span>
          <span className="sub">09:30 → 16:00 ET · NAV minute bars</span>
        </div>
        <DayPulse/>
      </div>

      <div className="grid-12" style={{ marginTop: 22 }}>
        <div className="col-4">
          <div className="sec-head">
            <span className="ttl">Performance attribution</span>
            <span className="sub">today · USD</span>
          </div>
          <PerfAttribution/>
        </div>

        <div className="col-4 vrule">
          <div className="sec-head">
            <span className="ttl">Today's executions</span>
            <span className="sub">8 of 14 fills · live</span>
          </div>
          <DashBlotter/>
        </div>

        <div className="col-4 vrule">
          <div className="sec-head">
            <span className="ttl">Risk register</span>
            <span className="sub">live · book limits</span>
          </div>
          <RiskRegister/>
        </div>
      </div>

      {/* ── Below the fold ──────────────────────────── */}
      <div className="grid-12 hrule-thick" style={{ marginTop: 22 }}>

        {/* col-8 · Positions ledger */}
        <div className="col-8">
          <div className="sec-head">
            <span className="ttl">Open positions · the ledger</span>
            <span className="sub">{longCount} long · {shortCount} short · 16:00 ET</span>
          </div>

          <table className="table">
            <thead>
              <tr>
                <th>Issue</th>
                <th className="r">Qty</th>
                <th className="r">Avg</th>
                <th className="r">Last</th>
                <th>30 sess.</th>
                <th className="r">Mkt value</th>
                <th className="r">P&amp;L</th>
                <th className="r">%</th>
              </tr>
            </thead>
            <tbody>
              <tr className="subhead"><td colSpan={8}>Long positions · {longCount}</td></tr>
              {positions.filter(p => p.qty > 0).map(p => {
                const mv = Math.abs(p.qty) * p.px;
                const pl = p.qty * (p.px - p.avg);
                const spark = makeArea(20, p.sym.length*23, p.px*0.95, p.px*0.01);
                return (
                  <tr key={p.sym}>
                    <td>
                      <div className="watch-cell-sym">
                        <span className="ticker">{p.sym}</span>
                        <span className="desc">{p.name}</span>
                      </div>
                    </td>
                    <td className="r">{Math.abs(p.qty).toLocaleString()}</td>
                    <td className="r">{fmtNum(p.avg)}</td>
                    <td className="r px">{fmtNum(p.px)}</td>
                    <td><EditorialSpark data={spark} w={60} h={18} fill/></td>
                    <td className="r">${mv.toLocaleString('en-US',{maximumFractionDigits:0})}</td>
                    <td className={`r ${pl>=0?'up':'dn'}`}>
                      {pl>=0?'+':'−'}${Math.abs(pl).toLocaleString('en-US',{maximumFractionDigits:0})}
                    </td>
                    <td className={`r ${p.ch>=0?'up':'dn'}`}>{fmtCh(p.ch)}</td>
                  </tr>
                );
              })}
              <tr className="subhead"><td colSpan={8}>Short positions · {shortCount}</td></tr>
              {positions.filter(p => p.qty < 0).map(p => {
                const mv = Math.abs(p.qty) * p.px;
                const pl = p.qty * (p.px - p.avg);
                const spark = makeArea(20, p.sym.length*29, p.px*0.95, p.px*0.012);
                return (
                  <tr key={p.sym}>
                    <td>
                      <div className="watch-cell-sym">
                        <span className="ticker">{p.sym}</span>
                        <span className="desc">{p.name}</span>
                      </div>
                    </td>
                    <td className="r">({Math.abs(p.qty).toLocaleString()})</td>
                    <td className="r">{fmtNum(p.avg)}</td>
                    <td className="r px">{fmtNum(p.px)}</td>
                    <td><EditorialSpark data={spark} w={60} h={18} fill/></td>
                    <td className="r">${mv.toLocaleString('en-US',{maximumFractionDigits:0})}</td>
                    <td className={`r ${pl>=0?'up':'dn'}`}>
                      {pl>=0?'+':'−'}${Math.abs(pl).toLocaleString('en-US',{maximumFractionDigits:0})}
                    </td>
                    <td className={`r ${p.ch>=0?'up':'dn'}`}>{fmtCh(p.ch)}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>

          {/* Editor's note on positioning */}
          <div className="hrule" style={{ marginTop: 18 }}>
            <div className="eyebrow">From the Editor · positioning</div>
            <h3 className="headline h4">A note on the book into Thursday's CPI.</h3>
            <div className="col-text" style={{ marginTop: 8 }}>
              <p>
                The desk retains a modest long bias into Thursday's print, hedged via the SPY
                put-spread held since Monday. Our view: a 0.2pp surprise either way is already
                priced; tape risk sits in second-order positioning around concentrated factor
                names.
              </p>
              <p>
                Net delta is approximately +0.62; gross exposure 2.13. The hedge book carries a
                positive vega tilt of 4,200 vega-dollars per point, which we will trim if VIX
                opens below 14.
              </p>
            </div>
            <div className="byline" style={{ marginTop: 8 }}>
              <span className="name">Editorial Desk</span> · 16:08 ET
            </div>
          </div>
        </div>

        {/* col-4 · Sectors league + Trading Hours */}
        <div className="col-4 vrule">
          <div className="sec-head">
            <span className="ttl">Sectors · league</span>
            <span className="sub">S&amp;P 500 today</span>
          </div>
          <table className="league">
            <tbody>
              {[
                { name:'Information Technology', ch:+1.84 },
                { name:'Communication Services', ch:+0.42 },
                { name:'Consumer Discretionary', ch:+0.31 },
                { name:'Financials',             ch:+0.12 },
                { name:'Industrials',            ch:-0.08 },
                { name:'Healthcare',             ch:-0.21 },
                { name:'Materials',              ch:-0.34 },
                { name:'Real Estate',            ch:-0.42 },
                { name:'Utilities',              ch:-0.58 },
                { name:'Energy',                 ch:-1.12 },
              ].map((s,i) => {
                const w = Math.min(70, Math.abs(s.ch) * 28);
                return (
                  <tr key={s.name}>
                    <td className="rk">{String(i+1).padStart(2,'0')}</td>
                    <td className="sector">{s.name}</td>
                    <td><span className={`bar ${s.ch>=0?'up':'dn'}`} style={{ width: `${w}px` }}/></td>
                    <td className={`r ${s.ch>=0?'up':'dn'}`}>{fmtCh(s.ch)}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>

          <div className="hrule" style={{ marginTop: 18 }}>
            <div className="sec-head">
              <span className="ttl">Trading hours · YTD</span>
              <span className="sub">cumulative</span>
            </div>
            <div style={{ display:'flex', alignItems:'baseline', gap:10 }}>
              <span className="bignum" style={{ fontSize: 32 }}>554<span className="frac">h</span></span>
              <span style={{ fontFamily:'var(--sans)', fontSize:10, letterSpacing:'.12em',
                              textTransform:'uppercase', color:'var(--ink-2)' }}>
                ~4.6h / session
              </span>
            </div>
            <div style={{ marginTop: 8, color:'var(--ink-0)' }}>
              <VBarsChart count={72} seed={37} vol={0.8} height={48}/>
            </div>
            <div style={{ display:'flex', justifyContent:'space-between', marginTop:4,
                          fontFamily:'var(--sans)', fontSize:8.5, letterSpacing:'.14em',
                          textTransform:'uppercase', color:'var(--ink-3)' }}>
              {['J','F','M','A','M','J','J','A','S','O','N','D'].map((m,i) => <span key={i}>{m}</span>)}
            </div>
          </div>

          <div className="hrule" style={{ marginTop: 18 }}>
            <div className="sec-head">
              <span className="ttl">Diary · this week</span>
              <span className="sub">5 events</span>
            </div>
            <table className="table dense">
              <tbody>
                {[
                  { d:'Mon', date:'18', e:'Powell · Atlanta Fed conference', t:'10:30 ET', tone:'major' },
                  { d:'Tue', date:'19', e:'Retail Sales · April', t:'08:30 ET' },
                  { d:'Wed', date:'20', e:'Housing Starts',       t:'08:30 ET' },
                  { d:'Thu', date:'21', e:'CPI · Core m/m',       t:'08:30 ET', tone:'major' },
                  { d:'Fri', date:'22', e:'U. Michigan Sentiment',t:'10:00 ET' },
                ].map((ev, i) => (
                  <tr key={i}>
                    <td style={{ fontFamily:'var(--sans)', fontSize:10,
                                  letterSpacing:'.14em', textTransform:'uppercase',
                                  fontWeight:600, color:'var(--ink-2)', width: 50 }}>
                      {ev.d} {ev.date}
                    </td>
                    <td className="name" style={{ fontStyle:'normal',
                          color: ev.tone === 'major' ? 'var(--claret)' : 'var(--ink-1)',
                          fontWeight: ev.tone === 'major' ? 600 : 400 }}>
                      {ev.e}
                    </td>
                    <td className="r" style={{ color:'var(--ink-3)', fontSize:11 }}>{ev.t}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      </div>

      {/* ── Action footer ──────────────────────────── */}
      <div className="hrule" style={{ marginTop: 22, display:'flex', gap:8, flexWrap:'wrap',
                                       alignItems:'center' }}>
        <span className="eyebrow muted" style={{ margin: 0 }}>Quick actions</span>
        <span style={{ flex:1 }}/>
        {[
          { label:'New ticket', primary:true },
          { label:'Rebalance' },
          { label:'Hedge book' },
          { label:'Run scenario' },
          { label:'Export tax lots' },
          { label:'Statements →' },
        ].map((b) => (
          <button key={b.label}
                  style={{
                    padding:'7px 14px', fontSize: 10,
                    fontFamily:'var(--sans)', letterSpacing:'.14em',
                    textTransform:'uppercase', fontWeight: 600, cursor:'pointer',
                    background: b.primary ? 'var(--ink-0)' : 'var(--paper)',
                    color: b.primary ? 'var(--paper)' : 'var(--ink-1)',
                    border:'1px solid',
                    borderColor: b.primary ? 'var(--ink-0)' : 'var(--rule-3)',
                  }}>
            {b.label}
          </button>
        ))}
      </div>
    </div>
  );
}

window.ScreenDashboard = ScreenDashboard;
