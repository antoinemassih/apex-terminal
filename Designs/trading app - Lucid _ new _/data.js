// Trading terminal mock data
window.TERMINAL_DATA = (function(){
  // Generate deterministic candle data
  function seedRand(seed){let s=seed;return ()=>{s=(s*9301+49297)%233280;return s/233280;};}

  function genCandles(n, start, vol, trend){
    const r = seedRand(start*1000|0);
    const out=[]; let p=start;
    for(let i=0;i<n;i++){
      const drift=(r()-0.45)*vol + trend;
      const o=p; const c=Math.max(0.1, p+drift);
      const hi=Math.max(o,c)+r()*vol*0.6;
      const lo=Math.min(o,c)-r()*vol*0.6;
      out.push({o,h:hi,l:lo,c,v: 50000+r()*450000|0});
      p=c;
    }
    return out;
  }

  const watchlist = [
    {sym:'NVDA', name:'NVIDIA Corp',          last: 1284.32, chg:  +28.40, pct:  +2.26, vol:'48.2M', mcap:'3.16T'},
    {sym:'AAPL', name:'Apple Inc',             last:  221.18, chg:  -1.85, pct:  -0.83, vol:'52.1M', mcap:'3.36T'},
    {sym:'MSFT', name:'Microsoft Corp',        last:  438.94, chg:  +3.12, pct:  +0.72, vol:'19.4M', mcap:'3.26T'},
    {sym:'TSLA', name:'Tesla Inc',             last:  254.18, chg: -10.27, pct:  -3.88, vol:'88.7M', mcap:'810B'},
    {sym:'AMZN', name:'Amazon.com',            last:  187.42, chg:  +1.18, pct:  +0.63, vol:'34.2M', mcap:'1.95T'},
    {sym:'META', name:'Meta Platforms',        last:  571.90, chg:  +8.44, pct:  +1.50, vol:'12.8M', mcap:'1.45T'},
    {sym:'GOOGL',name:'Alphabet Inc',          last:  171.06, chg:  -0.92, pct:  -0.53, vol:'24.6M', mcap:'2.11T'},
    {sym:'AVGO', name:'Broadcom Inc',          last: 1742.10, chg: +33.20, pct:  +1.94, vol: '4.1M', mcap:'811B'},
    {sym:'AMD',  name:'Advanced Micro',        last:  158.31, chg:  +4.05, pct:  +2.62, vol:'41.0M', mcap:'255B'},
    {sym:'JPM',  name:'JPMorgan Chase',        last:  219.84, chg:  -0.41, pct:  -0.19, vol: '8.7M', mcap:'628B'},
    {sym:'SPY',  name:'SPDR S&P 500 ETF',      last:  582.41, chg:  +2.18, pct:  +0.38, vol:'37.9M', mcap:'—'},
    {sym:'QQQ',  name:'Invesco QQQ Trust',     last:  498.76, chg:  +4.22, pct:  +0.85, vol:'28.4M', mcap:'—'},
  ];

  const heatmap = [
    {sec:'TECH',         w: 32, items:[
      {s:'NVDA',p:+2.26,w:18},{s:'AAPL',p:-0.83,w:14},{s:'MSFT',p:+0.72,w:14},
      {s:'AVGO',p:+1.94,w:8},{s:'AMD',p:+2.62,w:5},{s:'CRM',p:+0.41,w:4},
      {s:'ORCL',p:+1.10,w:4},{s:'ADBE',p:-0.22,w:3},
    ]},
    {sec:'COMMS',        w: 16, items:[
      {s:'META',p:+1.50,w:9},{s:'GOOGL',p:-0.53,w:9},{s:'NFLX',p:+0.84,w:3},{s:'DIS',p:-1.21,w:2},
    ]},
    {sec:'CONS DISC',    w: 14, items:[
      {s:'AMZN',p:+0.63,w:9},{s:'TSLA',p:-3.88,w:6},{s:'HD',p:+0.18,w:3},{s:'MCD',p:-0.42,w:2},
    ]},
    {sec:'FIN',          w: 12, items:[
      {s:'JPM',p:-0.19,w:5},{s:'BAC',p:+0.34,w:3},{s:'V',p:+0.66,w:4},{s:'MA',p:+0.51,w:3},
    ]},
    {sec:'HEALTH',       w: 10, items:[
      {s:'LLY',p:-1.04,w:5},{s:'UNH',p:+0.22,w:3},{s:'JNJ',p:-0.18,w:3},{s:'PFE',p:+1.42,w:2},
    ]},
    {sec:'ENERGY',       w:  6, items:[
      {s:'XOM',p:+1.84,w:3},{s:'CVX',p:+1.21,w:2},{s:'COP',p:+2.04,w:1},
    ]},
    {sec:'INDU',         w:  5, items:[
      {s:'CAT',p:-0.42,w:2},{s:'GE',p:+0.88,w:2},{s:'BA',p:-2.18,w:1},
    ]},
    {sec:'STAPLES',      w:  3, items:[
      {s:'PG',p:+0.18,w:1},{s:'KO',p:-0.12,w:1},{s:'WMT',p:+0.41,w:1},
    ]},
  ];

  const news = [
    {t:'09:42', src:'BBG',  head:'Nvidia surges past $1.28T as Blackwell shipments accelerate; analysts raise PT to $1,400', tag:'NVDA', tone:'pos'},
    {t:'09:38', src:'RTRS', head:'Tesla deliveries miss; Q3 guide trimmed on China softness', tag:'TSLA', tone:'neg'},
    {t:'09:35', src:'WSJ',  head:'Fed minutes show split on December cut; two members favor pause', tag:'MACRO', tone:'neu'},
    {t:'09:31', src:'CNBC', head:'Apple Vision Pro 2 production ramping for Q1 \'26 launch — supply chain note', tag:'AAPL', tone:'neu'},
    {t:'09:28', src:'BBG',  head:'AMD wins major hyperscaler deal; MI400 ramp ahead of schedule', tag:'AMD', tone:'pos'},
    {t:'09:24', src:'FT',   head:'EU antitrust opens probe into Meta\'s ad-stack acquisition', tag:'META', tone:'neg'},
    {t:'09:20', src:'RTRS', head:'Crude jumps 2.4% on OPEC+ extension speculation', tag:'OIL', tone:'pos'},
    {t:'09:16', src:'BBG',  head:'JPM strategist: \'risk-on into year-end if jobs print holds\'', tag:'MACRO', tone:'neu'},
    {t:'09:11', src:'WSJ',  head:'Broadcom signs multi-year custom-silicon deal with new hyperscaler', tag:'AVGO', tone:'pos'},
    {t:'09:04', src:'CNBC', head:'Microsoft Azure outage resolved after 47-minute disruption in US-East', tag:'MSFT', tone:'neu'},
  ];

  const positions = [
    {sym:'NVDA', side:'LONG', qty: 1200, avg: 1142.80, last: 1284.32, mv: 1541184, pl: +169824, plp: +12.39},
    {sym:'AAPL', side:'LONG', qty: 2400, avg:  198.40, last:  221.18, mv:  530832, pl:  +54672, plp: +11.48},
    {sym:'TSLA', side:'SHORT',qty:  800, avg:  281.20, last:  254.18, mv:  203344, pl:  +21616, plp:  +9.61},
    {sym:'AMD',  side:'LONG', qty: 1800, avg:  142.10, last:  158.31, mv:  284958, pl:  +29178, plp: +11.41},
    {sym:'SPY',  side:'LONG', qty:  600, avg:  561.40, last:  582.41, mv:  349446, pl:  +12606, plp:  +3.74},
    {sym:'META', side:'LONG', qty:  300, avg:  548.20, last:  571.90, mv:  171570, pl:   +7110, plp:  +4.32},
    {sym:'AMZN', side:'LONG', qty:  900, avg:  192.10, last:  187.42, mv:  168678, pl:   -4212, plp:  -2.44},
  ];

  // Order book around 1284.32
  const ob = (function(){
    const mid=1284.32, tick=0.05;
    const bids=[], asks=[];
    const r=seedRand(31);
    for(let i=0;i<14;i++){
      const px = +(mid - tick*(i+1)).toFixed(2);
      const sz = (40 + (r()*180|0)) * (i<3?3:1);
      bids.push({px, sz, total: 0});
    }
    for(let i=0;i<14;i++){
      const px = +(mid + tick*(i+1)).toFixed(2);
      const sz = (40 + (r()*180|0)) * (i<3?3:1);
      asks.push({px, sz, total: 0});
    }
    let t=0; bids.forEach(r=>{t+=r.sz; r.total=t;});
    t=0;       asks.forEach(r=>{t+=r.sz; r.total=t;});
    return {bids, asks, mid};
  })();

  // Time & sales
  const tape = (function(){
    const r=seedRand(7);
    const out=[]; let t=34829; let p=1284.32;
    for(let i=0;i<22;i++){
      const drift=(r()-0.5)*0.18;
      p = +(p+drift).toFixed(2);
      const sz = 50 + (r()*900|0);
      const side = r()>0.48 ? 'B':'S';
      const hh=9, mm=42-((i*7)/60|0)%60, ss=(t-i*7)%60;
      out.push({time:`${String(hh).padStart(2,'0')}:${String((mm+60)%60).padStart(2,'0')}:${String((ss+60)%60).padStart(2,'0')}`, px:p, sz, side});
    }
    return out;
  })();

  const candles = genCandles(120, 1180, 14, 0.85);
  // align last candle close to NVDA last
  candles[candles.length-1].c = 1284.32;
  candles[candles.length-1].h = Math.max(candles[candles.length-1].h, 1289.10);
  candles[candles.length-1].l = Math.min(candles[candles.length-1].l, 1276.40);

  const indices = [
    {n:'S&P 500',   v: 5824.10, c: +0.38, lvl:'5,824.10'},
    {n:'NASDAQ',    v:18642.20, c: +0.85, lvl:'18,642.20'},
    {n:'DOW',       v:42118.40, c: -0.12, lvl:'42,118.40'},
    {n:'RUSSELL',   v: 2284.66, c: +0.94, lvl:'2,284.66'},
    {n:'VIX',       v:   14.82, c: -3.21, lvl:'14.82'},
    {n:'10Y YIELD', v:    4.218,c: +0.041,lvl:'4.218%'},
    {n:'DXY',       v:  104.18, c: +0.18, lvl:'104.18'},
    {n:'GOLD',      v: 2714.40, c: +0.62, lvl:'2,714.40'},
    {n:'WTI',       v:   72.18, c: +2.41, lvl:'72.18'},
    {n:'BTC',       v:97184.00, c: +1.84, lvl:'97,184'},
  ];

  // Options chain for NVDA, expiry 2026-05-15 (mock)
  const optsChain = (function(){
    const r=seedRand(11);
    const strikes=[1240,1250,1260,1270,1280,1290,1300,1310,1320,1330,1340];
    return strikes.map(k=>{
      const moneyness = 1284.32 - k;
      const cBid = Math.max(0.1, 28 + moneyness*0.45 + (r()-.5)*1.4).toFixed(2);
      const cAsk = (parseFloat(cBid)+0.4+r()*0.4).toFixed(2);
      const pBid = Math.max(0.1, 28 - moneyness*0.45 + (r()-.5)*1.4).toFixed(2);
      const pAsk = (parseFloat(pBid)+0.4+r()*0.4).toFixed(2);
      const iv = (38 + Math.abs(moneyness)*0.04 + r()*2).toFixed(1);
      const cVol = (200+r()*4800|0); const pVol = (200+r()*4800|0);
      const cOI  = (1000+r()*22000|0); const pOI  = (1000+r()*22000|0);
      const delta = +(0.5 + moneyness*0.012).toFixed(2);
      return {k, cBid, cAsk, cVol, cOI, iv, pBid, pAsk, pVol, pOI, delta};
    });
  })();

  return {watchlist, heatmap, news, positions, ob, tape, candles, indices, optsChain};
})();
