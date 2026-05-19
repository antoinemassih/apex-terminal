// Floating + tiled widgets — bold editorial style inspired by reference imagery
const { useState: wS, useEffect: wE } = React;

// ---------- AM/PM clock card (orange/black) ----------
function ClockCard({ time='9:42', period='AM', bg='var(--accent)', fg='#0a0a08' }) {
  return (
    <div style={{
      background: bg, color: fg,
      padding: '16px 20px',
      borderRadius: 22,
      display: 'flex', alignItems: 'center', gap: 14,
      minWidth: 170,
      boxShadow: '0 8px 28px rgba(20,20,15,.18)',
    }}>
      <div style={{
        fontFamily:'JetBrains Mono', fontSize:11, lineHeight:1.1, fontWeight:600,
        textAlign:'right', opacity: 0.7,
      }}>
        <div style={{color: period==='AM'?fg:'currentColor'}}>AM</div>
        <div style={{color: period==='PM'?fg:'currentColor'}}>PM</div>
      </div>
      <div style={{
        fontFamily:'Inter Tight', fontSize: 42, fontWeight: 500,
        letterSpacing:'-0.04em', lineHeight:.9,
      }}>{time}</div>
    </div>
  );
}

// ---------- Big stat pill (Koncert-style) ----------
function StatPill({ label, sub, value, bg='var(--accent)', fg='#0a0a08', size='lg' }) {
  return (
    <div style={{
      background: bg, color: fg,
      borderRadius: 22,
      padding: '16px 18px',
      display:'flex', flexDirection:'column', justifyContent:'space-between',
      minHeight: size==='lg' ? 200 : 140,
      boxShadow: '0 8px 28px rgba(20,20,15,.12)',
    }}>
      <div>
        <div style={{fontFamily:'Inter Tight', fontSize:13, fontWeight:600, letterSpacing:'-0.005em'}}>{label}</div>
        {sub && <div style={{fontFamily:'Inter Tight', fontSize:11, opacity:.75, marginTop:2}}>{sub}</div>}
      </div>
      <div style={{
        fontFamily:'Inter Tight', fontWeight: 500,
        fontSize: size==='lg' ? 64 : 44,
        letterSpacing:'-0.04em', lineHeight:.9,
      }}>{value}</div>
    </div>
  );
}

// ---------- Pill button shapes (Readymag style) ----------
function PillRow({ items }) {
  return (
    <div style={{display:'flex', flexWrap:'wrap', gap:8}}>
      {items.map((it,i) => (
        <div key={i} style={{
          padding: '10px 18px',
          borderRadius: 999,
          fontFamily: 'Inter Tight',
          fontSize: 14, fontWeight: 500,
          background: it.fill ? it.color : 'transparent',
          border: it.fill ? 'none' : `1.5px solid ${it.color}`,
          color: it.fill ? '#0a0a08' : it.color,
          letterSpacing: '-0.005em',
          display:'inline-flex', alignItems:'center', gap:6,
        }}>{it.label}</div>
      ))}
    </div>
  );
}

// ---------- Polar / radial gauge (Olympic-style) ----------
function PolarGauge({ values=[], colors=['var(--accent)','var(--ink)'], size=180, label, sub }) {
  const cx = size/2, cy = size/2;
  const rOuter = size*0.46, rInner = size*0.18;
  const max = Math.max(...values, 1);
  const N = values.length;
  return (
    <div style={{position:'relative', width:size, height:size+30}}>
      <svg viewBox={`0 0 ${size} ${size}`} width={size} height={size}>
        {/* background ring */}
        <circle cx={cx} cy={cy} r={rOuter} fill="none" stroke="var(--hair)" strokeWidth="1"/>
        <circle cx={cx} cy={cy} r={rInner} fill="none" stroke="var(--hair)" strokeWidth="1"/>
        {values.map((v,i) => {
          const t0 = -Math.PI/2 + (i/N)*Math.PI*2;
          const t1 = -Math.PI/2 + ((i+1)/N)*Math.PI*2 - 0.01;
          const r  = rInner + (rOuter-rInner) * (v/max);
          const x0 = cx + Math.cos(t0)*rInner, y0 = cy + Math.sin(t0)*rInner;
          const x1 = cx + Math.cos(t0)*r, y1 = cy + Math.sin(t0)*r;
          const x2 = cx + Math.cos(t1)*r, y2 = cy + Math.sin(t1)*r;
          const x3 = cx + Math.cos(t1)*rInner, y3 = cy + Math.sin(t1)*rInner;
          const big = (t1-t0) > Math.PI ? 1 : 0;
          const d = `M ${x0} ${y0} L ${x1} ${y1} A ${r} ${r} 0 ${big} 1 ${x2} ${y2} L ${x3} ${y3} A ${rInner} ${rInner} 0 ${big} 0 ${x0} ${y0} Z`;
          const c = colors[i % colors.length];
          return <path key={i} d={d} fill={c} opacity={0.55 + 0.45*(v/max)}/>;
        })}
        {/* tick ring */}
        {Array.from({length: 60}).map((_,i) => {
          const t = -Math.PI/2 + (i/60)*Math.PI*2;
          const r0 = rOuter+2, r1 = rOuter+ (i%5===0 ? 7 : 4);
          return <line key={'t'+i}
            x1={cx+Math.cos(t)*r0} y1={cy+Math.sin(t)*r0}
            x2={cx+Math.cos(t)*r1} y2={cy+Math.sin(t)*r1}
            stroke="var(--ink)" strokeWidth={i%5===0?1:0.5} opacity="0.5"/>;
        })}
      </svg>
      {label && (
        <div style={{position:'absolute', inset:0, display:'grid', placeItems:'center', textAlign:'center', pointerEvents:'none'}}>
          <div>
            <div style={{fontFamily:'Inter Tight', fontSize:24, fontWeight:500, letterSpacing:'-0.03em'}}>{label}</div>
            {sub && <div style={{fontFamily:'JetBrains Mono', fontSize:9, color:'var(--muted)', letterSpacing:'.12em', textTransform:'uppercase', marginTop:2}}>{sub}</div>}
          </div>
        </div>
      )}
    </div>
  );
}

// ---------- Petal sentiment (8-segment, like the emotion ref) ----------
function PetalChart({ data, size=200 }) {
  const cx = size/2, cy = size/2, R = size*0.46;
  const N = data.length;
  return (
    <svg viewBox={`0 0 ${size} ${size}`} width={size} height={size}>
      {data.map((d,i) => {
        const t0 = -Math.PI/2 + (i/N)*Math.PI*2;
        const t1 = -Math.PI/2 + ((i+1)/N)*Math.PI*2;
        const tm = (t0+t1)/2;
        const tip = {x: cx+Math.cos(tm)*R, y: cy+Math.sin(tm)*R};
        const a = {x: cx+Math.cos(t0)*R*0.4, y: cy+Math.sin(t0)*R*0.4};
        const b = {x: cx+Math.cos(t1)*R*0.4, y: cy+Math.sin(t1)*R*0.4};
        const opacity = 0.25 + 0.7 * (d.v / 12);
        const lx = cx + Math.cos(tm)*R*0.7, ly = cy + Math.sin(tm)*R*0.7;
        return (
          <g key={i}>
            <path d={`M ${a.x},${a.y} Q ${tip.x},${tip.y} ${b.x},${b.y} Z`}
                  fill={d.color || 'var(--accent)'} opacity={opacity}/>
            <text x={lx} y={ly-2} textAnchor="middle"
                  fontFamily="Inter Tight" fontWeight="500" fontSize="14" fill="var(--ink)">{d.v}</text>
            <text x={lx} y={ly+10} textAnchor="middle"
                  fontFamily="Inter Tight" fontSize="9" fill="var(--ink)" opacity="0.7">{d.label}</text>
          </g>
        );
      })}
    </svg>
  );
}

// ---------- Bullet bar (saving-goal style) ----------
function BulletRow({ label, pct, color='var(--ink)' }) {
  return (
    <div style={{display:'grid', gridTemplateColumns:'120px 1fr auto', alignItems:'center', gap:14, marginBottom:8}}>
      <div style={{fontFamily:'Inter Tight', fontSize:12}}>{label}</div>
      <div style={{height:6, background:'rgba(20,20,15,.1)', borderRadius:99, overflow:'hidden'}}>
        <div style={{height:'100%', width:`${pct}%`, background:color, borderRadius:99}}/>
      </div>
      <div className="mono" style={{fontSize:11}}>{pct}%</div>
    </div>
  );
}

// ---------- Dot-matrix bar (Frekvens style) ----------
function DotBars({ values, cols=20, rows=8, size=6 }) {
  const max = Math.max(...values);
  const out = [];
  for (let i=0;i<cols;i++){
    const v = values[Math.floor(i * (values.length-1)/(cols-1))];
    const lvl = Math.round((v/max)*(rows-1));
    for (let r=0;r<rows;r++){
      const on = (rows-1-r) <= lvl;
      out.push(<circle key={i+'_'+r}
        cx={i*size+size/2} cy={r*size+size/2} r={size*0.32}
        fill={on?'currentColor':'rgba(255,255,255,.18)'}/>);
    }
  }
  return <svg viewBox={`0 0 ${cols*size} ${rows*size}`} width="100%" height={rows*size+4}>{out}</svg>;
}

Object.assign(window, { ClockCard, StatPill, PillRow, PolarGauge, PetalChart, BulletRow, DotBars });
