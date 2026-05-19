// ed-dom.jsx — Editorial DOM ladder
// Dense L2 ladder, ask above mid above bid, depth bars.

function EdDOMLadder({ centerPx, rows = 14 }){
  const ladder = useMemo(() => makeLadder(centerPx, rows, Math.floor(centerPx * 100)),
                          [centerPx, rows]);

  // Find max depth to scale bars
  const maxDepth = Math.max(...ladder.map(r => r.depth));

  return (
    <div className="dom-ed">
      <div className="dom-ed-head">
        <span>Size</span>
        <span className="c">Price</span>
        <span className="r">Size</span>
      </div>
      {ladder.map((r, i) => {
        const width = Math.max(2, (r.depth / maxDepth) * 100);
        const isAsk = r.side === 'ask';
        const isBid = r.side === 'bid';
        const isMid = r.side === 'mid';
        return (
          <div key={i} className={`dom-ed-row ${r.side}`}>
            {!isMid && (
              <div className={`depthbar ${isAsk ? 'ask' : 'bid'}`} style={{ width: `${width}%` }}/>
            )}
            <span className="sz">{isAsk ? r.vol.toLocaleString() : ''}</span>
            <span className="px">{r.px.toFixed(2)}</span>
            <span className="sz r">{!isAsk ? r.vol.toLocaleString() : ''}</span>
          </div>
        );
      })}
    </div>
  );
}

window.EdDOMLadder = EdDOMLadder;
