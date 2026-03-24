import { redis } from './redis.js';
import { config } from './config.js';
const PREFIX = 'ococo:ann:';
function key(symbol) {
    return `${PREFIX}${symbol}`;
}
/** Get cached annotations for a symbol. Returns null on miss. */
export async function getCached(symbol) {
    try {
        const raw = await redis.get(key(symbol));
        if (!raw)
            return null;
        return JSON.parse(raw);
    }
    catch {
        return null;
    }
}
/** Cache annotations for a symbol */
export async function setCache(symbol, annotations) {
    try {
        await redis.setex(key(symbol), config.cacheTtl, JSON.stringify(annotations));
    }
    catch (e) {
        console.warn('Cache set failed:', e);
    }
}
/** Invalidate cache for a symbol */
export async function invalidate(symbol) {
    try {
        await redis.del(key(symbol));
    }
    catch (e) {
        console.warn('Cache invalidate failed:', e);
    }
}
/**
 * Fetch with stampede protection.
 * Concurrent cache misses for the same symbol coalesce into a single DB fetch.
 */
const inflight = new Map();
export async function cachedFetch(symbol, fetcher) {
    const cached = await getCached(symbol);
    if (cached !== null)
        return cached;
    if (!inflight.has(symbol)) {
        const p = fetcher()
            .then(async (result) => { await setCache(symbol, result); return result; })
            .finally(() => inflight.delete(symbol));
        inflight.set(symbol, p);
    }
    return inflight.get(symbol);
}
/** Invalidate all annotation caches */
export async function invalidateAll() {
    try {
        const keys = await redis.keys(`${PREFIX}*`);
        if (keys.length > 0)
            await redis.del(...keys);
    }
    catch (e) {
        console.warn('Cache invalidateAll failed:', e);
    }
}
//# sourceMappingURL=cache.js.map