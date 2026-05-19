// Mock data for the trading app

const TICKER_BAR = [
  { sym: 'S&P 500',  px: '5,824.10',  chg: '+0.38%', dir: 'up' },
  { sym: 'NASDAQ',   px: '18,642.20', chg: '+0.85%', dir: 'up' },
  { sym: 'DOW',      px: '42,118.40', chg: '−0.12%', dir: 'down' },
  { sym: 'RUSSELL',  px: '2,384.94',  chg: '+0.94%', dir: 'up' },
  { sym: 'VIX',      px: '14.82',     chg: '−3.21%', dir: 'down' },
  { sym: '10Y',      px: '4.218%',    chg: '+0.04%', dir: 'up' },
  { sym: 'DXY',      px: '104.18',    chg: '+0.18%', dir: 'up' },
  { sym: 'GOLD',     px: '2,714.40',  chg: '+0.41%', dir: 'up' },
  { sym: 'BTC',      px: '94,218.10', chg: '+1.84%', dir: 'up' },
];

const WATCHLIST = [
  { sym: 'NVDA', name: 'NVIDIA Corp',         px: 1284.32, chg: +2.26, vol: '42.1M' },
  { sym: 'AAPL', name: 'Apple Inc',           px: 221.18,  chg: -0.83, vol: '38.4M' },
  { sym: 'MSFT', name: 'Microsoft Corp',      px: 438.94,  chg: +0.72, vol: '21.0M' },
  { sym: 'TSLA', name: 'Tesla Inc',           px: 254.18,  chg: -3.88, vol: '88.2M' },
  { sym: 'AMZN', name: 'Amazon.com',          px: 187.42,  chg: +0.83, vol: '32.1M' },
  { sym: 'META', name: 'Meta Platforms',      px: 571.90,  chg: +1.50, vol: '12.8M' },
  { sym: 'GOOGL',name: 'Alphabet',            px: 171.06,  chg: -0.55, vol: '18.6M' },
  { sym: 'AVGO', name: 'Broadcom',            px: 1742.10, chg: +1.04, vol: '4.20M' },
  { sym: 'AMD',  name: 'Adv. Micro Devices',  px: 168.42,  chg: +3.12, vol: '52.4M' },
  { sym: 'JPM',  name: 'JPMorgan Chase',      px: 248.18,  chg: +0.42, vol: '8.10M' },
  { sym: 'V',    name: 'Visa Inc',            px: 312.04,  chg: -0.18, vol: '5.40M' },
  { sym: 'NFLX', name: 'Netflix',             px: 894.30,  chg: +2.04, vol: '4.80M' },
];

const POSITIONS = [
  { sym: 'NVDA', name: 'NVIDIA Corp',     qty: 240,  avg: 982.40,  px: 1284.32, mv: 308237, pnl: +72460,  pnlPct: +30.74 },
  { sym: 'AAPL', name: 'Apple Inc',       qty: 1200, avg: 198.20,  px: 221.18,  mv: 265416, pnl: +27576,  pnlPct: +11.59 },
  { sym: 'MSFT', name: 'Microsoft Corp',  qty: 480,  avg: 412.10,  px: 438.94,  mv: 210691, pnl: +12883,  pnlPct: +6.51 },
  { sym: 'TSLA', name: 'Tesla Inc',       qty: 320,  avg: 268.40,  px: 254.18,  mv:  81338, pnl: -4550,   pnlPct: -5.30 },
  { sym: 'META', name: 'Meta Platforms',  qty: 180,  avg: 504.20,  px: 571.90,  mv: 102942, pnl: +12186,  pnlPct: +13.42 },
  { sym: 'AMZN', name: 'Amazon.com',      qty: 400,  avg: 178.90,  px: 187.42,  mv:  74968, pnl: +3408,   pnlPct: +4.76 },
  { sym: 'GOOGL',name: 'Alphabet',        qty: 600,  avg: 168.40,  px: 171.06,  mv: 102636, pnl: +1596,   pnlPct: +1.58 },
  { sym: 'AVGO', name: 'Broadcom',        qty: 80,   avg: 1602.10, px: 1742.10, mv: 139368, pnl: +11200,  pnlPct: +8.74 },
];

const NEWS = [
  { time: '23:18', src: 'BBG',  head: 'NVIDIA Q3 revenue tops estimates as data center demand accelerates' },
  { time: '23:04', src: 'RTRS', head: 'Fed minutes: officials see gradual rate path through Q2' },
  { time: '22:51', src: 'WSJ',  head: 'Tesla cuts production targets in Shanghai gigafactory' },
  { time: '22:40', src: 'CNBC', head: 'Oil falls to $68 as inventories rise, OPEC+ holds output' },
  { time: '22:31', src: 'BBG',  head: 'Apple Vision Pro 2 launches; pre-orders begin Friday' },
  { time: '22:18', src: 'FT',   head: 'JPMorgan boosts FY guidance, Dimon flags soft landing' },
];

const ALLOCATION = [
  { label: 'US Equities',     pct: 58, color: '#1ed760' },
  { label: 'Intl. Equities',  pct: 14, color: '#7c3aed' },
  { label: 'Fixed Income',    pct: 12, color: '#3b82f6' },
  { label: 'Commodities',     pct: 8,  color: '#f59e0b' },
  { label: 'Crypto',          pct: 5,  color: '#ec4899' },
  { label: 'Cash',            pct: 3,  color: '#6b7280' },
];

const ORDER_BOOK_BIDS = [
  { px: 1284.32, sz: 1240 },
  { px: 1284.27, sz: 880 },
  { px: 1284.22, sz: 1502 },
  { px: 1284.17, sz: 720 },
  { px: 1284.12, sz: 540 },
  { px: 1284.07, sz: 312 },
  { px: 1284.02, sz: 1820 },
  { px: 1283.97, sz: 460 },
  { px: 1283.92, sz: 380 },
  { px: 1283.87, sz: 220 },
  { px: 1283.82, sz: 905 },
  { px: 1283.77, sz: 144 },
];
const ORDER_BOOK_ASKS = [
  { px: 1284.42, sz: 612 },
  { px: 1284.47, sz: 880 },
  { px: 1284.52, sz: 1408 },
  { px: 1284.57, sz: 220 },
  { px: 1284.62, sz: 1080 },
  { px: 1284.67, sz: 442 },
  { px: 1284.72, sz: 312 },
  { px: 1284.77, sz: 720 },
  { px: 1284.82, sz: 880 },
  { px: 1284.87, sz: 240 },
  { px: 1284.92, sz: 1620 },
  { px: 1284.97, sz: 380 },
];

// Deterministic PRNG so candle layouts don't reflow on render
function mulberry32(seed) {
  return function() {
    let t = seed += 0x6D2B79F5;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function generateCandles(count, seed, startPrice = 1180, drift = 0.0008, vol = 0.012) {
  const rng = mulberry32(seed);
  const out = [];
  let price = startPrice;
  for (let i = 0; i < count; i++) {
    const open = price;
    const change = (rng() - 0.45) * vol * price + drift * price;
    const close = open + change;
    const high = Math.max(open, close) + rng() * vol * 0.5 * price;
    const low  = Math.min(open, close) - rng() * vol * 0.5 * price;
    const volume = 0.4 + rng() * 0.8;
    out.push({ open, close, high, low, volume, up: close >= open });
    price = close;
  }
  return out;
}

const NVDA_CANDLES = generateCandles(80, 7, 1180, 0.0012, 0.014);
const AAPL_CANDLES = generateCandles(80, 13, 210, 0.0006, 0.010);
const SPY_CANDLES  = generateCandles(40, 23, 514,  0.0006, 0.008);

// Trade plays — like cards in a deck. Each is a thesis with entry/target/stop.
const PLAYS = [
  {
    id: 'p1',
    sym: 'NVDA',
    name: 'NVIDIA breakout',
    side: 'LONG',
    thesis: 'Cup-and-handle on the daily; AI capex orders for FY26 came in 14% above the street.',
    tags: ['Breakout', 'Earnings', 'AI'],
    entry: 1268, target: 1420, stop: 1208,
    rr: 2.5, conviction: 4, hits: 0, status: 'fresh',
    posted: '2h ago', author: 'desk',
    series: [1180,1190,1185,1210,1230,1215,1240,1255,1260,1278,1284,1290],
    accent: '#1ed760',
  },
  {
    id: 'p2',
    sym: 'TSLA',
    name: 'TSLA rejection',
    side: 'SHORT',
    thesis: 'Price rejected the 200-DMA on rising volume; Shanghai output cut signals soft demand.',
    tags: ['Reversal', 'Momentum'],
    entry: 256, target: 228, stop: 268,
    rr: 2.3, conviction: 3, hits: 1, status: 'active',
    posted: '5h ago', author: 'jm',
    series: [268,270,266,262,265,260,258,255,254,252,254],
    accent: '#f15e6c',
  },
  {
    id: 'p3',
    sym: 'AMD',
    name: 'AMD MI400 cycle',
    side: 'LONG',
    thesis: 'Channel partners checks confirm MI400 ramp Q1\'26. Trading at a 22% discount to NVDA on PEG.',
    tags: ['Catalyst', 'AI', 'Semis'],
    entry: 165, target: 198, stop: 156,
    rr: 3.7, conviction: 5, hits: 2, status: 'active',
    posted: '1d ago', author: 'desk',
    series: [158,160,159,162,164,166,165,167,168,168],
    accent: '#7c3aed',
  },
  {
    id: 'p4',
    sym: 'META',
    name: 'META Reels monetization',
    side: 'LONG',
    thesis: 'CPMs inflected 3 quarters running, Reality Labs burn flat. Fair value $640.',
    tags: ['Fundamental', 'Catalyst'],
    entry: 568, target: 640, stop: 542,
    rr: 2.8, conviction: 4, hits: 0, status: 'fresh',
    posted: '3h ago', author: 'desk',
    series: [550,555,560,558,562,566,570,568,569,571,571],
    accent: '#3b82f6',
  },
  {
    id: 'p5',
    sym: 'XLE',
    name: 'Energy mean reversion',
    side: 'LONG',
    thesis: 'OPEC+ extension + winter supply tightness; ETF oversold on RSI(2).',
    tags: ['Mean reversion', 'Macro'],
    entry: 92.40, target: 99.20, stop: 89.50,
    rr: 2.3, conviction: 3, hits: 0, status: 'fresh',
    posted: '6h ago', author: 'macro',
    series: [94,93.5,93,92.8,92.4,92.2,92,92.4,92.6,92.4,92.5],
    accent: '#f59e0b',
  },
  {
    id: 'p6',
    sym: 'COIN',
    name: 'Coinbase BTC beta',
    side: 'LONG',
    thesis: 'Highest-beta liquid crypto proxy; ETF flows + halving setup into Q1.',
    tags: ['Crypto', 'Beta', 'High R/R'],
    entry: 268, target: 360, stop: 244,
    rr: 3.8, conviction: 4, hits: 0, status: 'fresh',
    posted: '8h ago', author: 'jm',
    series: [248,252,260,265,262,268,270,275,272,268,270],
    accent: '#ec4899',
  },
];

// Pending alerts the user has set
const ALERTS = [
  { id:'a1', sym:'NVDA', cond:'crosses above', val:1300, status:'armed', set:'2h ago', kind:'price' },
  { id:'a2', sym:'TSLA', cond:'falls below', val:250, status:'armed', set:'1d ago', kind:'price' },
  { id:'a3', sym:'AAPL', cond:'RSI(14) < ', val:30, status:'armed', set:'4h ago', kind:'indicator' },
  { id:'a4', sym:'AMD',  cond:'volume spike', val:'2× avg', status:'triggered', set:'08:42', kind:'volume' },
  { id:'a5', sym:'BTC',  cond:'crosses above', val:'$96,000', status:'armed', set:'6h ago', kind:'price' },
  { id:'a6', sym:'SPY',  cond:'gap up >', val:'1%', status:'armed', set:'12h ago', kind:'event' },
  { id:'a7', sym:'META', cond:'news mentions', val:'"Reality Labs"', status:'armed', set:'3d ago', kind:'news' },
];

// Sector heatmap
const SECTORS = [
  { sym:'TECH',  name:'Technology',          chg:+1.84, w:28 },
  { sym:'COMM',  name:'Communication',       chg:+1.22, w:14 },
  { sym:'DISC',  name:'Cons. Discretionary', chg:+0.62, w:11 },
  { sym:'FIN',   name:'Financials',          chg:+0.18, w:14 },
  { sym:'HLTH',  name:'Healthcare',          chg:-0.42, w:13 },
  { sym:'IND',   name:'Industrials',         chg:+0.34, w:9  },
  { sym:'ENGY',  name:'Energy',              chg:-1.18, w:5  },
  { sym:'STAP',  name:'Staples',             chg:-0.21, w:5  },
  { sym:'UTIL',  name:'Utilities',           chg:-0.62, w:3  },
  { sym:'MAT',   name:'Materials',           chg:+0.08, w:4  },
  { sym:'RE',    name:'Real Estate',         chg:-0.84, w:4  },
];

// Earnings calendar (next 7 sessions)
const EARNINGS = [
  { date:'Mon', d:13, sym:'CRWD', t:'AMC', est:'$0.94', impl:'±8.4%' },
  { date:'Tue', d:14, sym:'WMT',  t:'BMO', est:'$0.61', impl:'±4.1%' },
  { date:'Tue', d:14, sym:'HD',   t:'BMO', est:'$3.34', impl:'±3.8%' },
  { date:'Wed', d:15, sym:'NVDA', t:'AMC', est:'$0.78', impl:'±9.2%' },
  { date:'Wed', d:15, sym:'TGT',  t:'BMO', est:'$1.85', impl:'±6.4%' },
  { date:'Thu', d:16, sym:'PANW', t:'AMC', est:'$1.40', impl:'±7.1%' },
  { date:'Fri', d:17, sym:'BABA', t:'BMO', est:'$2.10', impl:'±5.8%' },
];

// Screener — gappers/movers
const SCREENER = [
  { sym:'NVDA', name:'NVIDIA',         px:1284.32, chg:+2.26, vol:'42.1M', mc:'3.14T', pe:68.2, sec:'Semis',  signal:'Breakout' },
  { sym:'AMD',  name:'Adv Micro Dev',  px:168.42,  chg:+3.12, vol:'52.4M', mc:'272B',  pe:62.0, sec:'Semis',  signal:'Volume spike' },
  { sym:'AVGO', name:'Broadcom',       px:1742.10, chg:+1.04, vol:'4.20M', mc:'808B',  pe:38.4, sec:'Semis',  signal:'52W high' },
  { sym:'PLTR', name:'Palantir',       px:64.20,   chg:+4.18, vol:'68.0M', mc:'138B',  pe:148.0,sec:'Software',signal:'Squeeze' },
  { sym:'NFLX', name:'Netflix',        px:894.30,  chg:+2.04, vol:'4.80M', mc:'380B',  pe:48.2, sec:'Media',  signal:'Breakout' },
  { sym:'COIN', name:'Coinbase',       px:268.40,  chg:+5.84, vol:'18.2M', mc:'66B',   pe:42.0, sec:'Crypto', signal:'High R/R' },
  { sym:'MSTR', name:'MicroStrategy',  px:392.20,  chg:+6.42, vol:'12.4M', mc:'88B',   pe:'—',  sec:'Crypto', signal:'Volume spike' },
  { sym:'SMCI', name:'Super Micro',    px:48.10,   chg:+8.20, vol:'104M',  mc:'28B',   pe:18.4, sec:'AI',     signal:'Earnings' },
  { sym:'ARM',  name:'Arm Holdings',   px:148.20,  chg:-3.24, vol:'8.40M', mc:'156B',  pe:88.4, sec:'Semis',  signal:'Reversal' },
  { sym:'TSLA', name:'Tesla',          px:254.18,  chg:-3.88, vol:'88.2M', mc:'810B',  pe:74.2, sec:'Auto',   signal:'Reversal' },
  { sym:'NIO',  name:'NIO Inc',        px:5.20,    chg:-4.62, vol:'42.0M', mc:'10.4B', pe:'—',  sec:'Auto',   signal:'52W low' },
  { sym:'PYPL', name:'PayPal',         px:78.40,   chg:-2.04, vol:'14.2M', mc:'82B',   pe:18.4, sec:'Fintech',signal:'Reversal' },
];

// 30-day P&L for calendar heatmap
const PNL_DAILY = (() => {
  const rng = mulberry32(42);
  return Array.from({length:30}, (_, i) => ({
    d: i + 1,
    pnl: Math.round((rng() - 0.42) * 18000),
  }));
})();

Object.assign(window, {
  TICKER_BAR, WATCHLIST, POSITIONS, NEWS, ALLOCATION,
  ORDER_BOOK_BIDS, ORDER_BOOK_ASKS,
  NVDA_CANDLES, AAPL_CANDLES, SPY_CANDLES,
  PLAYS, ALERTS, SECTORS, EARNINGS, SCREENER, PNL_DAILY,
  generateCandles,
});
