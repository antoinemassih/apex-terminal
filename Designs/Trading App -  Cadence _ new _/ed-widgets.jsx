// ed-widgets.jsx — Screen IV: "Widgets"
// A newspaper "Almanac" section: market widgets that don't fit elsewhere.
// Volatility, correlation, greeks, calendars, scenarios — each treated as
// a small figure with kicker, headline, dek, and figcap.

function VolSurface({ width = 260, height = 130 }){
  const strikes = [-3, -2, -1, 0, 1, 2, 3];
  const tenors  = ['1W','2W','1M','2M','3M','6M','1Y'];
  const data = useMemo(() => {
    const r = mulberry32(43);
    return tenors.map((t, ti) =>
      strikes.map((s, si) => {
        // Volatility smile: higher at wings, term structure inversion in front
        const smile = 22 + (s * s) * 1.6 - ti * 0.4;
        return Math.max(12, smile + (r() - 0.5) * 2.5);
      })
    );
  }, []);
  const min = Math.min(...data.flat());
  const max = Math.max(...data.flat());
  const cellW = width / strikes.length;
  const cellH = height / tenors.length;
  return (
    <svg className="ed-chart" width={width} height={height}>
      {data.map((row, ti) =>
        row.map((v, si) => {
          const t = (v - min) / (max - min);
          const ink = `rgba(28, 26, 23, ${0.08 + t * 0.62})`;
          return (
            <g key={`${ti}-${si}`}>
              <rect x={si * cellW} y={ti * cellH} width={cellW - 0.5} height={cellH - 0.5}
                    fill={ink}/>
              <text x={si * cellW + cellW/2} y={ti * cellH + cellH/2 + 3}
                    textAnchor="middle"
                    fontFamily="var(--mono)" fontSize="9"
                    fill={t > 0.55 ? 'var(--paper)' : 'var(--ink-1)'}>
                {v.toFixed(0)}
              </text>
            </g>
          );
        })
      )}
    </svg>
  );
}

function CorrelationGrid({ size = 7 }){
  const syms = ['SPY','QQQ','NVDA','AAPL','MSFT','TSLA','META'].slice(0, size);
  const data = useMemo(() => {
    const r = mulberry32(53);
    const m = [];
    for (let i = 0; i < size; i++){
      const row = [];
      for (let j = 0; j < size; j++){
        if (i === j) row.push(1);
        else if (j < i) row.push(m[j][i]); // symmetric
        else row.push(+(0.10 + r() * 0.85).toFixed(2));
      }
      m.push(row);
    }
    return m;
  }, [size]);

  const cellSize = 24;
  return (
    <div style={{ display:'grid',
                  gridTemplateColumns: `42px repeat(${size}, ${cellSize}px)`,
                  fontFamily:'var(--mono)', fontSize: 9.5,
                  borderTop:'.5px solid var(--rule-2)',
                  borderLeft:'.5px solid var(--rule-2)' }}>
      <div style={{ borderRight:'.5px solid var(--rule-2)',
                    borderBottom:'.5px solid var(--rule-2)',
                    height: cellSize }}/>
      {syms.map(s => (
        <div key={s} style={{
          padding:'4px 0',
          fontFamily:'var(--sans)', fontSize: 9, fontWeight: 600,
          letterSpacing:'.06em', color:'var(--ink-1)', textAlign:'center',
          borderRight:'.5px solid var(--rule-2)',
          borderBottom:'.5px solid var(--rule-2)',
        }}>{s}</div>
      ))}
      {data.map((row, i) => (
        <React.Fragment key={i}>
          <div style={{
            padding:'4px 6px',
            fontFamily:'var(--sans)', fontSize: 9, fontWeight: 600,
            letterSpacing:'.06em', color:'var(--ink-1)', textAlign:'right',
            borderRight:'.5px solid var(--rule-2)',
            borderBottom:'.5px solid var(--rule-2)',
          }}>{syms[i]}</div>
          {row.map((v, j) => {
            const ink = `rgba(28, 26, 23, ${0.05 + v * 0.55})`;
            return (
              <div key={j} style={{
                background: ink, height: cellSize,
                display:'flex', alignItems:'center', justifyContent:'center',
                color: v > 0.55 ? 'var(--paper)' : 'var(--ink-1)',
                borderRight:'.5px solid var(--rule-2)',
                borderBottom:'.5px solid var(--rule-2)',
                fontWeight: i === j ? 600 : 400,
              }}>
                {v.toFixed(2)}
              </div>
            );
          })}
        </React.Fragment>
      ))}
    </div>
  );
}

function GreeksWidget(){
  const greeks = [
    { l:'Δ Delta',  v:'+0.42', net:'+$8,420',  bar: 0.62, tone:'up' },
    { l:'Γ Gamma',  v:'+0.018', net:'+$360',   bar: 0.32, tone:'up' },
    { l:'Θ Theta',  v:'−$1,840 / day', net:'',  bar: 0.28, tone:'dn' },
    { l:'V Vega',   v:'+$420 / 1%',  net:'',    bar: 0.18, tone:'up' },
    { l:'ρ Rho',    v:'+$60 / bp',  net:'',     bar: 0.08, tone:'up' },
  ];
  return (
    <table className="table dense" style={{ width: '100%' }}>
      <tbody>
        {greeks.map((g, i) => (
          <tr key={i}>
            <td className="name" style={{
              width: '32%',
              fontFamily:'var(--sans)', fontStyle:'normal',
              fontSize: 10, letterSpacing:'.10em', textTransform:'uppercase',
              color:'var(--ink-2)', fontWeight: 600,
            }}>
              {g.l}
            </td>
            <td className={g.tone === 'up' ? 'up' : 'dn'}
                style={{ fontFamily:'var(--mono)', fontSize: 12, fontWeight: 600 }}>
              {g.v}
            </td>
            <td style={{ width: '36%' }}>
              <span style={{ display:'inline-block', height: 6,
                              width: `${g.bar * 100}%`,
                              background: g.tone === 'up' ? 'var(--buy)' : 'var(--sell)' }}/>
            </td>
            <td className="r" style={{ fontFamily:'var(--mono)', fontSize: 11,
                                        color: 'var(--ink-2)' }}>
              {g.net || '—'}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function CalendarBox({ entries }){
  return (
    <div style={{ display:'flex', flexDirection:'column' }}>
      {entries.map((e, i) => (
        <div key={i} style={{
          display:'grid',
          gridTemplateColumns:'52px 38px 1fr auto',
          gap: 10, alignItems:'baseline',
          padding:'8px 0',
          borderBottom: i < entries.length - 1 ? '.5px dotted var(--rule-1)' : 'none',
        }}>
          <div style={{
            fontFamily:'var(--sans)', fontSize: 10, letterSpacing:'.10em',
            color:'var(--ink-2)', fontWeight: 600, textTransform:'uppercase',
          }}>{e.day}</div>
          <div style={{
            fontFamily:'var(--mono)', fontSize: 11, color:'var(--ink-1)',
          }}>{e.time}</div>
          <div>
            <div style={{
              fontFamily:'var(--serif)', fontSize: 13, fontWeight: 600,
              color:'var(--ink-0)', lineHeight: 1.25,
            }}>{e.event}</div>
            {e.note && (
              <div style={{
                fontFamily:'var(--serif)', fontStyle:'italic',
                fontSize: 11, color:'var(--ink-2)', marginTop: 2,
              }}>{e.note}</div>
            )}
          </div>
          {e.imp && (
            <span style={{
              fontFamily:'var(--sans)', fontSize: 9, letterSpacing:'.12em',
              textTransform:'uppercase', color: e.imp === 'high' ? 'var(--claret)' : 'var(--ink-3)',
              fontWeight: 600,
            }}>● {e.imp}</span>
          )}
        </div>
      ))}
    </div>
  );
}

function OptionsChain({ centerPx }){
  const strikes = useMemo(() => {
    const step = Math.max(1, Math.round(centerPx * 0.005));
    return Array.from({ length: 9 }, (_, i) => Math.round(centerPx / step) * step + (i - 4) * step);
  }, [centerPx]);
  const r = mulberry32(67);
  return (
    <table className="table dense" style={{ width:'100%' }}>
      <thead>
        <tr>
          <th className="r" colSpan={2}>Calls</th>
          <th className="c">Strike</th>
          <th colSpan={2}>Puts</th>
        </tr>
        <tr>
          <th className="r" style={{ fontSize: 8.5 }}>Bid</th>
          <th className="r" style={{ fontSize: 8.5 }}>Ask</th>
          <th className="c" style={{ fontSize: 8.5 }}>—</th>
          <th style={{ fontSize: 8.5 }}>Bid</th>
          <th style={{ fontSize: 8.5 }}>Ask</th>
        </tr>
      </thead>
      <tbody>
        {strikes.map((k, i) => {
          const itm = k <= centerPx;
          const cBid = Math.max(.02, (centerPx - k) + r() * 1.4 + 1.2);
          const cAsk = cBid + 0.06 + r() * 0.06;
          const pBid = Math.max(.02, (k - centerPx) + r() * 1.4 + 1.2);
          const pAsk = pBid + 0.06 + r() * 0.06;
          const isAtm = Math.abs(k - centerPx) === Math.min(...strikes.map(s => Math.abs(s - centerPx)));
          return (
            <tr key={k} className={isAtm ? 'on' : ''}>
              <td className="r" style={{
                background: itm ? 'color-mix(in oklab, var(--buy) 8%, transparent)' : null,
                color: itm ? 'var(--buy-ink)' : 'var(--ink-2)',
              }}>{cBid.toFixed(2)}</td>
              <td className="r" style={{
                background: itm ? 'color-mix(in oklab, var(--buy) 8%, transparent)' : null,
                color: itm ? 'var(--buy-ink)' : 'var(--ink-2)',
                fontWeight: itm ? 600 : 400,
              }}>{cAsk.toFixed(2)}</td>
              <td className="c sym" style={{ color: isAtm ? 'var(--claret)' : null }}>
                {k.toFixed(0)}
              </td>
              <td style={{
                background: !itm ? 'color-mix(in oklab, var(--sell) 8%, transparent)' : null,
                color: !itm ? 'var(--sell-ink)' : 'var(--ink-2)',
              }}>{pBid.toFixed(2)}</td>
              <td style={{
                background: !itm ? 'color-mix(in oklab, var(--sell) 8%, transparent)' : null,
                color: !itm ? 'var(--sell-ink)' : 'var(--ink-2)',
                fontWeight: !itm ? 600 : 400,
              }}>{pAsk.toFixed(2)}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

function RiskWidget(){
  const exposures = [
    { label:'Tech',           pct: 38, ch: +1.8 },
    { label:'Comm Services',  pct: 14, ch: +0.4 },
    { label:'Cons. Disc',     pct: 12, ch: -0.6 },
    { label:'Financials',     pct: 10, ch: +0.1 },
    { label:'Cash',           pct: 26, ch:  0.0 },
  ];
  return (
    <div>
      {/* Bar */}
      <div style={{
        display: 'flex', height: 18,
        border: '1px solid var(--rule-3)',
        marginBottom: 10,
      }}>
        {exposures.map((e, i) => (
          <span key={i} style={{
            flex: e.pct,
            background: i === exposures.length - 1
              ? 'var(--paper-2)'
              : `color-mix(in oklab, var(--ink-0) ${20 + i*15}%, transparent)`,
            borderRight: i < exposures.length - 1 ? '.5px solid var(--paper)' : 'none',
          }} title={`${e.label} · ${e.pct}%`}/>
        ))}
      </div>
      <table className="table dense" style={{ width: '100%' }}>
        <tbody>
          {exposures.map((e, i) => (
            <tr key={i}>
              <td style={{
                width: 12, padding: '4px 0',
                background: i === exposures.length - 1
                  ? 'var(--paper-2)'
                  : `color-mix(in oklab, var(--ink-0) ${20 + i*15}%, transparent)`,
              }}/>
              <td className="sector" style={{ fontFamily:'var(--serif)',
                                                fontStyle:'italic' }}>{e.label}</td>
              <td className="r" style={{ width: 60 }}>{e.pct}%</td>
              <td className={`r ${e.ch >= 0 ? 'up' : 'dn'}`} style={{ width: 60 }}>
                {e.ch === 0 ? '—' : fmtCh(e.ch)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function VarBox(){
  // Distribution chart for Value-at-Risk
  const points = useMemo(() => {
    return Array.from({ length: 60 }, (_, i) => {
      const x = (i - 30) / 8;
      return Math.exp(-(x * x) / 2);
    });
  }, []);
  const max = Math.max(...points);
  return (
    <div>
      <div style={{ display:'flex', gap:14, marginBottom: 10, alignItems:'baseline' }}>
        <div>
          <div className="bignum" style={{ fontSize: 30 }}>
            $84<span className="frac">,200</span>
          </div>
          <div style={{
            fontFamily:'var(--sans)', fontSize: 9.5, letterSpacing:'.12em',
            textTransform:'uppercase', color:'var(--ink-2)', marginTop: 2,
          }}>
            VaR · 95% · 1d
          </div>
        </div>
        <div style={{ borderLeft:'1px solid var(--rule-2)', paddingLeft: 14 }}>
          <div className="bignum" style={{ fontSize: 30, color:'var(--sell-ink)' }}>
            $148<span className="frac">,500</span>
          </div>
          <div style={{
            fontFamily:'var(--sans)', fontSize: 9.5, letterSpacing:'.12em',
            textTransform:'uppercase', color:'var(--ink-2)', marginTop: 2,
          }}>
            CVaR · tail
          </div>
        </div>
      </div>
      <svg width="100%" height="64" viewBox="0 0 240 64" preserveAspectRatio="none"
            style={{ display:'block' }}>
        {points.map((v, i) => {
          const h = (v / max) * 58;
          const tail = i < 6;
          return (
            <rect key={i} x={i * 4} y={64 - h} width={3} height={h}
                  fill={tail ? 'var(--sell)' : 'var(--ink-1)'}
                  opacity={tail ? 0.8 : 0.55}/>
          );
        })}
        <line x1={24} y1={0} x2={24} y2={64} stroke="var(--claret)" strokeWidth=".75"
              strokeDasharray="3 3"/>
      </svg>
      <div className="figcap" style={{ marginTop: 4 }}>
        P&amp;L distribution · 1-day horizon · 5,000 Monte Carlo paths
      </div>
    </div>
  );
}

function ScenarioWidget(){
  const moves = [
    { label:'SPY −3%',                pnl:'−$48,200', tone:'dn' },
    { label:'SPY +3%',                pnl:'+$52,800', tone:'up' },
    { label:'VIX +5pt',               pnl:'−$14,600', tone:'dn' },
    { label:'10Y +25bp',              pnl:'+$8,400',  tone:'up' },
    { label:'NVDA −5%',               pnl:'−$32,100', tone:'dn' },
    { label:'USD/JPY +2%',            pnl:'−$2,400',  tone:'dn' },
  ];
  return (
    <table className="table dense" style={{ width:'100%' }}>
      <tbody>
        {moves.map((m, i) => (
          <tr key={i}>
            <td className="sector" style={{ fontFamily:'var(--serif)', fontStyle:'italic' }}>{m.label}</td>
            <td className={`r ${m.tone === 'up' ? 'up-strong' : 'dn-strong'}`}
                style={{ fontWeight: 600 }}>{m.pnl}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

// ── Drawdown — underwater equity curve ──
function DrawdownChart({ width = 300, height = 110 }){
  // Synthetic drawdown series — peak-to-trough percent of high-water mark
  const data = useMemo(() => {
    const r = mulberry32(91);
    const n = 90;
    const eq = [];
    let v = 100;
    for (let i = 0; i < n; i++){
      v += (r() - 0.42) * 0.9;
      eq.push(v);
    }
    let peak = -Infinity;
    return eq.map(e => {
      peak = Math.max(peak, e);
      return -((peak - e) / peak) * 100;  // negative percentages
    });
  }, []);

  const min = Math.min(...data), max = 0;
  const padL = 6, padR = 38, padT = 6, padB = 16;
  const w = width - padL - padR, h = height - padT - padB;
  const x = (i) => padL + (i / (data.length - 1)) * w;
  const y = (v) => padT + ((max - v) / (max - min)) * h;

  let path = `M${x(0).toFixed(1)} ${y(0).toFixed(1)}`;
  for (let i = 0; i < data.length; i++) path += ` L${x(i).toFixed(1)} ${y(data[i]).toFixed(1)}`;
  path += ` L${x(data.length-1).toFixed(1)} ${(padT+h).toFixed(1)} L${padL.toFixed(1)} ${(padT+h).toFixed(1)} Z`;

  let line = `M${x(0).toFixed(1)} ${y(data[0]).toFixed(1)}`;
  for (let i = 1; i < data.length; i++) line += ` L${x(i).toFixed(1)} ${y(data[i]).toFixed(1)}`;

  const worst = Math.min(...data);
  const worstIdx = data.indexOf(worst);

  return (
    <svg className="ed-chart" viewBox={`0 0 ${width} ${height}`}
          preserveAspectRatio="xMidYMid meet"
          style={{ display:'block', width:'100%', height:'auto' }}>
      {/* zero line */}
      <line className="grid-line" x1={padL} x2={padL + w} y1={y(0)} y2={y(0)}/>
      <text className="axis-text" x={padL + w + 4} y={y(0) + 3.5}>0%</text>

      {/* −5% reference */}
      {min < -5 && (
        <>
          <line className="grid-line" x1={padL} x2={padL + w} y1={y(-5)} y2={y(-5)}
                strokeDasharray="3 3"/>
          <text className="axis-text" x={padL + w + 4} y={y(-5) + 3.5}>−5%</text>
        </>
      )}

      {/* Underwater fill */}
      <path d={path} fill="var(--sell)" opacity=".15"/>
      <path d={line} fill="none" stroke="var(--sell)" strokeWidth="1.25"/>

      {/* Worst trough marker */}
      <circle cx={x(worstIdx)} cy={y(worst)} r="3"
              fill="var(--sell)" stroke="var(--paper)" strokeWidth="1"/>
      <text className="annotation-text"
            x={x(worstIdx) + 5} y={y(worst) + 4}
            style={{ fill:'var(--sell-ink)' }}>
        {worst.toFixed(1)}% · trough
      </text>
    </svg>
  );
}

// ── Benchmark race — cumulative return curves ──
function BenchmarkChart({ width = 320, height = 130 }){
  const data = useMemo(() => {
    const r = mulberry32(101);
    const n = 90;
    const series = [
      { label:'Book',  color:'var(--ink-0)',  stroke: 1.5, dash: null,    drift: 0.18 },
      { label:'SPY',   color:'var(--ink-2)',  stroke: 1.0, dash: null,    drift: 0.06 },
      { label:'QQQ',   color:'var(--ink-2)',  stroke: 1.0, dash: '4 3',   drift: 0.10 },
      { label:'60/40', color:'var(--ink-3)',  stroke: 1.0, dash: '2 2',   drift: 0.03 },
    ];
    return series.map(s => {
      let v = 0;
      const arr = [v];
      for (let i = 1; i < n; i++){
        v += s.drift / n + (r() - 0.48) * 0.5;
        arr.push(v);
      }
      return { ...s, data: arr };
    });
  }, []);

  const all = data.flatMap(s => s.data);
  const lo = Math.min(...all), hi = Math.max(...all);
  const span = hi - lo || 1;
  const padL = 6, padR = 60, padT = 8, padB = 18;
  const w = width - padL - padR, h = height - padT - padB;
  const x = (i, n) => padL + (i / (n - 1)) * w;
  const y = (v) => padT + (1 - (v - lo) / span) * h;

  return (
    <svg className="ed-chart" viewBox={`0 0 ${width} ${height}`}
          preserveAspectRatio="xMidYMid meet"
          style={{ display:'block', width:'100%', height:'auto' }}>
      <line className="grid-line" x1={padL} x2={padL + w} y1={y(0)} y2={y(0)}/>
      <text className="axis-text" x={padL + w + 4} y={y(0) + 3.5}>0%</text>
      {data.map((s, si) => {
        let p = `M${x(0, s.data.length).toFixed(1)} ${y(s.data[0]).toFixed(1)}`;
        for (let i = 1; i < s.data.length; i++) p += ` L${x(i, s.data.length).toFixed(1)} ${y(s.data[i]).toFixed(1)}`;
        const last = s.data[s.data.length - 1];
        return (
          <g key={si}>
            <path d={p} fill="none" stroke={s.color} strokeWidth={s.stroke}
                  strokeDasharray={s.dash || undefined}/>
            <text className="axis-text"
                  x={padL + w + 4} y={y(last) + 3.5}
                  style={{ fill: s.color, fontWeight: si === 0 ? 700 : 500 }}>
              {s.label} {last >= 0 ? '+' : '−'}{Math.abs(last).toFixed(1)}%
            </text>
          </g>
        );
      })}
    </svg>
  );
}

// ── Factor tilts ──
function FactorTilts(){
  // Tilts are -1 (short) to +1 (long), centred at 0
  const factors = [
    { name:'Size',       tilt: +0.42, note:'Large-cap bias' },
    { name:'Value',      tilt: -0.18, note:'Growth tilt' },
    { name:'Momentum',   tilt: +0.62, note:'Strong long-mom' },
    { name:'Quality',    tilt: +0.34, note:'Profitable book' },
    { name:'Low vol',    tilt: -0.22, note:'Above-mkt vol' },
    { name:'Yield',      tilt: -0.41, note:'Underweight div' },
    { name:'Beta',       tilt: +0.28, note:'Slightly cyclical' },
    { name:'Crowding',   tilt: +0.55, note:'High-conviction' },
  ];

  return (
    <table className="table dense" style={{ width:'100%' }}>
      <tbody>
        {factors.map((f, i) => {
          const pct = Math.abs(f.tilt) * 50;  // half-width
          const isLong = f.tilt >= 0;
          return (
            <tr key={i}>
              <td className="sector" style={{
                fontFamily:'var(--serif)', fontStyle:'italic', width: '28%',
              }}>{f.name}</td>
              <td style={{ width:'48%' }}>
                <div style={{
                  position:'relative', height: 12,
                  borderLeft: '.5px solid var(--rule-2)',
                  borderRight: '.5px solid var(--rule-2)',
                }}>
                  {/* Centre tick */}
                  <span style={{
                    position:'absolute', left: '50%', top: 0, bottom: 0,
                    width: 1, background:'var(--ink-0)',
                  }}/>
                  {/* Bar */}
                  <span style={{
                    position:'absolute', top: 1, bottom: 1,
                    left: isLong ? '50%' : `${50 - pct}%`,
                    width: `${pct}%`,
                    background: isLong ? 'var(--ink-0)' : 'var(--claret)',
                    opacity: .8,
                  }}/>
                </div>
              </td>
              <td className="r" style={{
                fontFamily:'var(--mono)', fontSize: 11,
                color: isLong ? 'var(--ink-0)' : 'var(--claret)',
                fontWeight: 600, width: '12%',
              }}>
                {f.tilt >= 0 ? '+' : '−'}{Math.abs(f.tilt).toFixed(2)}
              </td>
              <td style={{
                fontFamily:'var(--serif)', fontStyle:'italic',
                fontSize: 11, color:'var(--ink-2)', width: '12%',
              }}>{f.note}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

// ── Term structure of volatility ──
function TermStructure({ width = 280, height = 120 }){
  // Two curves: today vs. one-week-ago — showing kink in front
  const tenors  = ['1W','2W','1M','2M','3M','6M','1Y'];
  const today   = [27.4, 25.8, 24.6, 23.9, 23.4, 22.8, 22.1];
  const lastWk  = [22.8, 23.2, 23.6, 23.8, 23.6, 23.0, 22.3];
  const all = [...today, ...lastWk];
  const lo = Math.min(...all) - 1, hi = Math.max(...all) + 1;
  const padL = 18, padR = 36, padT = 10, padB = 22;
  const w = width - padL - padR, h = height - padT - padB;
  const x = (i) => padL + (i / (tenors.length - 1)) * w;
  const y = (v) => padT + (1 - (v - lo) / (hi - lo)) * h;
  const path = (d) => {
    let p = `M${x(0).toFixed(1)} ${y(d[0]).toFixed(1)}`;
    for (let i = 1; i < d.length; i++) p += ` L${x(i).toFixed(1)} ${y(d[i]).toFixed(1)}`;
    return p;
  };

  return (
    <svg className="ed-chart" viewBox={`0 0 ${width} ${height}`}
          preserveAspectRatio="xMidYMid meet"
          style={{ display:'block', width:'100%', height:'auto' }}>
      {/* Y-axis ticks at 22, 24, 26, 28 */}
      {[22, 24, 26, 28].map(v => (
        <g key={v}>
          <line className="grid-line" x1={padL} x2={padL + w} y1={y(v)} y2={y(v)}/>
          <text className="axis-text" x={padL - 4} y={y(v) + 3.5} textAnchor="end">{v}</text>
        </g>
      ))}
      {/* Last week */}
      <path d={path(lastWk)} fill="none" stroke="var(--ink-3)" strokeWidth="1"
            strokeDasharray="3 3"/>
      {lastWk.map((v, i) => (
        <circle key={i} cx={x(i)} cy={y(v)} r="1.5" fill="var(--ink-3)"/>
      ))}
      {/* Today */}
      <path d={path(today)} fill="none" stroke="var(--ink-0)" strokeWidth="1.5"/>
      {today.map((v, i) => (
        <circle key={i} cx={x(i)} cy={y(v)} r="2" fill="var(--ink-0)"/>
      ))}
      {/* Annotate front-month kink */}
      <line className="annotation" x1={x(0)} x2={x(0)} y1={padT} y2={padT + h}/>
      <text className="annotation-text" x={x(0) + 4} y={padT + 11}>FRONT · KINK</text>

      {/* X-axis */}
      {tenors.map((t, i) => (
        <text key={t} className="axis-text" x={x(i)} y={height - 8} textAnchor="middle">{t}</text>
      ))}

      {/* Labels */}
      <text className="axis-text" x={padL + w + 4} y={y(today[tenors.length-1]) + 3.5}
            style={{ fill:'var(--ink-0)', fontWeight: 700 }}>today</text>
      <text className="axis-text" x={padL + w + 4} y={y(lastWk[tenors.length-1]) + 3.5}
            style={{ fill:'var(--ink-3)' }}>1w ago</text>
    </svg>
  );
}

function ScreenWidgets({ activeSym }){
  const sym = SYMS.find(s => s.sym === activeSym) || SYMS[2];

  return (
    <div className="page-inner">
      {/* Section masthead */}
      <div style={{
        display:'flex', alignItems:'baseline', justifyContent:'space-between',
        borderBottom:'3px double var(--ink-0)', paddingBottom: 8, marginTop: 14,
      }}>
        <div>
          <div className="eyebrow">Section IV · The Almanac</div>
          <h1 className="headline h1" style={{ fontSize: 36 }}>
            Widgets &amp; Reference
          </h1>
        </div>
        <div style={{
          fontFamily:'var(--serif)', fontStyle:'italic',
          fontSize: 13, color:'var(--ink-2)', textAlign:'right',
          maxWidth: 280, lineHeight: 1.4,
        }}>
          A working trader's almanac — volatility, risk, calendars, and the
          options chain — kept in one place for ready reference.
        </div>
      </div>

      {/* Top row · three columns */}
      <div className="grid-12" style={{ marginTop: 22 }}>

        {/* Volatility surface */}
        <div className="col-4">
          <div className="sec-head">
            <span className="ttl">Volatility surface</span>
            <span className="sub">{sym.sym} · % · live</span>
          </div>
          <div className="eyebrow">Figure I · Implied vol by strike &amp; tenor</div>
          <VolSurface width={300} height={140}/>
          <div className="figcap" style={{ borderTop:'.5px solid var(--rule-1)', paddingTop: 4 }}>
            Rows: tenor (front → back). Columns: strike (−3σ → +3σ).
            Darker cells indicate richer vol.
          </div>
          <div className="dl-stats" style={{ marginTop: 12 }}>
            <div className="pair">
              <dt>ATM 30d</dt>
              <dd>24.6<span className="pct">%</span></dd>
            </div>
            <div className="pair">
              <dt>Skew</dt>
              <dd>+3.8<span className="pct">pp</span></dd>
            </div>
          </div>
        </div>

        {/* Greeks */}
        <div className="col-4 vrule">
          <div className="sec-head">
            <span className="ttl">Greeks &amp; exposure</span>
            <span className="sub">Net book · live</span>
          </div>
          <p style={{
            fontFamily:'var(--serif)', fontSize: 13, fontStyle:'italic',
            color:'var(--ink-2)', margin:'0 0 10px', lineHeight: 1.4,
          }}>
            First- and second-order sensitivities, marked to the desk's current
            book. Bars proportional to absolute exposure.
          </p>
          <GreeksWidget/>
          <div className="figcap" style={{ borderTop:'.5px solid var(--rule-1)',
                                            paddingTop: 4, marginTop: 8 }}>
            Figure II — Greeks for the open position in {sym.sym} and overlays.
          </div>
        </div>

        {/* Risk / VaR */}
        <div className="col-4 vrule">
          <div className="sec-head">
            <span className="ttl">Value at Risk</span>
            <span className="sub">95% · daily</span>
          </div>
          <VarBox/>
        </div>
      </div>

      {/* Performance band — drawdown, benchmark race, term structure */}
      <div className="grid-12 hrule-thick" style={{ marginTop: 24 }}>
        <div className="col-4">
          <div className="sec-head">
            <span className="ttl">Drawdown · 90 sessions</span>
            <span className="sub">Underwater curve</span>
          </div>
          <DrawdownChart width={320} height={120}/>
          <div className="figcap" style={{ borderTop:'.5px solid var(--rule-1)',
                                            paddingTop: 4, marginTop: 6 }}>
            Figure IV — Drawdown from running high-water mark.
            Trough of −4.1% reached 9 April; book has since recovered to flat.
          </div>
        </div>

        <div className="col-4 vrule">
          <div className="sec-head">
            <span className="ttl">Book vs. benchmarks</span>
            <span className="sub">90-day cumulative</span>
          </div>
          <BenchmarkChart width={340} height={130}/>
          <div className="figcap" style={{ borderTop:'.5px solid var(--rule-1)',
                                            paddingTop: 4, marginTop: 6 }}>
            Figure V — Cumulative returns. Solid: this book. Dashed: index proxies.
            Excess +6.4pp over SPY.
          </div>
        </div>

        <div className="col-4 vrule">
          <div className="sec-head">
            <span className="ttl">Vol term structure</span>
            <span className="sub">{sym.sym} · today vs. 1w</span>
          </div>
          <TermStructure width={300} height={120}/>
          <div className="figcap" style={{ borderTop:'.5px solid var(--rule-1)',
                                            paddingTop: 4, marginTop: 6 }}>
            Figure VI — Front-month kink steepened ~4.6 pts this week, reflecting
            pre-CPI demand for protection.
          </div>
        </div>
      </div>

      {/* Factor tilts — full-width */}
      <div className="hrule-thick" style={{ marginTop: 24, paddingTop: 14 }}>
        <div className="sec-head">
          <span className="ttl">Factor tilts · book exposure</span>
          <span className="sub">Beta-adjusted · 30-session window</span>
        </div>
        <p style={{
          fontFamily:'var(--serif)', fontSize: 13, fontStyle:'italic',
          color:'var(--ink-2)', margin: '0 0 10px', lineHeight: 1.4,
          maxWidth: 720,
        }}>
          The desk's book reads <em>large-cap, high-momentum, high-quality, low-yield</em> —
          a recognisable growth profile, with a meaningful crowding tilt that warrants
          monitoring into earnings season.
        </p>
        <FactorTilts/>
        <div className="figcap" style={{ borderTop:'.5px solid var(--rule-1)',
                                          paddingTop: 4, marginTop: 6 }}>
          Figure VII — Standardised factor loadings (−1, +1).
          Bars to the right indicate long exposure; claret bars indicate short.
        </div>
      </div>

      {/* Middle row · two columns */}
      <div className="grid-12 hrule-thick" style={{ marginTop: 24 }}>

        {/* Correlation matrix + scenarios */}
        <div className="col-6">
          <div className="sec-head">
            <span className="ttl">Correlation · 30 sessions</span>
            <span className="sub">Pearson · daily returns</span>
          </div>
          <div className="eyebrow">Figure III · Pairwise correlation</div>
          <CorrelationGrid size={7}/>
          <div className="figcap" style={{ borderTop:'.5px solid var(--rule-1)',
                                            paddingTop: 4, marginTop: 6 }}>
            Read down the rows; symmetric matrix.
            Values nearer to 1.00 are darker.
          </div>

          <div className="hrule" style={{ marginTop: 18 }}>
            <div className="sec-head">
              <span className="ttl">Scenario · P&amp;L</span>
              <span className="sub">Pre-computed shocks</span>
            </div>
            <ScenarioWidget/>
          </div>
        </div>

        {/* Options chain */}
        <div className="col-6 vrule">
          <div className="sec-head">
            <span className="ttl">Options chain · {sym.sym}</span>
            <span className="sub">17 May · 1w</span>
          </div>
          <p style={{
            fontFamily:'var(--serif)', fontSize: 13, fontStyle:'italic',
            color:'var(--ink-2)', margin: '0 0 10px', lineHeight: 1.4,
          }}>
            In-the-money rows shaded in the desk's editorial green (calls) and
            red (puts); the at-the-money row is highlighted in claret.
          </p>
          <OptionsChain centerPx={sym.px}/>
          <div className="figcap" style={{ borderTop:'.5px solid var(--rule-1)',
                                            paddingTop: 4, marginTop: 6 }}>
            Five strikes either side of spot. Bid / ask quoted live.
          </div>
        </div>
      </div>

      {/* Bottom row — Risk exposure + Calendars */}
      <div className="grid-12 hrule-thick" style={{ marginTop: 24 }}>

        {/* Risk exposure */}
        <div className="col-4">
          <div className="sec-head">
            <span className="ttl">Sector exposure</span>
            <span className="sub">Net book · today</span>
          </div>
          <RiskWidget/>
          <div className="figcap" style={{ marginTop: 6 }}>
            Long net exposure broken down by GICS sector; cash held in reserve.
          </div>
        </div>

        {/* Economic calendar */}
        <div className="col-4 vrule">
          <div className="sec-head">
            <span className="ttl">Economic calendar</span>
            <span className="sub">This week</span>
          </div>
          <CalendarBox entries={[
            { day:'Mon', time:'14:00', event:'US Industrial Production', note:'consensus +0.2%', imp:'low' },
            { day:'Tue', time:'08:30', event:'PPI · Final demand', note:'Y/Y est. 1.6%', imp:'med' },
            { day:'Wed', time:'08:30', event:'CPI · April', note:'Core est. 0.3% M/M', imp:'high' },
            { day:'Thu', time:'08:30', event:'Retail Sales', note:'consensus +0.1%', imp:'med' },
            { day:'Fri', time:'10:00', event:'UMich Sentiment', note:'preliminary read', imp:'low' },
          ]}/>
        </div>

        {/* Earnings calendar */}
        <div className="col-4 vrule">
          <div className="sec-head">
            <span className="ttl">Earnings · this week</span>
            <span className="sub">5 of 14 followed</span>
          </div>
          <CalendarBox entries={[
            { day:'Tue', time:'AMC', event:'Home Depot · HD', note:'EPS est. $3.62', imp:'med' },
            { day:'Tue', time:'AMC', event:'Palo Alto · PANW', note:'EPS est. $1.78', imp:'med' },
            { day:'Wed', time:'AMC', event:'NVIDIA · NVDA', note:'EPS est. $5.62 · IV 8.1%', imp:'high' },
            { day:'Thu', time:'BMO', event:'Walmart · WMT', note:'EPS est. $2.03', imp:'med' },
            { day:'Thu', time:'AMC', event:'Snowflake · SNOW', note:'EPS est. $0.16', imp:'low' },
          ]}/>
        </div>
      </div>

      {/* Pull quote / house view */}
      <div className="pullquote" style={{ marginTop: 28 }}>
        "The almanac is for the things you reach for once a week and remember
         once a year. We keep them all here so the desk doesn't have to."
        <span className="attrib">House view · Trading desk · May 2026</span>
      </div>
    </div>
  );
}

window.ScreenWidgets = ScreenWidgets;
