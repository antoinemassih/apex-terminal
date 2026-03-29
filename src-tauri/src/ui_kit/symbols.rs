//! Symbol catalog + search for the stock selector.

pub struct SymbolInfo {
    pub symbol: &'static str,
    pub name: &'static str,
}

/// Search symbols — returns up to `limit` results ranked: exact > prefix > contains
pub fn search_symbols(query: &str, limit: usize) -> Vec<&'static SymbolInfo> {
    if query.is_empty() { return SYMBOLS.iter().take(limit).collect(); }
    let q = query.to_uppercase();
    let mut exact = Vec::new();
    let mut prefix = Vec::new();
    let mut contains = Vec::new();

    for s in SYMBOLS.iter() {
        if s.symbol == q { exact.push(s); }
        else if s.symbol.starts_with(&q) { prefix.push(s); }
        else if s.name.to_uppercase().contains(&q) { contains.push(s); }
    }

    let mut result = Vec::new();
    for v in [exact, prefix, contains] {
        for s in v { if result.len() < limit && !result.iter().any(|r: &&SymbolInfo| r.symbol == s.symbol) { result.push(s); } }
    }
    result
}

pub const SYMBOLS: &[SymbolInfo] = &[
    // Mega-cap tech
    SymbolInfo{symbol:"AAPL",name:"Apple"},SymbolInfo{symbol:"MSFT",name:"Microsoft"},SymbolInfo{symbol:"NVDA",name:"Nvidia"},
    SymbolInfo{symbol:"GOOGL",name:"Alphabet A"},SymbolInfo{symbol:"GOOG",name:"Alphabet C"},SymbolInfo{symbol:"AMZN",name:"Amazon"},
    SymbolInfo{symbol:"META",name:"Meta Platforms"},SymbolInfo{symbol:"TSLA",name:"Tesla"},SymbolInfo{symbol:"AVGO",name:"Broadcom"},
    SymbolInfo{symbol:"ORCL",name:"Oracle"},SymbolInfo{symbol:"CRM",name:"Salesforce"},SymbolInfo{symbol:"AMD",name:"AMD"},
    SymbolInfo{symbol:"INTC",name:"Intel"},SymbolInfo{symbol:"NFLX",name:"Netflix"},
    // Financials
    SymbolInfo{symbol:"JPM",name:"JP Morgan"},SymbolInfo{symbol:"BAC",name:"Bank of America"},SymbolInfo{symbol:"WFC",name:"Wells Fargo"},
    SymbolInfo{symbol:"GS",name:"Goldman Sachs"},SymbolInfo{symbol:"MS",name:"Morgan Stanley"},SymbolInfo{symbol:"C",name:"Citigroup"},
    SymbolInfo{symbol:"BLK",name:"BlackRock"},SymbolInfo{symbol:"SCHW",name:"Schwab"},SymbolInfo{symbol:"AXP",name:"Amex"},
    SymbolInfo{symbol:"V",name:"Visa"},SymbolInfo{symbol:"MA",name:"Mastercard"},SymbolInfo{symbol:"PYPL",name:"PayPal"},
    // Healthcare
    SymbolInfo{symbol:"LLY",name:"Eli Lilly"},SymbolInfo{symbol:"UNH",name:"UnitedHealth"},SymbolInfo{symbol:"JNJ",name:"Johnson & Johnson"},
    SymbolInfo{symbol:"ABBV",name:"AbbVie"},SymbolInfo{symbol:"MRK",name:"Merck"},SymbolInfo{symbol:"PFE",name:"Pfizer"},
    SymbolInfo{symbol:"TMO",name:"Thermo Fisher"},SymbolInfo{symbol:"ABT",name:"Abbott"},
    // Consumer/Retail
    SymbolInfo{symbol:"WMT",name:"Walmart"},SymbolInfo{symbol:"COST",name:"Costco"},SymbolInfo{symbol:"TGT",name:"Target"},
    SymbolInfo{symbol:"HD",name:"Home Depot"},SymbolInfo{symbol:"LOW",name:"Lowe's"},SymbolInfo{symbol:"NKE",name:"Nike"},
    SymbolInfo{symbol:"SBUX",name:"Starbucks"},SymbolInfo{symbol:"MCD",name:"McDonald's"},SymbolInfo{symbol:"KO",name:"Coca-Cola"},
    SymbolInfo{symbol:"PEP",name:"PepsiCo"},SymbolInfo{symbol:"PG",name:"Procter & Gamble"},
    // Energy
    SymbolInfo{symbol:"XOM",name:"Exxon Mobil"},SymbolInfo{symbol:"CVX",name:"Chevron"},SymbolInfo{symbol:"COP",name:"ConocoPhillips"},
    SymbolInfo{symbol:"SLB",name:"Schlumberger"},SymbolInfo{symbol:"OXY",name:"Occidental"},
    // Industrials
    SymbolInfo{symbol:"CAT",name:"Caterpillar"},SymbolInfo{symbol:"DE",name:"Deere"},SymbolInfo{symbol:"BA",name:"Boeing"},
    SymbolInfo{symbol:"RTX",name:"RTX"},SymbolInfo{symbol:"LMT",name:"Lockheed Martin"},SymbolInfo{symbol:"GE",name:"GE Aerospace"},
    SymbolInfo{symbol:"HON",name:"Honeywell"},SymbolInfo{symbol:"UPS",name:"UPS"},
    // ETFs — Broad
    SymbolInfo{symbol:"SPY",name:"S&P 500 ETF"},SymbolInfo{symbol:"QQQ",name:"Nasdaq 100 ETF"},SymbolInfo{symbol:"IWM",name:"Russell 2000 ETF"},
    SymbolInfo{symbol:"DIA",name:"Dow Jones ETF"},SymbolInfo{symbol:"VTI",name:"Total Stock Market"},SymbolInfo{symbol:"VOO",name:"Vanguard S&P 500"},
    // ETFs — Sector
    SymbolInfo{symbol:"XLK",name:"Tech Select"},SymbolInfo{symbol:"XLF",name:"Financial Select"},SymbolInfo{symbol:"XLV",name:"Health Care Select"},
    SymbolInfo{symbol:"XLE",name:"Energy Select"},SymbolInfo{symbol:"XLI",name:"Industrial Select"},SymbolInfo{symbol:"XLRE",name:"Real Estate Select"},
    // ETFs — Fixed income / Volatility / Commodity
    SymbolInfo{symbol:"TLT",name:"20+ Year Treasury"},SymbolInfo{symbol:"IEF",name:"7-10 Year Treasury"},SymbolInfo{symbol:"SHY",name:"1-3 Year Treasury"},
    SymbolInfo{symbol:"GLD",name:"Gold"},SymbolInfo{symbol:"SLV",name:"Silver"},SymbolInfo{symbol:"USO",name:"Oil"},
    SymbolInfo{symbol:"VXX",name:"VIX Short-Term"},
    // Leveraged
    SymbolInfo{symbol:"TQQQ",name:"3x Nasdaq Bull"},SymbolInfo{symbol:"SQQQ",name:"3x Nasdaq Bear"},
    SymbolInfo{symbol:"SPXL",name:"3x S&P Bull"},SymbolInfo{symbol:"SPXS",name:"3x S&P Bear"},
    SymbolInfo{symbol:"SOXL",name:"3x Semis Bull"},SymbolInfo{symbol:"SOXS",name:"3x Semis Bear"},
    // Crypto ETFs
    SymbolInfo{symbol:"IBIT",name:"iShares Bitcoin"},SymbolInfo{symbol:"FBTC",name:"Fidelity Bitcoin"},
    SymbolInfo{symbol:"GBTC",name:"Grayscale Bitcoin"},SymbolInfo{symbol:"ETHA",name:"iShares Ethereum"},
    // Others
    SymbolInfo{symbol:"TSM",name:"TSMC"},SymbolInfo{symbol:"ASML",name:"ASML"},SymbolInfo{symbol:"BABA",name:"Alibaba"},
    SymbolInfo{symbol:"NIO",name:"NIO"},SymbolInfo{symbol:"F",name:"Ford"},SymbolInfo{symbol:"GM",name:"General Motors"},
    SymbolInfo{symbol:"RIVN",name:"Rivian"},SymbolInfo{symbol:"PLTR",name:"Palantir"},SymbolInfo{symbol:"COIN",name:"Coinbase"},
    SymbolInfo{symbol:"SQ",name:"Block"},SymbolInfo{symbol:"SHOP",name:"Shopify"},SymbolInfo{symbol:"SNOW",name:"Snowflake"},
    SymbolInfo{symbol:"UBER",name:"Uber"},SymbolInfo{symbol:"ABNB",name:"Airbnb"},SymbolInfo{symbol:"DASH",name:"DoorDash"},
    SymbolInfo{symbol:"CRWD",name:"CrowdStrike"},SymbolInfo{symbol:"ZS",name:"Zscaler"},SymbolInfo{symbol:"PANW",name:"Palo Alto"},
    SymbolInfo{symbol:"DDOG",name:"Datadog"},SymbolInfo{symbol:"NET",name:"Cloudflare"},
    SymbolInfo{symbol:"DIS",name:"Disney"},SymbolInfo{symbol:"CMCSA",name:"Comcast"},SymbolInfo{symbol:"T",name:"AT&T"},
    SymbolInfo{symbol:"VZ",name:"Verizon"},SymbolInfo{symbol:"TMUS",name:"T-Mobile"},
];
