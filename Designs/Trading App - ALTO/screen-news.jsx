// NEWS — feed with hero story, filters, sentiment chips
function NewsScreen() {
  const [filter, setFilter] = useState('All');
  const filters = ['All', 'Macro', 'Earnings', 'Tech', 'Crypto', 'Fed', 'M&A'];

  const stories = [
    { time:'23:18', src:'BBG',  cat:'Earnings', head:'NVIDIA Q3 revenue tops estimates as data center demand accelerates', sub:'Beat by $1.2B; guides Q4 above consensus. Stock +4% after-hours.', tickers:['NVDA','AMD'], sent:'pos' },
    { time:'23:04', src:'RTRS', cat:'Fed',      head:'Fed minutes: officials see gradual rate path through Q2', sub:'Members unanimous on holding; two see one cut in March.', tickers:['SPY','TLT'], sent:'neutral' },
    { time:'22:51', src:'WSJ',  cat:'Tech',     head:'Tesla cuts production targets in Shanghai gigafactory', sub:'Local demand softens 12%; cuts 2 shifts on Model Y line.', tickers:['TSLA'], sent:'neg' },
    { time:'22:40', src:'CNBC', cat:'Macro',    head:'Oil falls to $68 as inventories rise, OPEC+ holds output', sub:'EIA prints +6.2M bbl, doubling estimates.', tickers:['XLE','USO'], sent:'neg' },
    { time:'22:31', src:'BBG',  cat:'Tech',     head:'Apple Vision Pro 2 launches; pre-orders begin Friday', sub:'Lighter, $2,499, ships March 1.', tickers:['AAPL'], sent:'pos' },
    { time:'22:18', src:'FT',   cat:'Earnings', head:'JPMorgan boosts FY guidance, Dimon flags soft landing', sub:'NII up 8% y/y; raises full-year guide to $93B.', tickers:['JPM','XLF'], sent:'pos' },
    { time:'22:02', src:'BBG',  cat:'Crypto',   head:'BTC reclaims $94K as ETF flows hit weekly record', sub:'IBIT and FBTC together pulled in $2.1B last week.', tickers:['BTC','COIN'], sent:'pos' },
    { time:'21:48', src:'RTRS', cat:'M&A',      head:'Capital One closes Discover acquisition', sub:'Combined entity now 4th largest US issuer by purchase volume.', tickers:['COF','DFS'], sent:'pos' },
  ];
  const list = stories.filter(s => filter==='All' || s.cat===filter);
  const hero = list[0];
  const rest = list.slice(1);

  return (
    <div className="news-screen">
      <div className="news-head">
        <div>
          <div className="news-eyebrow"><Ic.News size={11}/> NEWSDESK · LIVE</div>
          <h1 className="news-title display">Newsroom</h1>
        </div>
        <div className="news-stream">
          <span className="dot-live" style={{ width:8,height:8,borderRadius:50,background:'var(--green)',display:'inline-block' }}/>
          <span>Streaming · 4 sources</span>
        </div>
      </div>

      <div className="news-filters">
        {filters.map(f => (
          <button key={f} className={'news-chip' + (filter===f?' active':'')} onClick={() => setFilter(f)}>{f}</button>
        ))}
        <button className="news-chip" style={{ marginLeft:'auto' }}><Ic.Filter size={11}/> Sources</button>
      </div>

      <div className="news-grid">
        {hero && (
          <div className="news-hero">
            <div className="news-hero-img">
              <div className="img-placeholder">
                <Ic.News size={36}/>
                <span>{hero.src}</span>
              </div>
            </div>
            <div className="news-hero-body">
              <div className="news-meta">
                <span className={'news-sent ' + hero.sent}>
                  {hero.sent==='pos' ? <Ic.TrendUp size={10}/> : hero.sent==='neg' ? <Ic.TrendDown size={10}/> : <Ic.Activity size={10}/>}
                  {hero.cat}
                </span>
                <span className="news-time"><Ic.Clock size={10}/> {hero.time}</span>
                <span className="news-src">{hero.src}</span>
              </div>
              <h2 className="news-hero-head">{hero.head}</h2>
              <p className="news-hero-sub">{hero.sub}</p>
              <div className="news-tickers">
                {hero.tickers.map(t => <span key={t} className="news-ticker">{t}</span>)}
              </div>
              <div className="news-actions">
                <button className="news-act primary"><Ic.Eye size={12}/> Read full</button>
                <button className="news-act"><Ic.Bookmark size={12}/> Save</button>
                <button className="news-act"><Ic.Share size={12}/></button>
              </div>
            </div>
          </div>
        )}

        <div className="news-list">
          {rest.map((s, i) => (
            <div key={i} className="news-row">
              <div className="news-row-time">
                <span className="num">{s.time}</span>
                <span className={'news-sent ' + s.sent}>
                  {s.sent==='pos' ? <Ic.TrendUp size={10}/> : s.sent==='neg' ? <Ic.TrendDown size={10}/> : <Ic.Activity size={10}/>}
                </span>
              </div>
              <div className="news-row-body">
                <div className="news-row-head">
                  <span className="news-src">{s.src}</span>
                  <span className="news-cat">{s.cat}</span>
                </div>
                <div className="news-row-headline">{s.head}</div>
                <div className="news-row-sub">{s.sub}</div>
                <div className="news-tickers">
                  {s.tickers.map(t => <span key={t} className="news-ticker sm">{t}</span>)}
                </div>
              </div>
              <button className="icon-btn-sm" aria-label="More"><Ic.Dots size={14}/></button>
            </div>
          ))}
        </div>
      </div>

      <style>{`
        .news-screen { padding: 18px 22px 22px; height: 100%; overflow-y: auto; flex: 1; display: flex; flex-direction: column; gap: 14px; }
        .news-head { display:flex; align-items:end; justify-content:space-between; }
        .news-eyebrow { display:inline-flex; align-items:center; gap:6px; font: 700 10px 'Inter', sans-serif; letter-spacing:0.12em; color: var(--text-secondary); }
        .news-title { font: 800 32px 'Inter Tight', sans-serif; letter-spacing: -0.03em; color:#fff; margin:4px 0 0; }
        .news-stream { display:inline-flex; align-items:center; gap:8px; font: 600 12px 'Inter', sans-serif; letter-spacing:-0.005em; color: var(--text-secondary); }

        .news-filters { display: flex; gap: 6px; flex-wrap: wrap; }
        .news-chip {
          height: 30px; padding: 0 14px; border-radius: 999px;
          background: rgba(255,255,255,0.06); border: 0; color:#fff; cursor: pointer;
          font: 600 12px 'Inter', sans-serif; letter-spacing: -0.005em;
          display:inline-flex; align-items:center; gap:5px;
        }
        .news-chip:hover { background: rgba(255,255,255,0.10); }
        .news-chip.active { background: #fff; color: #000; }

        .news-grid { display: grid; grid-template-columns: 1.2fr 1fr; gap: 14px; }
        @media (max-width: 1100px) { .news-grid { grid-template-columns: 1fr; } }

        .news-hero {
          background: var(--bg-elev-1); border-radius: 14px; overflow: hidden;
          display: flex; flex-direction: column;
        }
        .news-hero-img {
          aspect-ratio: 16/8;
          background: linear-gradient(135deg, #5038a0 0%, #1a1a1a 100%);
          display:flex; align-items:center; justify-content:center; position:relative;
        }
        .img-placeholder {
          display:flex; flex-direction:column; align-items:center; gap:8px;
          color: rgba(255,255,255,0.4);
          font: 700 12px 'Inter', sans-serif; letter-spacing: 0.1em;
        }
        .news-hero-body { padding: 18px 20px; display:flex; flex-direction:column; gap: 10px; }
        .news-meta { display:flex; align-items:center; gap:10px; }
        .news-sent {
          display:inline-flex; align-items:center; gap:4px;
          font: 700 10px 'Inter', sans-serif; letter-spacing: 0.06em; text-transform: uppercase;
          padding: 3px 8px; border-radius: 4px;
        }
        .news-sent.pos { color: var(--green); background: var(--green-bg); }
        .news-sent.neg { color: var(--red);   background: var(--red-bg); }
        .news-sent.neutral { color: var(--text-secondary); background: rgba(255,255,255,0.06); }
        .news-time { display:inline-flex; align-items:center; gap:4px; font: 500 11px 'Inter Tight', sans-serif; color: var(--text-tertiary); }
        .news-src {
          font: 700 9px 'Inter', sans-serif; letter-spacing: 0.08em; text-transform: uppercase;
          color: var(--text-secondary); padding: 2px 6px; border: 1px solid var(--line); border-radius: 4px;
        }
        .news-cat { font: 600 10px 'Inter', sans-serif; letter-spacing: 0.06em; text-transform: uppercase; color: var(--text-tertiary); }

        .news-hero-head { font: 700 22px 'Inter Tight', sans-serif; letter-spacing: -0.025em; color:#fff; margin:0; line-height: 1.25; text-wrap: pretty; }
        .news-hero-sub { font: 500 14px 'Inter', sans-serif; line-height: 1.5; color: var(--text-secondary); margin:0; letter-spacing: -0.005em; text-wrap: pretty; }

        .news-tickers { display:flex; gap:6px; flex-wrap: wrap; }
        .news-ticker {
          font: 700 11px 'Inter Tight', sans-serif; letter-spacing: -0.01em; color:#fff;
          padding: 3px 8px; background: rgba(255,255,255,0.08); border-radius: 4px;
        }
        .news-ticker.sm { font-size: 10px; padding: 2px 6px; }

        .news-actions { display: flex; gap: 6px; margin-top: 4px; }
        .news-act {
          height: 34px; padding: 0 14px; border-radius: 999px;
          background: rgba(255,255,255,0.06); color:#fff; border: 0; cursor: pointer;
          font: 600 12px 'Inter', sans-serif; letter-spacing: -0.005em;
          display:inline-flex; align-items:center; gap:6px;
        }
        .news-act:hover { background: rgba(255,255,255,0.12); }
        .news-act.primary { background: var(--green); color: #000; }

        .news-list { display:flex; flex-direction: column; gap: 1px; background: var(--line); border-radius: 14px; overflow: hidden; }
        .news-row {
          display: grid; grid-template-columns: 70px 1fr auto; gap: 14px; align-items: flex-start;
          padding: 14px 16px; background: var(--bg-elev-1);
        }
        .news-row:hover { background: var(--bg-card-hover); }
        .news-row-time { display:flex; flex-direction: column; gap: 4px; align-items:flex-start; }
        .news-row-time .num { font: 600 13px 'Inter Tight', sans-serif; color: var(--text-secondary); letter-spacing: -0.01em; }
        .news-row-body { display:flex; flex-direction: column; gap: 4px; min-width: 0; }
        .news-row-head { display:flex; align-items:center; gap: 8px; }
        .news-row-headline { font: 600 14px 'Inter', sans-serif; color:#fff; letter-spacing: -0.005em; line-height: 1.35; text-wrap: pretty; }
        .news-row-sub { font: 500 12px 'Inter', sans-serif; color: var(--text-secondary); letter-spacing: -0.005em; line-height: 1.5; }
      `}</style>
    </div>
  );
}

window.NewsScreen = NewsScreen;
