/**
 * Data ingestion worker.
 *
 * Fetches OHLCV bars from a data source (currently yfinance HTTP sidecar)
 * and writes them to InfluxDB. Runs on a schedule.
 *
 * Also triggers trendline detection after ingestion.
 */
/** Full ingestion + detection cycle for all symbols */
export declare function runIngestionCycle(): Promise<void>;
/** Ingest a single symbol (for on-demand use) */
export declare function ingestSingle(symbol: string): Promise<Record<string, number>>;
//# sourceMappingURL=ingest.d.ts.map