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