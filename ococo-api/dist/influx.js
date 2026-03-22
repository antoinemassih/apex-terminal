/**
 * InfluxDB client for OHLCV bar data.
 *
 * Bars are stored as points in the "stocks" bucket:
 *   measurement: "bars"
 *   tags: symbol, interval (1h, 4h, 1d, 1wk)
 *   fields: open, high, low, close, volume
 *   timestamp: bar time (unix nanoseconds)
 */
import { config } from './config.js';
const INFLUX_URL = config.influx.url;
const INFLUX_TOKEN = config.influx.token;
const INFLUX_ORG = config.influx.org;
const BUCKET = 'stocks';
/** Write bars to InfluxDB in line protocol (chunked to avoid timeouts) */
export async function writeBars(symbol, interval, bars) {
    if (bars.length === 0)
        return;
    const CHUNK_SIZE = 100;
    for (let i = 0; i < bars.length; i += CHUNK_SIZE) {
        const chunk = bars.slice(i, i + CHUNK_SIZE);
        const lines = chunk.map(b => {
            const ts = BigInt(Math.floor(b.time)) * 1000000000n;
            return `bars,symbol=${symbol},interval=${interval} open=${b.open},high=${b.high},low=${b.low},close=${b.close},volume=${b.volume} ${ts}`;
        }).join('\n');
        const resp = await fetch(`${INFLUX_URL}/api/v2/write?org=${INFLUX_ORG}&bucket=${BUCKET}&precision=ns`, {
            method: 'POST',
            headers: {
                'Authorization': `Token ${INFLUX_TOKEN}`,
                'Content-Type': 'text/plain',
            },
            body: lines,
        });
        if (!resp.ok) {
            const text = await resp.text();
            console.warn(`InfluxDB write chunk failed (${chunk.length} points): ${resp.status} ${text.slice(0, 200)}`);
            // Don't throw — continue with remaining chunks
        }
    }
}
/** Read bars from InfluxDB */
export async function readBars(symbol, interval, opts) {
    const start = opts?.start ?? '-1y';
    const stop = opts?.stop ?? 'now()';
    let flux = `from(bucket: "${BUCKET}")
  |> range(start: ${start}, stop: ${stop})
  |> filter(fn: (r) => r._measurement == "bars")
  |> filter(fn: (r) => r.symbol == "${symbol}")
  |> filter(fn: (r) => r.interval == "${interval}")
  |> pivot(rowKey: ["_time"], columnKey: ["_field"], valueColumn: "_value")
  |> sort(columns: ["_time"])`;
    if (opts?.limit) {
        flux += `\n  |> limit(n: ${opts.limit})`;
    }
    const resp = await fetch(`${INFLUX_URL}/api/v2/query?org=${INFLUX_ORG}`, {
        method: 'POST',
        headers: {
            'Authorization': `Token ${INFLUX_TOKEN}`,
            'Content-Type': 'application/vnd.flux',
            'Accept': 'application/csv',
        },
        body: flux,
    });
    if (!resp.ok) {
        const text = await resp.text();
        throw new Error(`InfluxDB query failed: ${resp.status} ${text}`);
    }
    const csv = await resp.text();
    return parseFluxCSV(csv);
}
/** Check how many bars exist for a symbol+interval */
export async function barCount(symbol, interval) {
    const flux = `from(bucket: "${BUCKET}")
  |> range(start: -10y)
  |> filter(fn: (r) => r._measurement == "bars" and r.symbol == "${symbol}" and r.interval == "${interval}" and r._field == "close")
  |> count()`;
    const resp = await fetch(`${INFLUX_URL}/api/v2/query?org=${INFLUX_ORG}`, {
        method: 'POST',
        headers: {
            'Authorization': `Token ${INFLUX_TOKEN}`,
            'Content-Type': 'application/vnd.flux',
            'Accept': 'application/csv',
        },
        body: flux,
    });
    if (!resp.ok)
        return 0;
    const csv = await resp.text();
    const lines = csv.trim().split('\n');
    if (lines.length < 2)
        return 0;
    // The count value is in the last column of the data row
    const parts = lines[lines.length - 1].split(',');
    const val = parseInt(parts[parts.length - 1]);
    return isNaN(val) ? 0 : val;
}
function parseFluxCSV(csv) {
    const lines = csv.trim().split('\n');
    if (lines.length < 2)
        return [];
    // Find header line (starts with empty string after the annotation rows)
    let headerIdx = -1;
    for (let i = 0; i < lines.length; i++) {
        if (lines[i].startsWith(',result,')) {
            headerIdx = i;
            break;
        }
    }
    if (headerIdx < 0)
        return [];
    const headers = lines[headerIdx].split(',');
    const timeCol = headers.indexOf('_time');
    const openCol = headers.indexOf('open');
    const highCol = headers.indexOf('high');
    const lowCol = headers.indexOf('low');
    const closeCol = headers.indexOf('close');
    const volCol = headers.indexOf('volume');
    if (timeCol < 0 || closeCol < 0)
        return [];
    const bars = [];
    for (let i = headerIdx + 1; i < lines.length; i++) {
        if (!lines[i] || lines[i].startsWith(','))
            continue;
        const cols = lines[i].split(',');
        if (cols.length <= closeCol)
            continue;
        const time = Math.floor(new Date(cols[timeCol]).getTime() / 1000);
        if (isNaN(time))
            continue;
        bars.push({
            time,
            open: parseFloat(cols[openCol]) || 0,
            high: parseFloat(cols[highCol]) || 0,
            low: parseFloat(cols[lowCol]) || 0,
            close: parseFloat(cols[closeCol]) || 0,
            volume: parseFloat(cols[volCol]) || 0,
        });
    }
    return bars;
}
//# sourceMappingURL=influx.js.map