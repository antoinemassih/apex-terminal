/**
 * Auto-trendline detection engine.
 *
 * Runs on server, detects trendlines across all timeframes,
 * using both wicks (high/low) and bodies (open/close).
 * Publishes results as annotations via Redis → WebSocket.
 */
interface Bar {
    time: number;
    open: number;
    high: number;
    low: number;
    close: number;
    volume: number;
}
export declare function runTrendlineDetection(symbol: string, barsMap: Record<string, Bar[]>): Promise<void>;
export {};
//# sourceMappingURL=trendlines.d.ts.map