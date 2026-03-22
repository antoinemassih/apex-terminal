import { query } from './db.js'

export interface SymbolInfo {
  symbol: string
  name: string | null
  type: string
  exchange: string | null
  sector: string | null
}

export interface RecentSymbol {
  symbol: string
  name: string | null
  accessed_at: string
}

/** Search symbols by prefix or name substring */
export async function searchSymbols(q: string, limit = 20): Promise<SymbolInfo[]> {
  const upper = q.toUpperCase()
  const result = await query(
    `SELECT symbol, name, type, exchange, sector FROM symbols
     WHERE symbol ILIKE $1 OR name ILIKE $2
     ORDER BY
       CASE WHEN symbol = $3 THEN 0
            WHEN symbol ILIKE $4 THEN 1
            ELSE 2 END,
       symbol
     LIMIT $5`,
    [`%${upper}%`, `%${q}%`, upper, `${upper}%`, limit]
  )
  return result.rows
}

/** Get all symbols (for browsing) */
export async function listSymbols(type?: string): Promise<SymbolInfo[]> {
  if (type) {
    const result = await query('SELECT symbol, name, type, exchange, sector FROM symbols WHERE type = $1 ORDER BY symbol', [type])
    return result.rows
  }
  const result = await query('SELECT symbol, name, type, exchange, sector FROM symbols ORDER BY symbol')
  return result.rows
}

/** Add or update a symbol in the catalog */
export async function upsertSymbol(info: SymbolInfo): Promise<void> {
  await query(
    `INSERT INTO symbols (symbol, name, type, exchange, sector, updated_at)
     VALUES ($1, $2, $3, $4, $5, NOW())
     ON CONFLICT (symbol) DO UPDATE SET
       name = COALESCE(EXCLUDED.name, symbols.name),
       type = EXCLUDED.type,
       exchange = COALESCE(EXCLUDED.exchange, symbols.exchange),
       sector = COALESCE(EXCLUDED.sector, symbols.sector),
       updated_at = NOW()`,
    [info.symbol.toUpperCase(), info.name, info.type, info.exchange, info.sector]
  )
}

/** Get recent symbols for a session */
export async function getRecents(sessionId = 'default', limit = 20): Promise<RecentSymbol[]> {
  const result = await query(
    `SELECT r.symbol, s.name, r.accessed_at
     FROM recent_symbols r
     LEFT JOIN symbols s ON s.symbol = r.symbol
     WHERE r.session_id = $1
     ORDER BY r.accessed_at DESC
     LIMIT $2`,
    [sessionId, limit]
  )
  return result.rows.map(r => ({
    symbol: r.symbol,
    name: r.name,
    accessed_at: r.accessed_at?.toISOString() ?? '',
  }))
}

/** Record a symbol access (upsert into recents) */
export async function touchRecent(symbol: string, sessionId = 'default'): Promise<void> {
  await query(
    `INSERT INTO recent_symbols (session_id, symbol, accessed_at)
     VALUES ($1, $2, NOW())
     ON CONFLICT (session_id, symbol)
     DO UPDATE SET accessed_at = NOW()`,
    [sessionId, symbol.toUpperCase()]
  )
  // Also ensure the symbol exists in the catalog (auto-add unknown symbols)
  await query(
    `INSERT INTO symbols (symbol, type) VALUES ($1, 'unknown') ON CONFLICT DO NOTHING`,
    [symbol.toUpperCase()]
  )
}
