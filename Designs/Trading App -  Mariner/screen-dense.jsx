// screen-dense.jsx — Chart Desk: dense chart + DOM ladder + watchlist + order ticket
// Inspired by image 10 (the IBKR-style dense terminal)

function DOMLadder({ centerPx }) {
  const rows = useMemo(() => makeLadder(centerPx, 22, Math.floor(centerPx * 100)), [centerPx]);
  return (
    <div style={{ display: 'flex', flexDirection: 'column', minHeight: 0 }}>
      <div style={{ display: 'grid', gridTemplateColumns: '60px 70px 60px', padding: '4px 6px',
        fontFamily: 'var(--mono)', fontSize: 9.5, color: 'var(--ink-3)', letterSpacing: '.06em', textTransform: 'uppercase',
        borderBottom: '1px solid var(--line-3)', backgroundColor: 'var(--bg-1)' }}>
        <span>Δ</span>
        <span style={{ textAlign: 'center' }}>Price</span>
        <span style={{ textAlign: 'right' }}>Size</span>
      </div>
      <div className="scroll" style={{ flex: 1, minHeight: 0 }}>
        {rows.map((r, i) => {
          const dCol = r.delta >= 0 ? 'var(--buy)' : 'var(--sell)';
          const sideCol = r.side === 'bid' ? 'rgba(226,93,93,.10)' : r.side === 'ask' ? 'rgba(111,191,115,.10)' : 'transparent';
          const barW = Math.max(2, r.depth * 100);
          return (
            <div key={i} className={`dom-row ${r.side}`} style={{ position: 'relative' }}>
              <div style={{ position: 'absolute', inset: 0,
                background: `linear-gradient(90deg, transparent ${100 - barW}%, ${sideCol} 100%)` }} />
              <span className="delta" style={{ position: 'relative', color: dCol }}>{r.delta >= 0 ? '+' : ''}{r.delta}</span>
              <span className="px" style={{ position: 'relative' }}>{r.px.toFixed(2)}</span>
              <span className="vol" style={{ position: 'relative' }}>{r.vol.toLocaleString()}</span>
            </div>);

        })}
      </div>
    </div>);

}

// MiniWatchlist removed — replaced by shared WatchlistPanel

function OrderTicket({ sym, refPrice }) {
  const [side, setSide] = useState('buy');
  const [qty, setQty] = useState(100);
  const [limit, setLimit] = useState(refPrice);
  const [type, setType] = useState('LMT');
  const [tif, setTif] = useState('DAY');
  useEffect(() => {setLimit(refPrice);}, [refPrice]);

  const cost = qty * limit;
  return (
    <div style={{ display: 'flex', flexDirection: 'column', padding: 10, gap: 10 }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <div style={{ display: 'flex', alignItems: 'baseline', gap: 8 }}>
          <span style={{ fontWeight: 700, letterSpacing: '.04em', color: 'var(--ink-0)' }}>{sym}</span>
          <span className="num" style={{ color: 'var(--ink-2)', fontSize: 11 }}>{fmtNum(refPrice)}</span>
          <span className="num t-up" style={{ fontSize: 10.5 }}>+2.49%</span>
        </div>
        <button className="btn-ghost" style={{ width: 22, height: 22, borderRadius: 4, display: 'inline-flex', alignItems: 'center', justifyContent: 'center' }}><Icon.Close /></button>
      </div>

      <div className="seg">
        <button className={`sg ${side === 'buy' ? 'on-buy' : ''}`} onClick={() => setSide('buy')}>BUY</button>
        <button className={`sg ${side === 'sell' ? 'on-sell' : ''}`} onClick={() => setSide('sell')}>SELL</button>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8 }}>
        <div className={side === 'sell' ? 'tile-sell' : 'tile-buy'} style={{ padding: '6px 8px' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between' }}>
            <span style={{ fontSize: 9.5, letterSpacing: '.08em', color: side === 'sell' ? 'var(--sell-ink)' : 'var(--buy-ink)' }}>BID</span>
          </div>
          <div className="num" style={{ fontSize: 18, color: 'var(--ink-0)' }}>{(refPrice - 0.07).toFixed(2)}</div>
        </div>
        <div className={side === 'sell' ? 'tile-buy' : 'tile-sell'} style={{ padding: '6px 8px', textAlign: 'right' }}>
          <div style={{ display: 'flex', justifyContent: 'flex-end' }}>
            <span style={{ fontSize: 9.5, letterSpacing: '.08em', color: side === 'sell' ? 'var(--buy-ink)' : 'var(--sell-ink)' }}>ASK</span>
          </div>
          <div className="num" style={{ fontSize: 18, color: 'var(--ink-0)' }}>{(refPrice + 0.07).toFixed(2)}</div>
        </div>
      </div>

      <div style={{ display: 'flex', justifyContent: 'center', fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--ink-3)', letterSpacing: '.04em' }}>
        SPREAD <span style={{ color: 'var(--ink-1)', marginLeft: 6 }}>0.14</span>
      </div>

      <div>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'baseline', marginBottom: 5 }}>
          <span style={{ fontSize: 10, color: 'var(--ink-3)', letterSpacing: '.06em', textTransform: 'uppercase' }}>Limit</span>
          <span className="mono" style={{ fontSize: 10, color: 'var(--ink-3)' }}>mid {refPrice.toFixed(2)}</span>
        </div>
        <div className="stp">
          <button className="stp-b" onClick={() => setLimit((l) => +(l - 0.01).toFixed(2))}>−</button>
          <input className="stp-i" value={limit.toFixed(2)} onChange={(e) => setLimit(+e.target.value || 0)} />
          <button className="stp-b" onClick={() => setLimit((l) => +(l + 0.01).toFixed(2))}>+</button>
        </div>
      </div>

      <div>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'baseline', marginBottom: 5 }}>
          <span style={{ fontSize: 10, color: 'var(--ink-3)', letterSpacing: '.06em', textTransform: 'uppercase' }}>Quantity</span>
          <div style={{ display: 'flex', gap: 4 }}>
            {[25, 50, 100].map((p) =>
            <button key={p} className="pill" onClick={() => setQty(Math.round(1000 * p / 100))}
            style={{ height: 18, padding: '0 7px', fontSize: 10 }}>{p}%</button>
            )}
          </div>
        </div>
        <div className="stp">
          <button className="stp-b" onClick={() => setQty((q) => Math.max(0, q - 10))}>−</button>
          <input className="stp-i" value={qty} onChange={(e) => setQty(+e.target.value || 0)} />
          <button className="stp-b" onClick={() => setQty((q) => q + 10)}>+</button>
        </div>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8 }}>
        <div>
          <div style={{ fontSize: 10, color: 'var(--ink-3)', letterSpacing: '.06em', textTransform: 'uppercase', marginBottom: 5 }}>Type</div>
          <div className="seg">
            <button className={`sg ${type === 'MKT' ? 'on-acc' : ''}`} onClick={() => setType('MKT')}>MKT</button>
            <button className={`sg ${type === 'LMT' ? 'on-acc' : ''}`} onClick={() => setType('LMT')}>LMT</button>
          </div>
        </div>
        <div>
          <div style={{ fontSize: 10, color: 'var(--ink-3)', letterSpacing: '.06em', textTransform: 'uppercase', marginBottom: 5 }}>TIF</div>
          <div className="seg">
            <button className={`sg ${tif === 'DAY' ? 'on-acc' : ''}`} onClick={() => setTif('DAY')}>DAY</button>
            <button className={`sg ${tif === 'GTC' ? 'on-acc' : ''}`} onClick={() => setTif('GTC')}>GTC</button>
          </div>
        </div>
      </div>

      <div className="hr" style={{ margin: '2px -10px' }} />

      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <div>
          <div style={{ fontSize: 10, color: 'var(--ink-3)', letterSpacing: '.06em', textTransform: 'uppercase' }}>Est. Cost</div>
          <div className="num" style={{ fontSize: 15, color: 'var(--ink-0)' }}>${fmtNum(cost)}</div>
          <div style={{ fontSize: 10, color: 'var(--ink-3)' }}>@ ${refPrice.toFixed(2)} · BP $5.83M</div>
        </div>
        <button className={side === 'buy' ? 'btn btn-buy' : 'btn btn-sell'}
        style={{ height: 46, minWidth: 120, fontSize: 13, letterSpacing: '.06em', textTransform: 'uppercase' }}>
          {side === 'buy' ? `Buy ${qty}` : `Sell ${qty}`}
        </button>
      </div>
    </div>);

}

function ChartHeader({ sym, price, ch, tf, setTf, candleStyle, setCandleStyle }) {
  const tfs = ['1m', '5m', '15m', '30m', '1H', '4H', '1D', '1W'];
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '8px 10px',
      borderBottom: '1px solid var(--line-3)', boxShadow: '0 1px 0 rgba(255,234,210,.025), inset 0 1px 0 var(--hi-1)', backgroundColor: 'var(--bg-2)', color: 'var(--ink-0)' }}>
      <button className="knob" style={{ width: 18, height: 18 }}>
        <Icon.Caret size={9} />
      </button>
      <div style={{ display: 'flex', alignItems: 'baseline', gap: 8 }}>
        <span style={{ fontWeight: 700, letterSpacing: '.04em' }}>{sym}</span>
        <span className="num" style={{ color: 'var(--accent-ink)' }}>${fmtNum(price)}</span>
        <span className={`num ${ch >= 0 ? 't-up' : 't-dn'}`} style={{ fontSize: 11 }}>{fmtCh(ch)}</span>
      </div>
      <div style={{ flex: 1 }} />
      <div className="seg" style={{ padding: 1 }}>
        {tfs.map((tt) =>
        <button key={tt} className={`sg ${tf === tt ? 'on-acc' : ''}`} onClick={() => setTf(tt)}
        style={{ height: 20, padding: '0 8px', fontSize: 10.5, letterSpacing: '.04em', borderColor: "rgb(161, 156, 142)" }}>
            {tt}
          </button>
        )}
      </div>
      <div className="seg" style={{ padding: 1 }}>
        {['Candle', 'Line'].map((s) =>
        <button key={s} className={`sg ${candleStyle === s ? 'on-acc' : ''}`} onClick={() => setCandleStyle(s)}
        style={{ height: 20, padding: '0 8px', fontSize: 10.5, letterSpacing: '.04em' }}>
            {s}
          </button>
        )}
      </div>
      <div style={{ display: 'flex', gap: 2, marginLeft: 6 }}>
        <button className="btn-ghost" style={{ width: 22, height: 22, borderRadius: 4, display: 'inline-flex', alignItems: 'center', justifyContent: 'center' }}><Icon.Pen /></button>
        <button className="btn-ghost" style={{ width: 22, height: 22, borderRadius: 4, display: 'inline-flex', alignItems: 'center', justifyContent: 'center' }}><Icon.Wave /></button>
        <button className="btn-ghost" style={{ width: 22, height: 22, borderRadius: 4, display: 'inline-flex', alignItems: 'center', justifyContent: 'center' }}><Icon.Maximize /></button>
      </div>
    </div>);

}

function ChartPanel({ sym, price, ch, seed, height = 320 }) {
  const [tf, setTf] = useState('5m');
  const [candleStyle, setCandleStyle] = useState('Candle');
  // Different number of candles per tf for visual variety
  const counts = { '1m': 110, '5m': 90, '15m': 80, '30m': 70, '1H': 60, '4H': 50, '1D': 45, '1W': 36 };
  const candles = useMemo(() => makeCandles(counts[tf] || 80, seed + tf.length, price * 0.997, price * 0.0035),
  [tf, seed, price]);
  const containerRef = useRef(null);
  const [w, setW] = useState(900);
  useEffect(() => {
    if (!containerRef.current) return;
    const ro = new ResizeObserver((es) => setW(es[0].contentRect.width));
    ro.observe(containerRef.current);
    return () => ro.disconnect();
  }, []);
  return (
    <div style={{ display: 'flex', flexDirection: 'column', minHeight: 0, flex: 1 }}>
      <ChartHeader sym={sym} price={price} ch={ch} tf={tf} setTf={setTf}
      candleStyle={candleStyle} setCandleStyle={setCandleStyle} />
      <div ref={containerRef} style={{ flex: 1, minHeight: 0, position: 'relative' }}>
        <div style={{ position: 'absolute', inset: 0 }}>
          <CandleChart candles={candles} width={w} height={height}
          lineOverlay={candleStyle === 'Line'} showVolume showGrid />
        </div>
        {/* Floating order ticket overlay */}
      </div>
    </div>);

}

function ScreenDense() {
  const [activeSym, setActiveSym] = useState('SPY');
  const sym = SYMS.find((s) => s.sym === activeSym) || SYMS[0];

  return (
    <div style={{ display: 'grid',
      gridTemplateColumns: '1fr 260px',
      gridTemplateRows: '1fr',
      height: '100%' }}>

      {/* Center: stacked charts with DOM ladder docked */}
      <div style={{ display: 'grid', gridTemplateRows: '1fr 1fr', minHeight: 0 }}>
        {/* Top chart with DOM ladder */}
        <div className="split-h" style={{ display: 'grid', gridTemplateColumns: '220px 1fr', minHeight: 0 }}>
          <div className="split-v" style={{ background: 'var(--bg-1)',
            display: 'flex', flexDirection: 'column', minHeight: 0 }}>
            <DOMLadder centerPx={sym.px} />
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', minHeight: 0, position: 'relative' }}>
            <ChartPanel sym={sym.sym} price={sym.px} ch={sym.ch} seed={sym.sym.length * 7} />
            {/* Floating order ticket */}
            <div className="card" style={{ position: 'absolute', top: 60, right: 20, width: 280, zIndex: 3 }}>
              <OrderTicket sym={sym.sym} refPrice={sym.px} />
            </div>
          </div>
        </div>

        {/* Bottom chart — second symbol */}
        <div style={{ display: 'flex', flexDirection: 'column', minHeight: 0 }}>
          <ChartPanel sym="AAPL" price={293.86} ch={1.78} seed={31} height={260} />
        </div>
      </div>

      {/* Right: shared watchlist + options */}
      <div className="split-vL" style={{ background: 'var(--bg-1)',
        display: 'flex', flexDirection: 'column', minHeight: 0 }}>
        <WatchlistPanel
          num="01"
          title="Watchlist"
          group="Megacaps"
          count={12}
          activeSym={activeSym}
          setActiveSym={setActiveSym} />

        {/* Options section */}
        <div className="split-hT">
          <div className="ph" style={{ padding: '6px 12px' }}>
            <span className="fr-label"><span className="num">02</span> Options · 4</span>
          </div>
          <div style={{ padding: '2px 12px 10px' }}>
            {[
            { side: 'C', sym: 'SPY', strike: '718', dte: '0DTE', bid: 2.67, ask: 2.79, col: 'var(--buy)' },
            { side: 'C', sym: 'SPY', strike: '716', dte: '0DTE', bid: 3.44, ask: 3.59, col: 'var(--buy)' },
            { side: 'P', sym: 'SPY', strike: '711', dte: '0DTE', bid: 3.03, ask: 3.16, col: 'var(--sell)' },
            { side: 'P', sym: 'SPY', strike: '709', dte: '0DTE', bid: 2.32, ask: 2.42, col: 'var(--sell)' }].
            map((o, i) =>
            <div key={i} style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '4px 0',
              fontFamily: 'var(--mono)', fontSize: 11 }}>
                <span style={{ color: o.col, width: 10 }}>{o.side}</span>
                <span style={{ color: 'var(--ink-2)' }}>{o.sym}</span>
                <span style={{ color: 'var(--ink-1)' }}>{o.strike}</span>
                <span style={{ color: 'var(--ink-3)', fontSize: 10 }}>{o.dte}</span>
                <span style={{ flex: 1 }} />
                <span style={{ color: o.col }}>{o.bid.toFixed(2)}</span>
                <span style={{ color: 'var(--ink-3)' }}>×</span>
                <span style={{ color: o.col }}>{o.ask.toFixed(2)}</span>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>);

}

window.ScreenDense = ScreenDense;