// Refined infographic widgets — burst diagrams, concentric rings, mosaics, petals
// All draw with SVG. Editorial / data-poster vibe.

// ============================================================
// 1) BurstChart — Olympic-poster style radial bar burst
//    Each ray is a year/datapoint; ray length = value.
// ============================================================
function BurstChart({ data, size=280, max,
                      fillBars='var(--accent)', fillTrack='rgba(20,20,15,.08)',
                      label, sub, showYears=true, startYear=1990 }){
  const cx = size/2, cy = size/2;
  const rIn = size*0.18, rOut = size*0.46;
  const N = data.length;
  const M = max ?? Math.max(...data) * 1.05;
  const gap = 0.6; // degrees between bars
  return (
    <svg viewBox={`0 0 ${size} ${size}`} width="100%" height="100%" preserveAspectRatio="xMidYMid meet">
      <circle cx={cx} cy={cy} r={rIn-2} fill="var(--paper)" stroke="var(--rule)" strokeWidth="0.5"/>
      {/* axis ring */}
      {[0.25, 0.5, 0.75, 1].map(t => (
        <circle key={t} cx={cx} cy={cy} r={rIn + (rOut-rIn)*t}
                fill="none" stroke="var(--hair)" strokeWidth="0.5" strokeDasharray="1 3"/>
      ))}
      {data.map((v, i) => {
        const a0 = -Math.PI/2 + (i/N)*Math.PI*2 + (gap*Math.PI/180)/2;
        const a1 = -Math.PI/2 + ((i+1)/N)*Math.PI*2 - (gap*Math.PI/180)/2;
        // background track
        const trackPath = ringSegment(cx, cy, rIn, rOut, a0, a1);
        const ratio = Math.max(0, Math.min(1, v/M));
        const r = rIn + (rOut-rIn)*ratio;
        const barPath = ringSegment(cx, cy, rIn, r, a0, a1);
        return (
          <g key={i}>
            <path d={trackPath} fill={fillTrack}/>
            <path d={barPath} fill={fillBars}/>
          </g>
        );
      })}
      {showYears && data.map((_, i) => {
        if (i % 4 !== 0) return null;
        const a = -Math.PI/2 + ((i+0.5)/N)*Math.PI*2;
        const r = rOut + 12;
        const x = cx + Math.cos(a)*r, y = cy + Math.sin(a)*r;
        const rotDeg = (a*180/Math.PI) + 90;
        const flip = (rotDeg > 90 && rotDeg < 270);
        return (
          <text key={'y'+i} x={x} y={y}
                fontFamily="JetBrains Mono" fontSize="9" fontWeight="500"
                fill="var(--ink)" opacity="0.55"
                textAnchor="middle"
                transform={`rotate(${flip?rotDeg+180:rotDeg} ${x} ${y})`}>
            {startYear + i}
          </text>
        );
      })}
      {label && (
        <text x={cx} y={cy-2} textAnchor="middle"
              fontFamily="Inter Tight" fontSize={size*0.11} fontWeight="500"
              letterSpacing="-0.04em" fill="var(--ink)">{label}</text>
      )}
      {sub && (
        <text x={cx} y={cy + size*0.06} textAnchor="middle"
              fontFamily="JetBrains Mono" fontSize={size*0.035}
              fill="var(--muted)" letterSpacing="0.12em">{sub}</text>
      )}
    </svg>
  );
}

function ringSegment(cx, cy, rIn, rOut, a0, a1){
  const big = (a1 - a0) > Math.PI ? 1 : 0;
  const x0 = cx + Math.cos(a0)*rIn, y0 = cy + Math.sin(a0)*rIn;
  const x1 = cx + Math.cos(a0)*rOut, y1 = cy + Math.sin(a0)*rOut;
  const x2 = cx + Math.cos(a1)*rOut, y2 = cy + Math.sin(a1)*rOut;
  const x3 = cx + Math.cos(a1)*rIn, y3 = cy + Math.sin(a1)*rIn;
  return `M ${x0} ${y0} L ${x1} ${y1} A ${rOut} ${rOut} 0 ${big} 1 ${x2} ${y2} L ${x3} ${y3} A ${rIn} ${rIn} 0 ${big} 0 ${x0} ${y0} Z`;
}

// ============================================================
// 2) ConcentricRings — layered ring infographic (sector exposure / risk)
//    Each ring is a category; segments fill clockwise from 12.
// ============================================================
function ConcentricRings({ rings, size=260, gap=4, palette }){
  const cx = size/2, cy = size/2;
  const outer = size*0.46;
  const inner = size*0.16;
  const ringCount = rings.length;
  const w = (outer - inner)/ringCount - gap;
  const cols = palette || ['var(--accent)', 'var(--ink)', 'var(--up)', 'var(--down)', 'var(--muted)'];
  return (
    <svg viewBox={`0 0 ${size} ${size}`} width="100%" height="100%" preserveAspectRatio="xMidYMid meet">
      {rings.map((ring, i) => {
        const r0 = outer - (i+1)*(w+gap) + gap;
        const r1 = outer - i*(w+gap);
        const segs = [];
        let acc = 0;
        const total = ring.values.reduce((a,b)=>a+b,0) || 1;
        ring.values.forEach((v, j) => {
          const a0 = -Math.PI/2 + (acc/total)*Math.PI*2;
          const a1 = -Math.PI/2 + ((acc+v)/total)*Math.PI*2 - 0.005;
          acc += v;
          if (v <= 0) return;
          const c = (ring.colors && ring.colors[j]) || cols[j%cols.length];
          segs.push(<path key={i+'_'+j} d={ringSegment(cx, cy, r0, r1, a0, a1)} fill={c}/>);
        });
        return <g key={i}>
          <circle cx={cx} cy={cy} r={(r0+r1)/2} fill="none" stroke="var(--hair)" strokeWidth="0.5"/>
          {segs}
          {ring.label && (
            <text x={cx + 4} y={cy - (r0+r1)/2 + 3}
                  fontFamily="JetBrains Mono" fontSize="8" letterSpacing="0.1em"
                  fill="var(--ink)" opacity="0.55">{ring.label}</text>
          )}
        </g>;
      })}
      <circle cx={cx} cy={cy} r={inner-2} fill="var(--paper)" stroke="var(--rule)" strokeWidth="0.5"/>
    </svg>
  );
}

// ============================================================
// 3) MosaicGrid — checker-style retention mosaic (left side of ref)
// ============================================================
function MosaicGrid({ rows=10, cols=10, density=0.55, accentEvery=12, size=200,
                     fg='var(--ink)', accent='var(--accent)' }){
  const cells = [];
  let idx = 0;
  for (let r=0;r<rows;r++){
    for (let c=0;c<cols;c++){
      // pseudo-random but stable
      const v = Math.abs(Math.sin(r*9.13 + c*4.27))*1000 % 1;
      const filled = v > (1 - density);
      const isAcc = (idx % accentEvery === 0) && filled;
      cells.push({r, c, filled, isAcc});
      idx++;
    }
  }
  const cw = size/cols, rh = size/rows;
  return (
    <svg viewBox={`0 0 ${size} ${size}`} width="100%" height="100%" preserveAspectRatio="xMidYMid meet">
      {cells.map((c,i) => (
        <rect key={i}
          x={c.c*cw+0.5} y={c.r*rh+0.5}
          width={cw-1} height={rh-1}
          fill={c.filled ? (c.isAcc ? accent : fg) : 'transparent'}
          stroke="var(--hair)" strokeWidth="0.5"/>
      ))}
    </svg>
  );
}

// ============================================================
// 4) WaveBars — dot-matrix waveform / equalizer bar set
// ============================================================
function WaveBars({ values=[], cols=24, rows=10, dotSize=5, fg, off='rgba(20,20,15,.08)' }){
  const max = Math.max(...values, 1);
  const out = [];
  for (let i=0;i<cols;i++){
    const v = values[Math.floor(i * (values.length-1)/(cols-1))] || 0;
    const lvl = Math.round((v/max)*(rows-1));
    for (let r=0;r<rows;r++){
      const on = (rows-1-r) <= lvl;
      const alpha = on ? 1 : 1;
      out.push(<circle key={i+'_'+r}
        cx={i*dotSize+dotSize/2} cy={r*dotSize+dotSize/2} r={dotSize*0.32}
        fill={on ? (fg||'var(--ink)') : off}/>);
    }
  }
  return (
    <svg viewBox={`0 0 ${cols*dotSize} ${rows*dotSize}`}
         width="100%" preserveAspectRatio="none"
         style={{display:'block', height: rows*dotSize}}>
      {out}
    </svg>
  );
}

// ============================================================
// 5) FlowerSentiment — 8-petal sentiment chart (Apple-emotion ref)
// ============================================================
function FlowerSentiment({ data, size=240, palette=['var(--accent)','var(--ink)'] }){
  const cx = size/2, cy = size/2;
  const R = size*0.46;
  const N = data.length;
  return (
    <svg viewBox={`0 0 ${size} ${size}`} width="100%" height="100%" preserveAspectRatio="xMidYMid meet">
      <circle cx={cx} cy={cy} r={size*0.06} fill="var(--paper)" stroke="var(--rule)" strokeWidth="0.5"/>
      {data.map((d, i) => {
        const tm = -Math.PI/2 + ((i+0.5)/N)*Math.PI*2;
        const t0 = tm - Math.PI/N + 0.06;
        const t1 = tm + Math.PI/N - 0.06;
        const tip = {x: cx+Math.cos(tm)*R, y: cy+Math.sin(tm)*R};
        const innerR = size*0.08;
        const a = {x: cx+Math.cos(t0)*innerR, y: cy+Math.sin(t0)*innerR};
        const b = {x: cx+Math.cos(t1)*innerR, y: cy+Math.sin(t1)*innerR};
        const opacity = 0.18 + 0.7 * (d.v / Math.max(...data.map(x=>x.v)));
        const color = d.color || palette[0];
        const lx = cx + Math.cos(tm)*R*0.66, ly = cy + Math.sin(tm)*R*0.66;
        return (
          <g key={i}>
            <path d={`M ${a.x},${a.y} Q ${tip.x},${tip.y} ${b.x},${b.y} Z`}
                  fill={color} opacity={opacity}/>
            <text x={lx} y={ly+1} textAnchor="middle"
                  fontFamily="Inter Tight" fontWeight="500" fontSize={size*0.06}
                  fill={d.dark ? 'var(--paper)' : 'var(--ink)'}
                  letterSpacing="-0.02em">{d.v}</text>
            <text x={lx} y={ly+size*0.055} textAnchor="middle"
                  fontFamily="Inter Tight" fontSize={size*0.034}
                  fill={d.dark ? 'rgba(241,237,225,.6)' : 'var(--muted)'}>{d.label}</text>
          </g>
        );
      })}
    </svg>
  );
}

// ============================================================
// 6) StackedBars — vertical micro stacked bar (treemap alt)
// ============================================================
function StackedBars({ data, height=60 }){
  const total = data.reduce((s,d)=>s+d.v, 0);
  let acc = 0;
  return (
    <div style={{display:'flex', height, gap:1, borderRadius:6, overflow:'hidden'}}>
      {data.map((d,i) => {
        const w = `${(d.v/total)*100}%`;
        const el = <div key={i} style={{
          flex: `0 0 ${w}`, background: d.color, color: d.fg||'#0a0a08',
          padding:'4px 6px', display:'flex', flexDirection:'column', justifyContent:'space-between'
        }}>
          <div style={{fontFamily:'JetBrains Mono', fontSize:9, opacity:.7}}>{d.label}</div>
          <div style={{fontFamily:'Inter Tight', fontSize:14, fontWeight:600, letterSpacing:'-0.02em'}}>{d.v}%</div>
        </div>;
        return el;
      })}
    </div>
  );
}

// ============================================================
// 7) RadialBars — half-circle composite gauge
// ============================================================
function RadialBars({ items, size=240 }){
  const cx = size/2, cy = size*0.62;
  const items2 = items.slice(0, 6);
  const N = items2.length;
  const baseR = size*0.20;
  const step = size*0.04;
  return (
    <svg viewBox={`0 0 ${size} ${size*0.7}`} width="100%" height="100%" preserveAspectRatio="xMidYMid meet">
      {items2.map((it,i) => {
        const r = baseR + i*step;
        const t = Math.max(0, Math.min(1, it.v));
        const a0 = Math.PI;
        const a1 = Math.PI + Math.PI * t;
        const x0 = cx + Math.cos(a0)*r, y0 = cy + Math.sin(a0)*r;
        const x1 = cx + Math.cos(a1)*r, y1 = cy + Math.sin(a1)*r;
        const big = (a1-a0) > Math.PI ? 1 : 0;
        return (
          <g key={i}>
            <path d={`M ${cx-r} ${cy} A ${r} ${r} 0 0 1 ${cx+r} ${cy}`}
                  fill="none" stroke="var(--hair)" strokeWidth="2"/>
            <path d={`M ${x0} ${y0} A ${r} ${r} 0 ${big} 1 ${x1} ${y1}`}
                  fill="none" stroke={it.color || 'var(--accent)'} strokeWidth="6" strokeLinecap="square"/>
            <text x={cx-r-6} y={cy-1} textAnchor="end"
                  fontFamily="JetBrains Mono" fontSize="9" letterSpacing=".08em"
                  fill="var(--muted)">{it.label}</text>
            <text x={cx+r+6} y={cy-1}
                  fontFamily="JetBrains Mono" fontSize="9" letterSpacing=".08em"
                  fill="var(--ink)" fontWeight="600">{Math.round(t*100)}</text>
          </g>
        );
      })}
    </svg>
  );
}

// ============================================================
// 8) FlowDots — beeswarm/scatter dots in a circle
// ============================================================
function FlowDots({ size=220, count=80, accent='var(--accent)' }){
  const cx = size/2, cy = size/2, R = size*0.45;
  const dots = [];
  for (let i=0;i<count;i++){
    const t = (i*7919) % 360 * Math.PI/180;
    const r = R * Math.sqrt(Math.abs(Math.sin(i*1.3)) * 0.95);
    const x = cx + Math.cos(t)*r, y = cy + Math.sin(t)*r;
    const isAcc = i%9===0;
    dots.push(<circle key={i} cx={x} cy={y} r={isAcc?3:1.6}
                      fill={isAcc?accent:'var(--ink)'} opacity={isAcc?1:0.5}/>);
  }
  return <svg viewBox={`0 0 ${size} ${size}`} width="100%" height="100%">{dots}</svg>;
}

// ============================================================
// 9) Sparkline (refined)
// ============================================================
function Sparkline({ values, width=120, height=36, color='var(--ink)', fill }){
  if (!values.length) return null;
  const min = Math.min(...values), max = Math.max(...values);
  const r = max - min || 1;
  const pts = values.map((v,i) => `${(i/(values.length-1))*width},${height - ((v-min)/r)*height*0.85 - 2}`);
  const path = 'M ' + pts.join(' L ');
  const area = path + ` L ${width},${height} L 0,${height} Z`;
  return (
    <svg viewBox={`0 0 ${width} ${height}`} width="100%" height={height} preserveAspectRatio="none">
      {fill && <path d={area} fill={fill} opacity={0.18}/>}
      <path d={path} fill="none" stroke={color} strokeWidth="1.2"/>
    </svg>
  );
}

// ============================================================
// 10) MarketBands — yield curve / IV term structure
// ============================================================
function MarketBands({ tenors, values, size=280, height=120 }){
  const padL=28, padR=12, padT=12, padB=22;
  const W = size - padL - padR, H = height - padT - padB;
  const max = Math.max(...values), min = Math.min(...values);
  const r = max-min || 1;
  const pts = values.map((v,i) => ({
    x: padL + (i/(values.length-1))*W,
    y: padT + H - ((v-min)/r)*H,
  }));
  const path = 'M ' + pts.map(p=>`${p.x},${p.y}`).join(' L ');
  return (
    <svg viewBox={`0 0 ${size} ${height}`} width="100%" height={height}>
      {[0,0.25,0.5,0.75,1].map(t=>(
        <line key={t} x1={padL} x2={size-padR} y1={padT+H*t} y2={padT+H*t}
              stroke="var(--hair)" strokeWidth="0.5" strokeDasharray="1 3"/>
      ))}
      <path d={path + ` L ${size-padR},${height-padB} L ${padL},${height-padB} Z`}
            fill="var(--accent)" opacity="0.18"/>
      <path d={path} fill="none" stroke="var(--accent)" strokeWidth="1.5"/>
      {pts.map((p,i)=>(
        <circle key={i} cx={p.x} cy={p.y} r="2.5" fill="var(--ink)"/>
      ))}
      {tenors.map((t,i)=>(
        <text key={i} x={pts[i].x} y={height-6} textAnchor="middle"
              fontFamily="JetBrains Mono" fontSize="9" fill="var(--muted)" letterSpacing=".08em">
          {t}
        </text>
      ))}
      {values.map((v,i)=>(
        <text key={'v'+i} x={pts[i].x} y={pts[i].y-6} textAnchor="middle"
              fontFamily="JetBrains Mono" fontSize="9" fill="var(--ink)" fontWeight="600">
          {v.toFixed(2)}
        </text>
      ))}
    </svg>
  );
}

Object.assign(window, {
  BurstChart, ConcentricRings, MosaicGrid, WaveBars,
  FlowerSentiment, StackedBars, RadialBars, FlowDots,
  Sparkline, MarketBands, ringSegment,
});
