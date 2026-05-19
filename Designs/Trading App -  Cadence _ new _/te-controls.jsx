// te-controls.jsx — TE-style tactile controls: rotary Knob, KeyCap button row
// Designed to be dropped into any panel as a physical-feeling control.

function TEKnob({ size = 64, min = 0, max = 100, value, defaultValue = 50,
                  onChange, label = 'DIAL', sublabel = null, color = 'amber',
                  ticks = 11, snap = false }){
  const [internal, setInternal] = useState(defaultValue);
  const v = value !== undefined ? value : internal;
  const set = (nv) => {
    const clamped = Math.max(min, Math.min(max, nv));
    if (onChange) onChange(clamped);
    if (value === undefined) setInternal(clamped);
  };

  // Map value → rotation, sweeping 270° (-135 to +135)
  const range = max - min;
  const pct = (v - min) / range;
  const rot = -135 + pct * 270;

  const dragRef = useRef(null);
  const startRef = useRef({ y: 0, v: 0 });

  const onPointerDown = (e) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    startRef.current = { y: e.clientY, v: v };
  };
  const onPointerMove = (e) => {
    if (!e.currentTarget.hasPointerCapture(e.pointerId)) return;
    const dy = startRef.current.y - e.clientY;   // up = increase
    const sensitivity = range / 120;
    let nv = startRef.current.v + dy * sensitivity;
    if (snap) nv = Math.round(nv);
    set(nv);
  };
  const onWheel = (e) => {
    e.preventDefault();
    const step = range / 50;
    set(v - Math.sign(e.deltaY) * step);
  };

  const indicatorTicks = Array.from({ length: ticks }, (_, i) => {
    const a = -135 + (i / (ticks - 1)) * 270;
    const active = (i / (ticks - 1)) <= pct;
    return { a, active };
  });

  const accentVar = color === 'amber' ? 'var(--accent)' :
                    color === 'buy'   ? 'var(--buy)'    :
                    color === 'sell'  ? 'var(--sell)'   : 'var(--accent)';

  return (
    <div className="teknob-wrap" style={{ width: size + 24 }}>
      <div className="teknob-ringwrap" style={{ width: size + 14, height: size + 14 }}>
        {/* Tick ring */}
        <svg className="teknob-ticks" width={size + 14} height={size + 14}
             viewBox={`0 0 ${size + 14} ${size + 14}`}>
          {indicatorTicks.map((t, i) => {
            const cx = (size + 14) / 2;
            const r1 = size / 2 + 3;
            const r2 = size / 2 + 6.5;
            const rad = (t.a - 90) * Math.PI / 180;
            const x1 = cx + Math.cos(rad) * r1, y1 = cx + Math.sin(rad) * r1;
            const x2 = cx + Math.cos(rad) * r2, y2 = cx + Math.sin(rad) * r2;
            return (
              <line key={i} x1={x1} y1={y1} x2={x2} y2={y2}
                    stroke={t.active ? accentVar : 'var(--line-3)'}
                    strokeWidth={t.active ? 1.5 : 1}
                    strokeLinecap="round"
                    style={{ filter: t.active ? `drop-shadow(0 0 2px ${accentVar})` : 'none' }}/>
            );
          })}
        </svg>
        {/* Knob body */}
        <div ref={dragRef}
             className="teknob-body"
             style={{ width: size, height: size, transform: `rotate(${rot}deg)` }}
             onPointerDown={onPointerDown}
             onPointerMove={onPointerMove}
             onPointerUp={(e) => e.currentTarget.releasePointerCapture(e.pointerId)}
             onWheel={onWheel}>
          <div className="teknob-knurl"/>
          <span className="teknob-pointer" style={{ background: accentVar, boxShadow: `0 0 6px ${accentVar}` }}/>
          <span className="teknob-dot"/>
        </div>
      </div>
      <div className="teknob-label">
        <span className="teknob-num">{snap ? Math.round(v) : v.toFixed(1)}</span>
        <span className="teknob-name">{label}</span>
        {sublabel && <span className="teknob-sub">{sublabel}</span>}
      </div>
    </div>
  );
}

// KeyCap — small keycap-style button with monospace label + tiny index chip
function KeyCap({ on = false, children, sublabel, idx, onClick, w = 38, h = 30 }){
  return (
    <button className={`keycap ${on ? 'on' : ''}`} onClick={onClick}
            style={{ width: w, height: h }}>
      {idx && <span className="keycap-idx">{idx}</span>}
      <span className="keycap-main">{children}</span>
      {sublabel && <span className="keycap-sub">{sublabel}</span>}
    </button>
  );
}

// FieldLabel — TE-style "01 LIMIT" label with serial chip
function FieldLabel({ idx, children, hint = null }){
  return (
    <div className="te-field-label">
      <span className="te-field-idx">{idx}</span>
      <span className="te-field-name">{children}</span>
      {hint && <span className="te-field-hint">{hint}</span>}
    </div>
  );
}

window.TEKnob = TEKnob;
window.KeyCap = KeyCap;
window.FieldLabel = FieldLabel;
