// Chart components — candlestick + indicators, drawn with SVG
const { useMemo, useState, useEffect, useRef } = React;

function Candles({ candles, width=900, height=420, showVol=true, showMA=true, showBB=false }) {
  const padL = 56, padR = 64, padT = 14, padB = showVol ? 80 : 24;
  const plotH = height - padT - padB;
  const plotW = width - padL - padR;

  const lows  = candles.map(c => c.l);
  const highs = candles.map(c => c.h);
  let yMin = Math.min(...lows), yMax = Math.max(...highs);
  const pad = (yMax - yMin) * 0.06;
  yMin -= pad; yMax += pad;

  const x = i => padL + (i + 0.5) * (plotW / candles.length);
  const y = v => padT + (1 - (v - yMin) / (yMax - yMin)) * plotH;
  const cw = Math.max(2, plotW / candles.length * 0.66);

  // MA(20)
  const ma20 = useMemo(() => candles.map((_,i) => {
    const s = Math.max(0, i-19);
    const sl = candles.slice(s, i+1);
    return sl.reduce((a,b)=>a+b.c,0)/sl.length;
  }), [candles]);

  // MA(50)
  const ma50 = useMemo(() => candles.map((_,i) => {
    const s = Math.max(0, i-49);
    const sl = candles.slice(s, i+1);
    return sl.reduce((a,b)=>a+b.c,0)/sl.length;
  }), [candles]);

  // BB(20, 2σ)
  const bb = useMemo(() => candles.map((_,i) => {
    const s = Math.max(0, i-19); const sl = candles.slice(s, i+1);
    const mu = sl.reduce((a,b)=>a+b.c,0)/sl.length;
    const sd = Math.sqrt(sl.reduce((a,b)=>a+(b.c-mu)*(b.c-mu),0)/sl.length);
    return {u: mu+2*sd, l: mu-2*sd};
  }), [candles]);

  // y-axis ticks
  const ticks = 6;
  const tickVals = Array.from({length:ticks+1}, (_,i)=> yMin + (yMax-yMin)*i/ticks);

  // x-axis ticks (~6)
  const xTickIdx = Array.from({length:7}, (_,i)=> Math.round(i*(candles.length-1)/6));

  // Volume scale
  const volMax = Math.max(...candles.map(c=>c.v));
  const volH = 56;
  const volY = padT + plotH + 12;

  // Crosshair
  const [cross, setCross] = useState(null);
  const svgRef = useRef(null);
  function onMove(e){
    const rect = svgRef.current.getBoundingClientRect();
    const px = (e.clientX - rect.left) * (width / rect.width);
    const py = (e.clientY - rect.top) * (height / rect.height);
    if (px<padL||px>padL+plotW||py<padT||py>padT+plotH){ setCross(null); return; }
    const idx = Math.min(candles.length-1, Math.max(0, Math.floor((px - padL) / (plotW/candles.length))));
    setCross({ idx, py });
  }

  const last = candles[candles.length-1];
  const lastY = y(last.c);
  const lastUp = last.c >= last.o;

  return (
    <svg ref={svgRef} viewBox={`0 0 ${width} ${height}`}
         preserveAspectRatio="none"
         style={{width:'100%', height:'100%', display:'block', cursor:'crosshair'}}
         onMouseMove={onMove} onMouseLeave={()=>setCross(null)}>
      {/* grid */}
      {tickVals.map((v,i) => (
        <g key={'g'+i}>
          <line x1={padL} x2={width-padR} y1={y(v)} y2={y(v)}
                stroke="var(--hair-2)" strokeDasharray="2 4"/>
          <text x={width-padR+8} y={y(v)+3} fontSize="10"
                fontFamily="JetBrains Mono" fill="var(--muted)">
            {v.toFixed(2)}
          </text>
        </g>
      ))}
      {xTickIdx.map((i,k) => (
        <text key={'xt'+k} x={x(i)} y={padT+plotH+14}
              fontSize="9" fontFamily="JetBrains Mono"
              fill="var(--muted)" textAnchor="middle"
              letterSpacing="1">
          {`T-${candles.length-1-i}`}
        </text>
      ))}

      {/* BB */}
      {showBB && (
        <g>
          <path d={
            'M' + bb.map((b,i)=>`${x(i)},${y(b.u)}`).join(' L ')
            + ' L ' + bb.map((b,i)=>`${x(candles.length-1-i)},${y(bb[candles.length-1-i].l)}`).join(' L ')
            + ' Z'
          } fill="color-mix(in srgb, var(--accent) 8%, transparent)" stroke="none"/>
          <path d={'M ' + bb.map((b,i)=>`${x(i)},${y(b.u)}`).join(' L ')}
                fill="none" stroke="var(--accent)" strokeWidth="0.6" opacity="0.8"/>
          <path d={'M ' + bb.map((b,i)=>`${x(i)},${y(b.l)}`).join(' L ')}
                fill="none" stroke="var(--accent)" strokeWidth="0.6" opacity="0.8"/>
        </g>
      )}

      {/* MA lines */}
      {showMA && (
        <g>
          <path d={'M ' + ma20.map((v,i)=>`${x(i)},${y(v)}`).join(' L ')}
                fill="none" stroke="var(--ink)" strokeWidth="1.2" opacity="0.65"/>
          <path d={'M ' + ma50.map((v,i)=>`${x(i)},${y(v)}`).join(' L ')}
                fill="none" stroke="var(--accent)" strokeWidth="1.2" opacity="0.85"
                strokeDasharray="3 3"/>
        </g>
      )}

      {/* candles */}
      {candles.map((c,i) => {
        const up = c.c >= c.o;
        const fill = up ? 'var(--up)' : 'var(--down)';
        const cx = x(i);
        return (
          <g key={i}>
            <line x1={cx} x2={cx} y1={y(c.h)} y2={y(c.l)} stroke={fill} strokeWidth="1"/>
            <rect
              x={cx - cw/2}
              y={Math.min(y(c.o), y(c.c))}
              width={cw}
              height={Math.max(1, Math.abs(y(c.o) - y(c.c)))}
              fill={fill} stroke={fill}
            />
          </g>
        );
      })}

      {/* volume */}
      {showVol && (
        <g>
          <line x1={padL} x2={width-padR} y1={volY+volH} y2={volY+volH}
                stroke="var(--hair)"/>
          {candles.map((c,i) => {
            const up = c.c >= c.o;
            const h = (c.v / volMax) * volH;
            return (
              <rect key={'v'+i}
                    x={x(i) - cw/2} width={cw}
                    y={volY + volH - h} height={h}
                    fill={up ? 'var(--up)' : 'var(--down)'} opacity="0.55"/>
            );
          })}
          <text x={padL} y={volY-2} fontSize="9" fontFamily="JetBrains Mono"
                fill="var(--muted)" letterSpacing="1">VOL</text>
        </g>
      )}

      {/* last price line */}
      <g>
        <line x1={padL} x2={width-padR} y1={lastY} y2={lastY}
              stroke="var(--accent)" strokeWidth="1" strokeDasharray="3 3"/>
        <rect x={width-padR+2} y={lastY-9} width={56} height={18}
              fill="var(--accent)"/>
        <text x={width-padR+8} y={lastY+4} fontSize="11" fontFamily="JetBrains Mono"
              fill="white" fontWeight="600">{last.c.toFixed(2)}</text>
      </g>

      {/* crosshair */}
      {cross && (
        <g pointerEvents="none">
          <line x1={x(cross.idx)} x2={x(cross.idx)} y1={padT} y2={padT+plotH}
                stroke="var(--ink)" strokeDasharray="2 3" opacity="0.5"/>
          <line x1={padL} x2={width-padR} y1={cross.py} y2={cross.py}
                stroke="var(--ink)" strokeDasharray="2 3" opacity="0.5"/>
          <rect x={x(cross.idx)-30} y={padT+plotH+22} width={60} height={14}
                fill="var(--ink)"/>
          <text x={x(cross.idx)} y={padT+plotH+32} textAnchor="middle"
                fontSize="9" fontFamily="JetBrains Mono" fill="var(--bg)">
            T-{candles.length-1-cross.idx}
          </text>
        </g>
      )}
    </svg>
  );
}

// Sparkline
function Spark({ values, w=80, h=22, up }) {
  const min = Math.min(...values), max = Math.max(...values);
  const x = i => (i/(values.length-1)) * w;
  const y = v => h - ((v-min)/(max-min || 1))*h;
  const d = 'M ' + values.map((v,i)=>`${x(i)},${y(v)}`).join(' L ');
  return (
    <svg viewBox={`0 0 ${w} ${h}`} width={w} height={h} preserveAspectRatio="none">
      <path d={d} fill="none"
            stroke={up ? 'var(--up)' : 'var(--down)'} strokeWidth="1"/>
    </svg>
  );
}

// Mini-chart for hero
function HeroSpark({ candles, w=600, h=160 }) {
  const close = candles.map(c=>c.c);
  const min = Math.min(...close), max = Math.max(...close);
  const x = i => (i/(close.length-1)) * w;
  const y = v => h - ((v-min)/(max-min || 1))*h*0.85 - h*0.05;
  const d = 'M ' + close.map((v,i)=>`${x(i)},${y(v)}`).join(' L ');
  const area = d + ` L ${w},${h} L 0,${h} Z`;
  const up = close[close.length-1] >= close[0];
  return (
    <svg viewBox={`0 0 ${w} ${h}`} width="100%" height="100%" preserveAspectRatio="none">
      <path d={area} fill={up?'color-mix(in srgb, var(--up) 14%, transparent)':'color-mix(in srgb, var(--down) 14%, transparent)'} />
      <path d={d} fill="none" stroke={up?'var(--up)':'var(--down)'} strokeWidth="1.4"/>
    </svg>
  );
}

// Dot-matrix sparkline (editorial look)
function DotChart({ values, w=160, h=40 }) {
  const cols = 40, rows = 10;
  const min = Math.min(...values), max = Math.max(...values);
  const cw = w/cols, ch = h/rows;
  const sample = i => values[Math.floor(i*(values.length-1)/(cols-1))];
  const out = [];
  for (let i=0;i<cols;i++){
    const v = sample(i);
    const lvl = Math.round(((v-min)/(max-min||1))*(rows-1));
    for (let r=0;r<rows;r++){
      const on = (rows-1-r) <= lvl;
      out.push(<circle key={i+'-'+r} cx={i*cw+cw/2} cy={r*ch+ch/2} r={1.1}
        fill={on ? 'var(--ink)' : 'var(--hair)'}/>);
    }
  }
  return <svg viewBox={`0 0 ${w} ${h}`} width="100%" height={h}>{out}</svg>;
}

Object.assign(window, { Candles, Spark, HeroSpark, DotChart });
