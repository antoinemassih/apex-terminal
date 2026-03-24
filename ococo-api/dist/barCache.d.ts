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
interface Bar {
    time: number;
    open: number;
    high: number;
    low: number;
    close: number;
    volume: number;
}
/** Write bars to Redis sorted set (score = timestamp) */
export declare function writeBarsToRedis(symbol: string, interval: string, bars: Bar[]): Promise<void>;
/** Read bars from Redis, newest first, with optional pagination */
export declare function readBarsFromRedis(symbol: string, interval: string, opts?: {
    before?: number;
    limit?: number;
    after?: number;
}): Promise<Bar[]>;
/** Get total bar count for a symbol:interval */
export declare function barCountInRedis(symbol: string, interval: string): Promise<number>;
/** Get the time range of bars in Redis */
export declare function barTimeRange(symbol: string, interval: string): Promise<{
    oldest: number;
    newest: number;
} | null>;
/** Check if a specific time range exists in Redis */
export declare function hasBarsInRedis(symbol: string, interval: string, from: number, to: number): Promise<boolean>;
/** Delete all bars for a symbol:interval */
export declare function clearBarsInRedis(symbol: string, interval: string): Promise<void>;
export {};
//# sourceMappingURL=barCache.d.ts.map