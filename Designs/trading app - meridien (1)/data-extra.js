// Extra mock data for new pages
window.TERMINAL_DATA_EXTRA = (function(){
  function seedRand(seed){let s=seed;return ()=>{s=(s*9301+49297)%233280;return s/233280;};}
  const r = seedRand(91);

  // Allocations for portfolio
  const allocSector = [
    {k:'Tech',         w: 42, pl: +12.4, c:'#d6552b'},
    {k:'Comms',        w: 14, pl:  +1.8, c:'#1f6f3b'},
    {k:'Cons. Disc.',  w: 12, pl:  -0.4, c:'#ffd24d'},
    {k:'Financials',   w: 10, pl:  +0.6, c:'#2557d6'},
    {k:'Healthcare',   w:  8, pl:  +1.2, c:'#8a8675'},
    {k:'Energy',       w:  6, pl:  +2.4, c:'#14140f'},
    {k:'Industrials',  w:  4, pl:  -1.1, c:'#b3ad9c'},
    {k:'Cash',         w:  4, pl:   0.0, c:'#e3dccd'},
  ];

  const perfHistory = (function(){
    const out=[]; let p=100; let bench=100;
    for(let i=0;i<252;i++){
      p += (r()-0.45)*1.2 + 0.04;
      bench += (r()-0.46)*0.9 + 0.03;
      out.push({d:i, p:+p.toFixed(2), b:+bench.toFixed(2)});
    }
    return out;
  })();

  // Risk metrics
  const riskScenarios = [
    {k:'Market −1σ',   pl: -184230, pct: -3.18},
    {k:'Market −2σ',   pl: -421180, pct: -7.24},
    {k:'Market −3σ',   pl: -742400, pct:-12.78},
    {k:'Tech sell-off',pl: -298400, pct: -5.14},
    {k:'Rate +50bp',   pl: -118200, pct: -2.04},
    {k:'VIX → 28',     pl: -242100, pct: -4.18},
    {k:'USD +5%',      pl:  -84200, pct: -1.45},
    {k:'Oil +20%',     pl:  +44800, pct: +0.77},
  ];

  const riskGreeks = [
    {k:'Beta',     v:'1.18', n:'vs SPX'},
    {k:'Vega',     v:'+8,420', n:'$/vol pt'},
    {k:'Theta',    v:'−1,284', n:'$/day'},
    {k:'Delta $',  v:'+4.21M', n:'net'},
    {k:'Gamma',    v:'+0.084', n:''},
    {k:'Rho',      v:'−2,180', n:'$/bp'},
  ];

  const exposureBySym = [
    {sym:'NVDA', long: 1.54, short:0,    net:+1.54},
    {sym:'AAPL', long: 0.53, short:0,    net:+0.53},
    {sym:'TSLA', long: 0,    short:0.20, net:-0.20},
    {sym:'AMD',  long: 0.28, short:0,    net:+0.28},
    {sym:'SPY',  long: 0.35, short:0,    net:+0.35},
    {sym:'META', long: 0.17, short:0,    net:+0.17},
    {sym:'AMZN', long: 0.17, short:0,    net:+0.17},
  ];

  // Research analyst picks
  const ratings = [
    {sym:'NVDA', firm:'Goldman',     act:'Buy',     pt:1450, prev:1380, date:'04-25'},
    {sym:'AAPL', firm:'Morgan Stanley', act:'Equal-weight', pt:230, prev:225, date:'04-24'},
    {sym:'TSLA', firm:'JPMorgan',    act:'Underweight', pt:180, prev:200, date:'04-22'},
    {sym:'MSFT', firm:'BofA',        act:'Buy',     pt:480, prev:455, date:'04-22'},
    {sym:'META', firm:'Wedbush',     act:'Outperform', pt:625, prev:580, date:'04-19'},
    {sym:'AVGO', firm:'Citi',        act:'Buy',     pt:1900, prev:1820, date:'04-18'},
  ];

  const earnings = [
    {sym:'NVDA', date:'May 28', ept:'4.42', actl:'4.61', surp:+4.3, rev:'30.2B'},
    {sym:'TSLA', date:'May 21', ept:'0.61', actl:'0.54', surp:-11.5, rev:'24.1B'},
    {sym:'AAPL', date:'May 02', ept:'1.52', actl:'1.55', surp:+2.0, rev:'95.4B'},
    {sym:'MSFT', date:'May 04', ept:'2.78', actl:'2.94', surp:+5.8, rev:'68.4B'},
    {sym:'META', date:'May 01', ept:'4.42', actl:'4.71', surp:+6.6, rev:'42.3B'},
  ];

  // Account dropdown data
  const accounts = [
    {id:'JM-MAIN', name:'Main · Margin', value:'$5.83M', flat:'+1.7%'},
    {id:'JM-IRA',  name:'Roth IRA',      value:'$1.12M', flat:'+0.8%'},
    {id:'JM-401K', name:'401(k)',        value:'$842K',  flat:'+0.4%'},
    {id:'JM-HFD',  name:'Hedge Fund · D-share', value:'$28.4M', flat:'+2.1%'},
  ];

  const alerts = [
    {t:'09:42', sym:'NVDA', msg:'Crossed above $1,280 trigger', tone:'pos'},
    {t:'09:38', sym:'TSLA', msg:'Stop-loss armed at $250', tone:'neg'},
    {t:'09:14', sym:'PORT', msg:'Daily P&L exceeded +$250K threshold', tone:'pos'},
    {t:'08:55', sym:'VIX',  msg:'Dropped below 15.0', tone:'pos'},
    {t:'08:30', sym:'MACRO',msg:'CPI release in 30m — auto-hedge ready', tone:'neu'},
  ];

  return {allocSector, perfHistory, riskScenarios, riskGreeks, exposureBySym, ratings, earnings, accounts, alerts};
})();
