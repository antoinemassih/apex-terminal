/**
 * Data ingestion worker.
 *
 * Fetches OHLCV bars from a data source (currently yfinance HTTP sidecar)
 * and writes them to InfluxDB. Runs on a schedule.
 *
 * Also triggers trendline detection after ingestion.
 */
import { writeBars, readBars } from './influx.js';
import { runTrendlineDetection } from './trendlines.js';
import { query } from './db.js';
const YFINANCE_URL = process.env.YFINANCE_URL ?? 'http://127.0.0.1:8777';
const INTERVALS = [
    { interval: '1h', period: '1mo', label: '1h' },
    { interval: '1d', period: '2y', label: '1d' },
    { interval: '1wk', period: '10y', label: '1wk' },
];
/** Fetch bars from yfinance sidecar */
async function fetchBars(symbol, interval, period) {
    try {
        const resp = await fetch(`${YFINANCE_URL}/bars?symbol=${symbol}&interval=${interval}&period=${period}`);
        if (!resp.ok)
            return [];
        return resp.json();
    }
    catch {
        return [];
    }
}
/** Aggregate 1h bars into 4h bars */
function aggregate4h(bars1h) {
    const result = [];
    for (let i = 0; i < bars1h.length; i += 4) {
        const chunk = bars1h.slice(i, i + 4);
        if (chunk.length === 0)
            continue;
        result.push({
            time: chunk[0].time,
            open: chunk[0].open,
            high: Math.max(...chunk.map(b => b.high)),
            low: Math.min(...chunk.map(b => b.low)),
            close: chunk[chunk.length - 1].close,
            volume: chunk.reduce((s, b) => s + b.volume, 0),
        });
    }
    return result;
}
/** Ingest all intervals for a symbol */
async function ingestSymbol(symbol) {
    const counts = {};
    for (const { interval, period, label } of INTERVALS) {
        const bars = await fetchBars(symbol, interval, period);
        if (bars.length > 0) {
            await writeBars(symbol, label, bars);
            counts[label] = bars.length;
            // Also create 4h from 1h
            if (label === '1h') {
                const bars4h = aggregate4h(bars);
                await writeBars(symbol, '4h', bars4h);
                counts['4h'] = bars4h.length;
            }
        }
    }
    return counts;
}
/** Get all symbols from the DB catalog */
async function getSymbols() {
    const result = await query("SELECT symbol FROM symbols WHERE type != 'unknown' ORDER BY symbol");
    return result.rows.map((r) => r.symbol);
}
/** Full ingestion + detection cycle for all symbols */
export async function runIngestionCycle() {
    const symbols = await getSymbols();
    console.info(`Ingestion cycle starting for ${symbols.length} symbols`);
    for (const symbol of symbols) {
        try {
            const counts = await ingestSymbol(symbol);
            const totalBars = Object.values(counts).reduce((s, n) => s + n, 0);
            if (totalBars === 0) {
                console.warn(`  ${symbol}: no data from yfinance`);
                continue;
            }
            console.info(`  ${symbol}: ingested ${Object.entries(counts).map(([k, v]) => `${k}:${v}`).join(' ')}`);
            // Read bars back from InfluxDB for trendline detection
            const barsMap = {};
            for (const tf of ['1h', '4h', '1d', '1wk']) {
                const bars = await readBars(symbol, tf, { start: tf === '1wk' ? '-10y' : tf === '1d' ? '-2y' : '-3mo' });
                if (bars.length > 20)
                    barsMap[tf] = bars;
            }
            await runTrendlineDetection(symbol, barsMap);
        }
        catch (e) {
            console.error(`  ${symbol}: ingestion failed:`, e);
        }
    }
    console.info('Ingestion cycle complete');
}
/** Ingest a single symbol (for on-demand use) */
export async function ingestSingle(symbol) {
    const counts = await ingestSymbol(symbol);
    // Run detection after ingestion
    const barsMap = {};
    for (const tf of ['1h', '4h', '1d', '1wk']) {
        const bars = await readBars(symbol, tf, { start: tf === '1wk' ? '-10y' : tf === '1d' ? '-2y' : '-3mo' });
        if (bars.length > 20)
            barsMap[tf] = bars;
    }
    await runTrendlineDetection(symbol, barsMap);
    return counts;
}
//# sourceMappingURL=ingest.js.map