// APEX — chart-first markets screen. Sym info bar, full-bleed chart, time & sales strip.
// DOM lives in left rail (toggleable from topbar). Order ticket floats (toggleable).

function CandleChart({ candles, height = 380, showSMA = true, lastPrice }) {
  const w = 1200;
  const h = height;
  const padTop = 8, padBot = 56, padR = 56;
  const chartH = h - padTop - padBot;
  const allHi = Math.max(...candles.map(c => c.high));
  const allLo = Math.min(...candles.map(c => c.low));
  const range = allHi - allLo;
  const yScale = v => padTop + (1 - (v - allLo) / range) * chartH;
  const cw = (w - padR) / candles.length;
  const bodyW = Math.max(2, cw * 0.7);
  const sma = candles.map((_, i) => {
    const slice = candles.slice(Math.max(0, i - 19), i + 1);
    return slice.reduce((s, c) => s + c.close, 0) / slice.length;
  });
  const maxVol = Math.max(...candles.map(c => c.volume));
  const volH = 36;
  const volTop = h - padBot + 16;
  const ticks = 6;
  const tickVals = Array.from({length: ticks}, (_, i) => allLo + (range * i) / (ticks - 1));

  return (
    <svg viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none" style={{ width: '100%', height: '100%', display: 'block' }}>
      {tickVals.map((v, i) => (
        <g key={i}>
          <line x1="0" x2={w - padR} y1={yScale(v)} y2={yScale(v)} stroke="rgba(255,255,255,0.04)" strokeWidth="1"/>
          <text x={w - padR + 6} y={yScale(v) + 3} fill="#7a7a7a" style={{ font: "500 10px 'Inter Tight', sans-serif", letterSpacing: '-0.01em' }}>
            {v.toFixed(2)}
          </text>
        </g>
      ))}
      {candles.map((c, i) => {
        const x = i * cw + cw/2;
        const color = c.up ? '#1ed760' : '#f15e6c';
        const yO = yScale(c.open), yC = yScale(c.close);
        const yT = Math.min(yO, yC), yB = Math.max(yO, yC);
        return (
          <g key={i}>
            <line x1={x} x2={x} y1={yScale(c.high)} y2={yScale(c.low)} stroke={color} strokeWidth="1"/>
            <rect x={x - bodyW/2} y={yT} width={bodyW} height={Math.max(1, yB - yT)} fill={color}/>
            <rect x={x - bodyW/2} y={volTop + (1 - c.volume/maxVol) * volH}
                  width={bodyW} height={(c.volume/maxVol) * volH}
                  fill={color} opacity="0.4"/>
          </g>
        );
      })}
      {showSMA && (
        <path d={sma.map((v, i) => `${i===0?'M':'L'}${i*cw + cw/2},${yScale(v)}`).join(' ')}
              stroke="#e6c84a" strokeWidth="1.5" fill="none" opacity="0.85"/>
      )}
      {lastPrice && (
        <g>
          <line x1="0" x2={w - padR} y1={yScale(lastPrice)} y2={yScale(lastPrice)} stroke="#1ed760" strokeWidth="1" strokeDasharray="4 4"/>
          <rect x={w - padR + 2} y={yScale(lastPrice) - 9} width={52} height={18} fill="#1ed760" rx="3"/>
          <text x={w - padR + 28} y={yScale(lastPrice) + 4} textAnchor="middle" fill="#000" style={{ font: "700 11px 'Inter Tight', sans-serif", letterSpacing: '-0.01em' }}>
            {lastPrice.toFixed(2)}
          </text>
        </g>
      )}
    </svg>
  );
}

function ChartTools({ tf, onTf }) {
  const tfs = ['1m', '5m', '15m', '1h', '4h', '1d', '1w'];
  return (
    <>
      {tfs.map(t => <ToolBtn key={t} active={t === tf} onClick={() => onTf(t)}>{t}</ToolBtn>)}
      <div style={{ width: 1, height: 16, background: 'var(--line)', margin: '0 4px' }}/>
      <ToolBtn>Candles <Ic.Chevron size={11} dir="down"/></ToolBtn>
      <ToolBtn>Indicators</ToolBtn>
      <ToolBtn>Drawing</ToolBtn>
      <ToolBtn>Alerts</ToolBtn>
    </>
  );
}

function ApexScreen({ activeSym }) {
  const sym = WATCHLIST.find(w => w.sym === activeSym) || WATCHLIST[0];
  const up = sym.chg >= 0;
  const candleSet = sym.sym === 'AAPL' ? AAPL_CANDLES : sym.sym === 'SPY' ? SPY_CANDLES : NVDA_CANDLES;

  return (
    <div className="apex-screen">
      {/* Symbol info bar */}
      <div className="apex-symbar">
        <div className="sb-left">
          <span className="sb-sym display">{sym.sym}</span>
          <div className="sb-sub">
            <span className="sb-name">{sym.name}</span>
            <span className="sb-tag">NASDAQ</span>
          </div>
        </div>
        <div className="sb-prices">
          <span className="sb-px display num">{sym.px.toFixed(2)}</span>
          <div className="sb-chg-block">
            <span className={'sb-chg num ' + (up ? 'pos' : 'neg')}>
              {up ? '+' : '−'}{Math.abs(sym.chg * sym.px / 100).toFixed(2)}
            </span>
            <span className={'sb-chg-pct num ' + (up ? 'pos' : 'neg')}>
              {up ? '+' : '−'}{Math.abs(sym.chg).toFixed(2)}%
            </span>
          </div>
        </div>
        <div className="sb-stats">
          <Stat l="Open" v={(sym.px - 8).toFixed(2)}/>
          <Stat l="High" v={(sym.px + 7.4).toFixed(2)}/>
          <Stat l="Low" v={(sym.px - 12.1).toFixed(2)}/>
          <Stat l="Volume" v={sym.vol}/>
          <Stat l="Mkt Cap" v="3.14T"/>
          <Stat l="P/E" v="68.2"/>
          <Stat l="52W H" v={(sym.px * 1.18).toFixed(2)}/>
          <Stat l="52W L" v={(sym.px * 0.62).toFixed(2)}/>
        </div>
      </div>

      {/* Main chart fills the area */}
      <div className="chart-main">
        <CandleChart candles={candleSet} lastPrice={sym.px}/>
      </div>

      {/* Time & sales strip */}
      <div className="apex-bottom">
        <div className="ts-head">
          <span className="ts-title">Time &amp; Sales</span>
          <span className="ts-sub">Last 50 prints · streaming</span>
        </div>
        <div className="ts-grid">
          {Array.from({length: 18}).map((_, i) => {
            const dir = i % 3 === 0 ? 'down' : 'up';
            const px = sym.px + (Math.sin(i * 1.7) * 0.4);
            const sz = 100 + Math.floor(Math.abs(Math.sin(i * 0.9)) * 800);
            const t = `06:42:${String(58 - i).padStart(2,'0')}`;
            return (
              <div key={i} className="ts-row">
                <span className="ts-t num">{t}</span>
                <span className={'ts-px num ' + (dir === 'up' ? 'pos' : 'neg')}>{px.toFixed(2)}</span>
                <span className="ts-sz num">{sz}</span>
                <span className="ts-ex">NSDQ</span>
              </div>
            );
          })}
        </div>
      </div>

      <style>{`
        .apex-screen {
          display: grid;
          grid-template-rows: 64px 1fr 168px;
          height: 100%; min-height: 0;
          background: var(--bg-elev-1);
          border-radius: 12px;
          overflow: hidden;
        }
        .apex-symbar {
          display: flex; align-items: center; gap: 24px;
          padding: 0 20px; border-bottom: 1px solid var(--line);
        }
        .sb-left { display: flex; align-items: center; gap: 14px; }
        .sb-sym { font: 800 26px 'Inter Tight', sans-serif; letter-spacing: -0.03em; color: #fff; }
        .sb-sub { display: flex; flex-direction: column; gap: 2px; }
        .sb-name { font: 500 13px 'Inter', sans-serif; color: var(--text-secondary); letter-spacing: -0.005em; }
        .sb-tag {
          font: 600 9px 'Inter', sans-serif; letter-spacing: 0.06em;
          color: var(--text-tertiary); text-transform: uppercase;
        }
        .sb-prices { display: flex; align-items: baseline; gap: 14px; }
        .sb-px { font: 700 28px 'Inter Tight', sans-serif; color: #fff; }
        .sb-chg-block { display: flex; flex-direction: column; gap: 0; }
        .sb-chg { font: 600 13px 'Inter Tight', sans-serif; }
        .sb-chg-pct { font: 600 13px 'Inter Tight', sans-serif; }
        .sb-stats { display: flex; gap: 22px; margin-left: auto; padding-right: 4px; }

        .chart-main { background: var(--bg-base); padding: 8px 0; min-height: 0; }

        .apex-bottom {
          background: rgba(0,0,0,0.4);
          border-top: 1px solid var(--line);
          display: flex; flex-direction: column;
          min-height: 0;
        }
        .ts-head {
          display: flex; align-items: baseline; gap: 12px;
          padding: 10px 20px; border-bottom: 1px solid var(--line);
          flex: 0 0 auto;
        }
        .ts-title { font: 700 13px 'Inter Tight', sans-serif; letter-spacing: -0.02em; color: #fff; }
        .ts-sub { font: 500 11px 'Inter', sans-serif; color: var(--text-tertiary); letter-spacing: -0.005em; }
        .ts-grid {
          flex: 1; min-height: 0; overflow-y: auto;
          display: grid; grid-template-columns: repeat(3, 1fr);
          padding: 4px 0;
        }
        .ts-row {
          display: grid; grid-template-columns: 80px 80px 1fr 60px; gap: 12px;
          padding: 4px 20px;
          font-size: 12px;
        }
        .ts-row:hover { background: rgba(255,255,255,0.03); }
        .ts-t { color: var(--text-tertiary); font: 500 12px 'Inter Tight', sans-serif; letter-spacing: -0.01em; }
        .ts-px { font: 600 12px 'Inter Tight', sans-serif; }
        .ts-sz { color: var(--text-secondary); font: 500 12px 'Inter Tight', sans-serif; letter-spacing: -0.01em; }
        .ts-ex { font: 600 10px 'Inter', sans-serif; color: var(--text-tertiary); letter-spacing: 0.06em; }
      `}</style>
    </div>
  );
}

function Stat({ l, v }) {
  return (
    <div className="stat">
      <span className="stat-l">{l}</span>
      <span className="stat-v num">{v}</span>
      <style>{`
        .stat { display: flex; flex-direction: column; gap: 1px; }
        .stat-l { font: 500 10px 'Inter', sans-serif; letter-spacing: 0.02em; color: var(--text-tertiary); }
        .stat-v { font: 600 13px 'Inter Tight', sans-serif; color: #fff; letter-spacing: -0.01em; }
      `}</style>
    </div>
  );
}

window.ApexScreen = ApexScreen;
window.ChartTools = ChartTools;
