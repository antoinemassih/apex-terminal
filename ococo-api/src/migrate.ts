import { pool } from './db.js'

async function migrate() {
  console.info('Running migrations...')

  await pool.query(`
    CREATE TABLE IF NOT EXISTS annotations (
      id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
      symbol          VARCHAR(20) NOT NULL,
      source          VARCHAR(30) NOT NULL DEFAULT 'user',
      type            VARCHAR(30) NOT NULL,
      points          JSONB NOT NULL DEFAULT '[]',
      style           JSONB NOT NULL DEFAULT '{}',
      strength        REAL NOT NULL DEFAULT 0.5,
      "group"         VARCHAR(50),
      tags            TEXT[] NOT NULL DEFAULT '{}',
      visibility      TEXT[] NOT NULL DEFAULT '{*}',
      timeframe       VARCHAR(10),
      ttl             TIMESTAMPTZ,
      metadata        JSONB NOT NULL DEFAULT '{}',
      created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
      updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
    )
  `)

  await pool.query(`CREATE INDEX IF NOT EXISTS idx_annotations_symbol ON annotations (symbol)`)
  await pool.query(`CREATE INDEX IF NOT EXISTS idx_annotations_symbol_source ON annotations (symbol, source)`)
  await pool.query(`CREATE INDEX IF NOT EXISTS idx_annotations_group ON annotations ("group")`)
  await pool.query(`CREATE INDEX IF NOT EXISTS idx_annotations_tags ON annotations USING GIN (tags)`)
  await pool.query(`CREATE INDEX IF NOT EXISTS idx_annotations_ttl ON annotations (ttl) WHERE ttl IS NOT NULL`)

  await pool.query(`
    CREATE TABLE IF NOT EXISTS alert_rules (
      id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
      annotation_id   UUID REFERENCES annotations(id) ON DELETE CASCADE,
      symbol          VARCHAR(20) NOT NULL,
      condition       VARCHAR(30) NOT NULL,
      price           REAL,
      active          BOOLEAN NOT NULL DEFAULT true,
      last_triggered  TIMESTAMPTZ,
      cooldown_sec    INTEGER NOT NULL DEFAULT 60,
      notify          JSONB NOT NULL DEFAULT '{"websocket": true}',
      created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
    )
  `)

  await pool.query(`CREATE INDEX IF NOT EXISTS idx_alert_rules_symbol ON alert_rules (symbol) WHERE active = true`)
  await pool.query(`CREATE INDEX IF NOT EXISTS idx_alert_rules_annotation ON alert_rules (annotation_id)`)

  // Symbols catalog: known symbols with metadata
  await pool.query(`
    CREATE TABLE IF NOT EXISTS symbols (
      symbol          VARCHAR(20) PRIMARY KEY,
      name            VARCHAR(100),
      type            VARCHAR(20) NOT NULL DEFAULT 'stock',
      exchange        VARCHAR(20),
      sector          VARCHAR(50),
      metadata        JSONB NOT NULL DEFAULT '{}',
      updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
    )
  `)
  await pool.query(`CREATE INDEX IF NOT EXISTS idx_symbols_type ON symbols (type)`)
  await pool.query(`CREATE INDEX IF NOT EXISTS idx_symbols_name_search ON symbols USING GIN (to_tsvector('english', name))`)

  // Recent symbols per user/session
  await pool.query(`
    CREATE TABLE IF NOT EXISTS recent_symbols (
      id              SERIAL PRIMARY KEY,
      session_id      VARCHAR(50) NOT NULL DEFAULT 'default',
      symbol          VARCHAR(20) NOT NULL,
      accessed_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
    )
  `)
  await pool.query(`CREATE INDEX IF NOT EXISTS idx_recent_session ON recent_symbols (session_id, accessed_at DESC)`)
  await pool.query(`CREATE UNIQUE INDEX IF NOT EXISTS idx_recent_unique ON recent_symbols (session_id, symbol)`)

  // Seed popular symbols if empty
  const symbolCount = await pool.query(`SELECT COUNT(*) FROM symbols`)
  if (parseInt(symbolCount.rows[0].count) === 0) {
    const seeds = [
      ['AAPL', 'Apple Inc.', 'stock', 'NASDAQ', 'Technology'],
      ['MSFT', 'Microsoft Corporation', 'stock', 'NASDAQ', 'Technology'],
      ['GOOG', 'Alphabet Inc.', 'stock', 'NASDAQ', 'Technology'],
      ['AMZN', 'Amazon.com Inc.', 'stock', 'NASDAQ', 'Consumer Cyclical'],
      ['META', 'Meta Platforms Inc.', 'stock', 'NASDAQ', 'Technology'],
      ['NVDA', 'NVIDIA Corporation', 'stock', 'NASDAQ', 'Technology'],
      ['TSLA', 'Tesla Inc.', 'stock', 'NASDAQ', 'Consumer Cyclical'],
      ['AMD', 'Advanced Micro Devices', 'stock', 'NASDAQ', 'Technology'],
      ['NFLX', 'Netflix Inc.', 'stock', 'NASDAQ', 'Communication'],
      ['CRM', 'Salesforce Inc.', 'stock', 'NYSE', 'Technology'],
      ['SPY', 'S&P 500 ETF', 'etf', 'NYSE', null],
      ['QQQ', 'Nasdaq-100 ETF', 'etf', 'NASDAQ', null],
      ['IWM', 'Russell 2000 ETF', 'etf', 'NYSE', null],
      ['DIA', 'Dow Jones ETF', 'etf', 'NYSE', null],
      ['VIX', 'CBOE Volatility Index', 'index', 'CBOE', null],
      ['JPM', 'JPMorgan Chase & Co.', 'stock', 'NYSE', 'Financial'],
      ['BAC', 'Bank of America Corp.', 'stock', 'NYSE', 'Financial'],
      ['GS', 'Goldman Sachs Group', 'stock', 'NYSE', 'Financial'],
      ['V', 'Visa Inc.', 'stock', 'NYSE', 'Financial'],
      ['MA', 'Mastercard Inc.', 'stock', 'NYSE', 'Financial'],
      ['XOM', 'Exxon Mobil Corporation', 'stock', 'NYSE', 'Energy'],
      ['CVX', 'Chevron Corporation', 'stock', 'NYSE', 'Energy'],
      ['COIN', 'Coinbase Global Inc.', 'stock', 'NASDAQ', 'Financial'],
      ['MARA', 'Marathon Digital Holdings', 'stock', 'NASDAQ', 'Technology'],
      ['SQ', 'Block Inc.', 'stock', 'NYSE', 'Technology'],
      ['BTC-USD', 'Bitcoin USD', 'crypto', null, null],
      ['ETH-USD', 'Ethereum USD', 'crypto', null, null],
      ['SOL-USD', 'Solana USD', 'crypto', null, null],
      ['PLTR', 'Palantir Technologies', 'stock', 'NYSE', 'Technology'],
      ['INTC', 'Intel Corporation', 'stock', 'NASDAQ', 'Technology'],
      ['UBER', 'Uber Technologies', 'stock', 'NYSE', 'Technology'],
      ['SNAP', 'Snap Inc.', 'stock', 'NYSE', 'Communication'],
      ['ROKU', 'Roku Inc.', 'stock', 'NASDAQ', 'Communication'],
      ['RIVN', 'Rivian Automotive', 'stock', 'NASDAQ', 'Consumer Cyclical'],
      ['SOFI', 'SoFi Technologies', 'stock', 'NASDAQ', 'Financial'],
      ['ARM', 'Arm Holdings', 'stock', 'NASDAQ', 'Technology'],
      ['SMCI', 'Super Micro Computer', 'stock', 'NASDAQ', 'Technology'],
      ['AVGO', 'Broadcom Inc.', 'stock', 'NASDAQ', 'Technology'],
      ['PANW', 'Palo Alto Networks', 'stock', 'NASDAQ', 'Technology'],
      ['CRWD', 'CrowdStrike Holdings', 'stock', 'NASDAQ', 'Technology'],
    ]
    for (const [sym, name, type, exchange, sector] of seeds) {
      await pool.query(
        'INSERT INTO symbols (symbol, name, type, exchange, sector) VALUES ($1, $2, $3, $4, $5) ON CONFLICT DO NOTHING',
        [sym, name, type, exchange, sector]
      )
    }
    console.info(`Seeded ${seeds.length} symbols`)
  }

  // Migrate old drawings table data if it exists
  const oldTable = await pool.query(`SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_name = 'drawings')`)
  if (oldTable.rows[0].exists) {
    const count = await pool.query(`SELECT COUNT(*) FROM drawings`)
    if (parseInt(count.rows[0].count) > 0) {
      await pool.query(`
        INSERT INTO annotations (id, symbol, source, type, points, style, timeframe, created_at, updated_at)
        SELECT
          id, symbol, 'user', type,
          points,
          jsonb_build_object('color', color, 'opacity', opacity, 'lineStyle', line_style, 'thickness', thickness),
          timeframe, created_at, updated_at
        FROM drawings
        ON CONFLICT (id) DO NOTHING
      `)
      console.info(`Migrated drawings to annotations table`)
    }
  }

  console.info('Migrations complete')
  await pool.end()
}

migrate().catch(err => {
  console.error('Migration failed:', err)
  process.exit(1)
})
