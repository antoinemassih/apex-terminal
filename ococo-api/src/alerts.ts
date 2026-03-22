import { query } from './db.js'
import type { AlertRule } from './types.js'

function rowToAlert(row: any): AlertRule {
  return {
    id: row.id,
    annotation_id: row.annotation_id,
    symbol: row.symbol,
    condition: row.condition,
    price: row.price,
    active: row.active,
    last_triggered: row.last_triggered?.toISOString() ?? null,
    cooldown_sec: row.cooldown_sec,
    notify: row.notify ?? { websocket: true },
    created_at: row.created_at?.toISOString() ?? '',
  }
}

export async function listAlerts(symbol?: string): Promise<AlertRule[]> {
  if (symbol) {
    const result = await query('SELECT * FROM alert_rules WHERE symbol = $1 ORDER BY created_at', [symbol])
    return result.rows.map(rowToAlert)
  }
  const result = await query('SELECT * FROM alert_rules ORDER BY created_at')
  return result.rows.map(rowToAlert)
}

export async function getActiveAlerts(symbol: string): Promise<AlertRule[]> {
  const result = await query(
    'SELECT * FROM alert_rules WHERE symbol = $1 AND active = true',
    [symbol]
  )
  return result.rows.map(rowToAlert)
}

export async function createAlert(alert: Partial<AlertRule> & { symbol: string; condition: string }): Promise<AlertRule> {
  const result = await query(
    `INSERT INTO alert_rules (annotation_id, symbol, condition, price, active, cooldown_sec, notify)
     VALUES ($1, $2, $3, $4, $5, $6, $7)
     RETURNING *`,
    [
      alert.annotation_id ?? null,
      alert.symbol,
      alert.condition,
      alert.price ?? null,
      alert.active ?? true,
      alert.cooldown_sec ?? 60,
      JSON.stringify(alert.notify ?? { websocket: true }),
    ]
  )
  return rowToAlert(result.rows[0])
}

export async function updateAlert(id: string, updates: Partial<AlertRule>): Promise<AlertRule | null> {
  const sets: string[] = []
  const params: any[] = [id]
  let idx = 2

  if (updates.active !== undefined) { sets.push(`active = $${idx++}`); params.push(updates.active) }
  if (updates.price !== undefined) { sets.push(`price = $${idx++}`); params.push(updates.price) }
  if (updates.condition !== undefined) { sets.push(`condition = $${idx++}`); params.push(updates.condition) }
  if (updates.cooldown_sec !== undefined) { sets.push(`cooldown_sec = $${idx++}`); params.push(updates.cooldown_sec) }
  if (updates.notify !== undefined) { sets.push(`notify = $${idx++}`); params.push(JSON.stringify(updates.notify)) }

  if (sets.length === 0) return null
  const result = await query(`UPDATE alert_rules SET ${sets.join(', ')} WHERE id = $1 RETURNING *`, params)
  return result.rows[0] ? rowToAlert(result.rows[0]) : null
}

export async function deleteAlert(id: string): Promise<void> {
  await query('DELETE FROM alert_rules WHERE id = $1', [id])
}

export async function triggerAlert(id: string): Promise<void> {
  await query('UPDATE alert_rules SET last_triggered = NOW() WHERE id = $1', [id])
}

/** Check price against all active alerts for a symbol. Returns triggered alerts. */
export async function checkAlerts(symbol: string, price: number): Promise<AlertRule[]> {
  const alerts = await getActiveAlerts(symbol)
  const triggered: AlertRule[] = []

  for (const alert of alerts) {
    if (!alert.price) continue

    // Check cooldown
    if (alert.last_triggered) {
      const elapsed = Date.now() - new Date(alert.last_triggered).getTime()
      if (elapsed < alert.cooldown_sec * 1000) continue
    }

    let hit = false
    switch (alert.condition) {
      case 'cross_above':
      case 'touch':
        hit = Math.abs(price - alert.price) / alert.price < 0.001 // within 0.1%
        break
      case 'cross_below':
        hit = Math.abs(price - alert.price) / alert.price < 0.001
        break
      case 'above':
        hit = price >= alert.price
        break
      case 'below':
        hit = price <= alert.price
        break
    }

    if (hit) {
      await triggerAlert(alert.id)
      triggered.push(alert)
    }
  }

  return triggered
}
