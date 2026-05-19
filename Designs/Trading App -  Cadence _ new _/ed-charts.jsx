// ed-charts.jsx — Editorial-style charts: thin line, dotted grid, claret annotation
// Quieter than the trading candle chart — designed to be read like a newspaper figure.

function EditorialLine({ data, width = 400, height = 140, padding = { t:8, r:30, b:18, l:8 },
                        annotation = null, lastLabel = true, fill = true }){
  const w = width - padding.l - padding.r;
  const h = height - padding.t - padding.b;
  if (!data?.length) return null;

  const min = Math.min(...data), max = Math.max(...data);
  const span = (max - min) || 1;
  const pad = span * 0.08;
  const lo = min - pad, hi = max + pad;
  const x = (i) => padding.l + (i / (data.length - 1)) * w;
  const y = (v) => padding.t + (1 - (v - lo) / (hi - lo)) * h;

  let path = `M${x(0).toFixed(1)} ${y(data[0]).toFixed(1)}`;
  for (let i = 1; i < data.length; i++) path += ` L${x(i).toFixed(1)} ${y(data[i]).toFixed(1)}`;
  const fillPath = `${path} L${x(data.length-1).toFixed(1)} ${(padding.t+h).toFixed(1)} L${x(0).toFixed(1)} ${(padding.t+h).toFixed(1)} Z`;

  const ticks = 4;
  const yTicks = Array.from({ length: ticks }, (_, i) => {
    const v = lo + (hi - lo) * (i / (ticks - 1));
    return { v, y: y(v) };
  });

  const last = data[data.length-1];
  return (
    <svg className="ed-chart" width={width} height={height}>
      {yTicks.map((t, i) => (
        <g key={i}>
          <line className="grid-line" x1={padding.l} x2={padding.l + w} y1={t.y} y2={t.y}/>
          <text className="axis-text" x={padding.l + w + 4} y={t.y + 3.5}>{t.v.toFixed(0)}</text>
        </g>
      ))}
      {fill && <path className="price-fill" d={fillPath}/>}
      <path className="price-line" d={path}/>
      {annotation && (
        <g>
          <line className="annotation" x1={x(annotation.idx)} x2={x(annotation.idx)}
                y1={padding.t} y2={padding.t + h}/>
          <text className="annotation-text" x={x(annotation.idx) - 2} y={padding.t + 10}
                textAnchor="end">{annotation.label}</text>
        </g>
      )}
      {lastLabel && (
        <g>
          <circle className="last-dot" cx={x(data.length-1)} cy={y(last)} r="2.5"/>
          <text className="axis-text" x={padding.l + w + 4} y={y(last) + 3.5}
                style={{ fontWeight:600, fill:'var(--claret)' }}>{last.toFixed(2)}</text>
        </g>
      )}
    </svg>
  );
}

function EditorialCandle({ candles, width = 700, height = 280,
                          padding = { t:12, r:54, b:24, l:8 } }){
  const w = width - padding.l - padding.r;
  const h = height - padding.t - padding.b;
  const volH = Math.floor(h * 0.16);
  const priceH = h - volH - 8;

  const min = Math.min(...candles.map(c => c.l));
  const max = Math.max(...candles.map(c => c.h));
  const span = (max - min) || 1;
  const pad = span * 0.05;
  const lo = min - pad, hi = max + pad;
  const yToPx = (v) => padding.t + (1 - (v - lo) / (hi - lo)) * priceH;
  const cw = w / candles.length;
  const bw = Math.max(1.2, cw * 0.5);

  const vMax = Math.max(...candles.map(c => c.v));
  const vY = (v) => padding.t + priceH + 8 + (1 - v / vMax) * volH;

  const ticks = 6;
  const grid = Array.from({ length: ticks }, (_, i) => {
    const v = lo + (hi - lo) * (i / (ticks - 1));
    return { y: yToPx(v), label: v.toFixed(0) };
  });

  const xTicks = 6;
  const xLabels = Array.from({ length: xTicks }, (_, i) => {
    const idx = Math.floor((candles.length - 1) * (i / (xTicks - 1)));
    return { x: padding.l + idx * cw + cw/2,
             label: `${String(9 + Math.floor(idx*6/candles.length)).padStart(2,'0')}:${String((idx*45)%60).padStart(2,'0')}` };
  });

  const last = candles[candles.length - 1];
  return (
    <svg className="ed-chart ed-candle" width={width} height={height}>
      {grid.map((g, i) => (
        <g key={i}>
          <line className="grid-line" x1={padding.l} x2={padding.l + w} y1={g.y} y2={g.y}/>
          <text className="axis-text" x={padding.l + w + 4} y={g.y + 3.5}>{g.label}</text>
        </g>
      ))}
      {candles.map((c, i) => {
        const cx = padding.l + i * cw + cw/2;
        const up = c.c >= c.o;
        const top = yToPx(Math.max(c.o, c.c));
        const bot = yToPx(Math.min(c.o, c.c));
        return (
          <g key={i} className={up ? 'up' : 'dn'}>
            <line className="wick" x1={cx} x2={cx} y1={yToPx(c.h)} y2={yToPx(c.l)}/>
            <rect x={cx - bw/2} y={top} width={bw} height={Math.max(.5, bot - top)}/>
            <rect x={cx - bw/2} y={vY(c.v)} width={bw}
                  height={padding.t + priceH + 8 + volH - vY(c.v)}
                  fill={up ? 'var(--buy)' : 'var(--sell)'} opacity=".35"/>
          </g>
        );
      })}
      {/* Last price line + claret marker */}
      <line className="annotation" x1={padding.l} x2={padding.l + w}
            y1={yToPx(last.c)} y2={yToPx(last.c)}/>
      <text className="axis-text" x={padding.l + w + 4} y={yToPx(last.c) + 3.5}
            style={{ fontWeight:600, fill:'var(--claret)' }}>{last.c.toFixed(2)}</text>
      {xLabels.map((t, i) => (
        <text key={i} className="axis-text" x={t.x} y={height - 6} textAnchor="middle">{t.label}</text>
      ))}
    </svg>
  );
}

function EditorialSpark({ data, w = 90, h = 22, fill = false }){
  if (!data?.length) return null;
  const min = Math.min(...data), max = Math.max(...data);
  const span = (max - min) || 1;
  const x = (i) => (i / (data.length - 1)) * (w - 2) + 1;
  const y = (v) => h - 1 - ((v - min) / span) * (h - 2);
  let path = `M${x(0).toFixed(1)} ${y(data[0]).toFixed(1)}`;
  for (let i = 1; i < data.length; i++) path += ` L${x(i).toFixed(1)} ${y(data[i]).toFixed(1)}`;
  const up = data[data.length-1] >= data[0];
  const color = up ? 'var(--buy)' : 'var(--sell)';
  return (
    <svg width={w} height={h} style={{ display:'block' }}>
      {fill && (
        <path d={`${path} L${x(data.length-1).toFixed(1)} ${h} L${x(0).toFixed(1)} ${h} Z`}
              fill={color} opacity=".12"/>
      )}
      <path d={path} fill="none" stroke={color} strokeWidth="1"/>
    </svg>
  );
}

window.EditorialLine = EditorialLine;
window.EditorialCandle = EditorialCandle;
window.EditorialSpark = EditorialSpark;
