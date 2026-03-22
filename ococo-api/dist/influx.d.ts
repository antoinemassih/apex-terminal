/**
 * InfluxDB client for OHLCV bar data.
 *
 * Bars are stored as points in the "stocks" bucket:
 *   measurement: "bars"
 *   tags: symbol, interval (1h, 4h, 1d, 1wk)
 *   fields: open, high, low, close, volume
 *   timestamp: bar time (unix nanoseconds)
 */
interface Bar {
    time: number;
    open: number;
    high: number;
    low: number;
    close: number;
    volume: number;
}
/** Write bars to InfluxDB in line protocol (chunked to avoid timeouts) */
export declare function writeBars(symbol: string, interval: string, bars: Bar[]): Promise<void>;
/** Read bars from InfluxDB */
export declare function readBars(symbol: string, interval: string, opts?: {
    start?: string;
    stop?: string;
    limit?: number;
}): Promise<Bar[]>;
/** Check how many bars exist for a symbol+interval */
export declare function barCount(symbol: string, interval: string): Promise<number>;
export {};
//# sourceMappingURL=influx.d.ts.map