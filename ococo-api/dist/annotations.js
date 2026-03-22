import { query } from './db.js';
import { invalidate } from './cache.js';
import { getCached, setCache } from './cache.js';
function rowToAnnotation(row) {
    return {
        id: row.id,
        symbol: row.symbol,
        source: row.source,
        type: row.type,
        points: row.points ?? [],
        style: row.style ?? {},
        strength: row.strength,
        group: row.group,
        tags: row.tags ?? [],
        visibility: row.visibility ?? ['*'],
        timeframe: row.timeframe,
        ttl: row.ttl?.toISOString() ?? null,
        metadata: row.metadata ?? {},
        created_at: row.created_at?.toISOString() ?? '',
        updated_at: row.updated_at?.toISOString() ?? '',
    };
}
export async function listAnnotations(filter) {
    // Try cache for simple symbol-only queries
    if (filter.symbol && !filter.source && !filter.group && !filter.tags && !filter.type) {
        const cached = await getCached(filter.symbol);
        if (cached)
            return cached;
    }
    const conditions = [];
    const params = [];
    let idx = 1;
    if (filter.symbol) {
        conditions.push(`symbol = $${idx++}`);
        params.push(filter.symbol);
    }
    if (filter.source) {
        conditions.push(`source = $${idx++}`);
        params.push(filter.source);
    }
    if (filter.group) {
        conditions.push(`"group" = $${idx++}`);
        params.push(filter.group);
    }
    if (filter.type) {
        conditions.push(`type = $${idx++}`);
        params.push(filter.type);
    }
    if (filter.tags?.length) {
        conditions.push(`tags && $${idx++}`);
        params.push(filter.tags);
    }
    // Exclude expired annotations
    conditions.push(`(ttl IS NULL OR ttl > NOW())`);
    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    const result = await query(`SELECT * FROM annotations ${where} ORDER BY created_at`, params);
    const annotations = result.rows.map(rowToAnnotation);
    // Cache if it was a symbol-only query
    if (filter.symbol && !filter.source && !filter.group && !filter.tags && !filter.type) {
        await setCache(filter.symbol, annotations);
    }
    return annotations;
}
export async function getAnnotation(id) {
    const result = await query('SELECT * FROM annotations WHERE id = $1', [id]);
    return result.rows[0] ? rowToAnnotation(result.rows[0]) : null;
}
export async function createAnnotation(ann) {
    const result = await query(`INSERT INTO annotations (symbol, source, type, points, style, strength, "group", tags, visibility, timeframe, ttl, metadata)
     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
     RETURNING *`, [
        ann.symbol,
        ann.source ?? 'user',
        ann.type,
        JSON.stringify(ann.points ?? []),
        JSON.stringify(ann.style ?? {}),
        ann.strength ?? 0.5,
        ann.group ?? null,
        ann.tags ?? [],
        ann.visibility ?? ['*'],
        ann.timeframe ?? null,
        ann.ttl ?? null,
        JSON.stringify(ann.metadata ?? {}),
    ]);
    await invalidate(ann.symbol);
    return rowToAnnotation(result.rows[0]);
}
export async function upsertAnnotation(ann) {
    const result = await query(`INSERT INTO annotations (id, symbol, source, type, points, style, strength, "group", tags, visibility, timeframe, ttl, metadata)
     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
     ON CONFLICT (id) DO UPDATE SET
       points = EXCLUDED.points,
       style = EXCLUDED.style,
       strength = EXCLUDED.strength,
       "group" = EXCLUDED."group",
       tags = EXCLUDED.tags,
       visibility = EXCLUDED.visibility,
       ttl = EXCLUDED.ttl,
       metadata = EXCLUDED.metadata,
       updated_at = NOW()
     RETURNING *`, [
        ann.id,
        ann.symbol,
        ann.source ?? 'user',
        ann.type,
        JSON.stringify(ann.points ?? []),
        JSON.stringify(ann.style ?? {}),
        ann.strength ?? 0.5,
        ann.group ?? null,
        ann.tags ?? [],
        ann.visibility ?? ['*'],
        ann.timeframe ?? null,
        ann.ttl ?? null,
        JSON.stringify(ann.metadata ?? {}),
    ]);
    await invalidate(ann.symbol);
    return rowToAnnotation(result.rows[0]);
}
export async function updateAnnotation(id, updates) {
    const existing = await getAnnotation(id);
    if (!existing)
        return null;
    const sets = ['updated_at = NOW()'];
    const params = [id];
    let idx = 2;
    if (updates.points !== undefined) {
        sets.push(`points = $${idx++}`);
        params.push(JSON.stringify(updates.points));
    }
    if (updates.style !== undefined) {
        sets.push(`style = $${idx++}`);
        params.push(JSON.stringify(updates.style));
    }
    if (updates.strength !== undefined) {
        sets.push(`strength = $${idx++}`);
        params.push(updates.strength);
    }
    if (updates.group !== undefined) {
        sets.push(`"group" = $${idx++}`);
        params.push(updates.group);
    }
    if (updates.tags !== undefined) {
        sets.push(`tags = $${idx++}`);
        params.push(updates.tags);
    }
    if (updates.visibility !== undefined) {
        sets.push(`visibility = $${idx++}`);
        params.push(updates.visibility);
    }
    if (updates.ttl !== undefined) {
        sets.push(`ttl = $${idx++}`);
        params.push(updates.ttl);
    }
    if (updates.metadata !== undefined) {
        sets.push(`metadata = $${idx++}`);
        params.push(JSON.stringify(updates.metadata));
    }
    const result = await query(`UPDATE annotations SET ${sets.join(', ')} WHERE id = $1 RETURNING *`, params);
    await invalidate(existing.symbol);
    return result.rows[0] ? rowToAnnotation(result.rows[0]) : null;
}
export async function updatePoints(id, points) {
    const result = await query('UPDATE annotations SET points = $1, updated_at = NOW() WHERE id = $2 RETURNING symbol', [JSON.stringify(points), id]);
    if (result.rows[0])
        await invalidate(result.rows[0].symbol);
}
export async function updateStyle(id, style) {
    const result = await query(`UPDATE annotations SET style = style || $1, updated_at = NOW() WHERE id = $2 RETURNING symbol`, [JSON.stringify(style), id]);
    if (result.rows[0])
        await invalidate(result.rows[0].symbol);
}
export async function deleteAnnotation(id) {
    const result = await query('DELETE FROM annotations WHERE id = $1 RETURNING symbol', [id]);
    if (result.rows[0])
        await invalidate(result.rows[0].symbol);
}
export async function deleteByFilter(filter) {
    const conditions = [];
    const params = [];
    let idx = 1;
    if (filter.symbol) {
        conditions.push(`symbol = $${idx++}`);
        params.push(filter.symbol);
    }
    if (filter.source) {
        conditions.push(`source = $${idx++}`);
        params.push(filter.source);
    }
    if (filter.group) {
        conditions.push(`"group" = $${idx++}`);
        params.push(filter.group);
    }
    if (conditions.length === 0)
        return 0; // safety: no unfiltered bulk delete
    const result = await query(`DELETE FROM annotations WHERE ${conditions.join(' AND ')}`, params);
    if (filter.symbol)
        await invalidate(filter.symbol);
    return result.rowCount ?? 0;
}
export async function reapExpired() {
    const result = await query('DELETE FROM annotations WHERE ttl IS NOT NULL AND ttl < NOW()');
    return result.rowCount ?? 0;
}
//# sourceMappingURL=annotations.js.map