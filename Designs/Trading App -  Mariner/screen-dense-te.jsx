// screen-dense-te.jsx — Chart Desk · TE refinement
// Same layout, refined chrome plus tactile TE controls (rotary knob,
// keycap MA toggles, indexed FieldLabels on order ticket).

function TEOrderTicket({ sym, refPrice }) {
  const [side, setSide] = useState('BUY');
  const [type, setType] = useState('LMT');
  const [qty, setQty] = useState(100);
  const [px, setPx] = useState(refPrice);
  useEffect(() => setPx(refPrice), [refPrice]);

  return (
    <div style={{ padding: '10px 12px 12px' }}>
      <div className="ph" style={{ margin: '-10px -12px 10px', padding: '7px 12px' }}>
        <span className="fr-label"><span className="num">A1</span> Order · {sym}</span>
        <span className="te-live" style={{ fontSize: 9 }}>ARMED</span>
      </div>

      {/* BUY / SELL */}
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 6, marginBottom: 10 }}>
        <KeyCap on={side === 'BUY'} idx="01" sublabel="LONG" onClick={() => setSide('BUY')} w="auto" h={42}>BUY</KeyCap>
        <KeyCap on={side === 'SELL'} idx="02" sublabel="SHORT" onClick={() => setSide('SELL')} w="auto" h={42}>SELL</KeyCap>
      </div>

      {/* Order type row */}
      <div style={{ marginBottom: 8 }}>
        <FieldLabel idx="03" hint="ORDER TYPE">Type</FieldLabel>
      </div>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr 1fr', gap: 4, marginBottom: 12 }}>
        {['MKT', 'LMT', 'STP', 'STL'].map((t) =>
        <KeyCap key={t} on={type === t} onClick={() => setType(t)} w="auto" h={26}>{t}</KeyCap>
        )}
      </div>

      {/* Knob + Price */}
      <div style={{ display: 'grid', gridTemplateColumns: '88px 1fr', gap: 10, alignItems: 'center', marginBottom: 10 }}>
        <TEKnob size={62} min={0} max={500} value={qty} onChange={setQty}
        snap label="QTY" sublabel="SHARES" color="amber" />
        <div>
          <FieldLabel idx="04" hint="LIMIT">Price</FieldLabel>
          <div className="well" style={{ marginTop: 4, padding: '6px 8px',
            fontFamily: 'var(--mono)', fontSize: 18, color: 'var(--ink-1)',
            letterSpacing: '.02em', display: 'flex', alignItems: 'baseline', gap: 6 }}>
            <span style={{ color: 'var(--ink-4)', fontSize: 11 }}>$</span>
            <span style={{ fontVariantNumeric: 'tabular-nums' }}>{px.toFixed(2)}</span>
          </div>
          <div className="stp" style={{ marginTop: 4, display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 3 }}>
            <button className="stp-b" onClick={() => setPx((p) => p - 0.05)}>−.05</button>
            <button className="stp-b" onClick={() => setPx((p) => p - 0.01)}>−.01</button>
            <button className="stp-b" onClick={() => setPx((p) => p + 0.01)}>+.01</button>
          </div>
        </div>
      </div>

      {/* Notional pip row */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center',
        fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--ink-3)',
        letterSpacing: '.12em', textTransform: 'uppercase',
        padding: '8px 0', borderTop: '1px solid var(--line-3)',
        borderBottom: '1px solid var(--line-3)', marginBottom: 10 }}>
        <span>NOTIONAL</span>
        <span style={{ color: 'var(--ink-1)', fontSize: 13 }}>
          ${(qty * px).toLocaleString(undefined, { maximumFractionDigits: 0 })}
        </span>
      </div>

      {/* Submit */}
      <button style={{ width: '100%', padding: '10px 12px', borderRadius: 2,
        background: side === 'BUY' ?
        'linear-gradient(180deg, #2a3b25 0%, #1c2b18 100%)' :
        'linear-gradient(180deg, #3b2528 0%, #2b181a 100%)',
        border: '1px solid #0a0703',
        boxShadow: 'inset 0 1px 0 rgba(255,234,210,.08), inset 0 -2px 0 rgba(0,0,0,.4)',
        color: side === 'BUY' ? 'var(--buy)' : 'var(--sell)',
        fontFamily: 'var(--mono)', fontSize: 11, letterSpacing: '.18em',
        textTransform: 'uppercase', cursor: 'pointer', fontWeight: 700 }}>
        {side} {qty} {sym} @ {px.toFixed(2)}
      </button>
    </div>);

}

function TEChartControls({ ma1, setMa1, ma2, setMa2, period, setPeriod }) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 18,
      padding: '8px 14px',
      backgroundColor: 'var(--bg-2)',
      borderBottom: '1px solid var(--line-3)', fontFamily: "\"IBM Plex Sans\"" }}>
      <TEKnob size={48} min={5} max={200} value={ma1} onChange={setMa1}
      snap label="MA · 1" color="amber" />
      <TEKnob size={48} min={5} max={200} value={ma2} onChange={setMa2}
      snap label="MA · 2" color="buy" />
      <div style={{ borderLeft: '1px solid var(--line-3)', alignSelf: 'stretch' }} />
      <div style={{ display: 'flex', flexDirection: 'column', gap: 5 }}>
        <FieldLabel idx="05" hint="TIMEFRAME">Period</FieldLabel>
        <div style={{ display: 'flex', gap: 3 }}>
          {['1m', '5m', '15m', '1h', '1D'].map((t) =>
          <KeyCap key={t} on={period === t} onClick={() => setPeriod(t)} w={34} h={28}>{t}</KeyCap>
          )}
        </div>
      </div>
      <div style={{ flex: 1 }} />
      <span className="te-live">REC · LIVE</span>
    </div>);

}

function ScreenDenseTE() {
  const [activeSym, setActiveSym] = useState('SPY');
  const [ma1, setMa1] = useState(20);
  const [ma2, setMa2] = useState(50);
  const [period, setPeriod] = useState('5m');
  const sym = SYMS.find((s) => s.sym === activeSym) || SYMS[0];

  return (
    <div className="te" style={{ display: 'grid',
      gridTemplateColumns: '1fr 280px',
      gridTemplateRows: '1fr',
      height: '100%' }}>
      {/* Center: stacked charts with DOM ladder docked */}
      <div style={{ display: 'grid', gridTemplateRows: '1fr 1fr', minHeight: 0 }}>
        {/* Top chart — TE-style control bar + DOM ladder */}
        <div style={{ display: 'flex', flexDirection: 'column', minHeight: 0 }}>
          <TEChartControls ma1={ma1} setMa1={setMa1} ma2={ma2} setMa2={setMa2}
          period={period} setPeriod={setPeriod} />
          <div className="split-h" style={{ display: 'grid', gridTemplateColumns: '220px 1fr',
            flex: 1, minHeight: 0 }}>
            <div className="split-v" style={{ background: 'var(--bg-1)',
              display: 'flex', flexDirection: 'column', minHeight: 0, position: 'relative' }}>
              <DOMLadder centerPx={sym.px} />
              <span className="te-slot">DOM · A</span>
            </div>
            <div style={{ display: 'flex', flexDirection: 'column', minHeight: 0, position: 'relative' }}>
              <ChartPanel sym={sym.sym} price={sym.px} ch={sym.ch} seed={sym.sym.length * 7} />
              <div className="te-grille" />
              <span className="te-slot">CHART · 01 · {period.toUpperCase()}</span>
            </div>
          </div>
        </div>

        {/* Bottom chart — second symbol */}
        <div style={{ display: 'flex', flexDirection: 'column', minHeight: 0, position: 'relative' }}>
          <ChartPanel sym="AAPL" price={293.86} ch={1.78} seed={31} height={260} />
          <div className="te-grille" />
          <span className="te-slot">CHART · 02</span>
        </div>
      </div>

      {/* Right: watchlist + TE order ticket */}
      <div className="split-vL" style={{ background: 'var(--bg-1)',
        display: 'flex', flexDirection: 'column', minHeight: 0 }}>
        <WatchlistPanel
          num="01"
          title="Watchlist"
          group="Megacaps"
          count={12}
          activeSym={activeSym}
          setActiveSym={setActiveSym} />

        <div className="split-hT" style={{ borderTop: '1px solid var(--line-3)' }}>
          <TEOrderTicket sym={sym.sym} refPrice={sym.px} />
        </div>
      </div>
    </div>);

}

window.ScreenDenseTE = ScreenDenseTE;