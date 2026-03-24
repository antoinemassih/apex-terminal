/**
 * Redis-backed bar working set.
 *
 * Stores recent OHLCV bars per symbol:interval in Redis sorted sets.
 * Score = unix timestamp, value = JSON-encoded bar.
 * Much faster than InfluxDB for pagination reads (~0.1ms vs ~50ms).
 *
 * Capacity: ~200K bars per symbol:interval (configurable).
 * TTL: none (evicted by count, not time).
 */
import { redis } from './redis.js';
const MAX_BARS_PER_KEY = 200_000;
const PREFIX = 'bars:';
function key(symbol, interval) {
    return `${PREFIX}${symbol}:${interval}`;
}
const PIPELINE_BATCH = 1_000; // max commands per pipeline to avoid blocking other Redis ops
/** Write bars to Redis sorted set (score = timestamp) */
export async function writeBarsToRedis(symbol, interval, bars) {
    if (bars.length === 0)
        return;
    const k = key(symbol, interval);
    // Process in batches to avoid monopolising the Redis connection
    for (let i = 0; i < bars.length; i += PIPELINE_BATCH) {
        const chunk = bars.slice(i, i + PIPELINE_BATCH);
        const pipeline = redis.pipeline();
        for (const bar of chunk) {
            pipeline.zadd(k, bar.time, JSON.stringify(bar));
        }
        await pipeline.exec();
    }
    // Trim to max capacity (remove oldest)
    const count = await redis.zcard(k);
    if (count > MAX_BARS_PER_KEY) {
        await redis.zremrangebyrank(k, 0, count - MAX_BARS_PER_KEY - 1);
    }
}
/** Read bars from Redis, newest first, with optional pagination */
export async function readBarsFromRedis(symbol, interval, opts) {
    const k = key(symbol, interval);
    const limit = opts?.limit ?? 1000;
    let results;
    if (opts?.before) {
        // Get bars BEFORE a timestamp (for backward pagination)
        results = await redis.zrangebyscore(k, opts.after ?? '-inf', `(${opts.before}`, // exclusive upper bound
        'LIMIT', 0, limit);
    }
    else if (opts?.after) {
        // Get bars AFTER a timestamp
        results = await redis.zrangebyscore(k, `(${opts.after}`, '+inf', 'LIMIT', 0, limit);
    }
    else {
        // Get the most recent bars
        results = await redis.zrevrange(k, 0, limit - 1);
        results.reverse(); // chronological order
    }
    return results.map(s => {
        try {
            return JSON.parse(s);
        }
        catch {
            return null;
        }
    }).filter(Boolean);
}
/** Get total bar count for a symbol:interval */
export async function barCountInRedis(symbol, interval) {
    return redis.zcard(key(symbol, interval));
}
/** Get the time range of bars in Redis */
export async function barTimeRange(symbol, interval) {
    const k = key(symbol, interval);
    const oldest = await redis.zrange(k, 0, 0, 'WITHSCORES');
    const newest = await redis.zrevrange(k, 0, 0, 'WITHSCORES');
    if (oldest.length < 2 || newest.length < 2)
        return null;
    return {
        oldest: parseFloat(oldest[1]),
        newest: parseFloat(newest[1]),
    };
}
/** Check if a specific time range exists in Redis */
export async function hasBarsInRedis(symbol, interval, from, to) {
    const count = await redis.zcount(key(symbol, interval), from, to);
    return count > 0;
}
/** Delete all bars for a symbol:interval */
export async function clearBarsInRedis(symbol, interval) {
    await redis.del(key(symbol, interval));
}
//# sourceMappingURL=barCache.js.map