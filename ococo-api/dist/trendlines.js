/**
 * Auto-trendline detection engine.
 *
 * Runs on server, detects trendlines across all timeframes,
 * using both wicks (high/low) and bodies (open/close).
 * Publishes results as annotations via Redis → WebSocket.
 */
import { query } from './db.js';
import { publishSignal } from './signalBus.js';
import { invalidate } from './cache.js';
import { v4 as uuid } from 'uuid';
// ---------------------------------------------------------------------------
// Pivot Detection
// ---------------------------------------------------------------------------
function detectPivots(bars, lookback, source) {
    const pivots = [];
    const len = bars.length;
    for (let i = lookback; i < len - lookback; i++) {
        const highVal = source === 'wick' ? bars[i].high : Math.max(bars[i].open, bars[i].close);
        const lowVal = source === 'wick' ? bars[i].low : Math.min(bars[i].open, bars[i].close);
        // Check swing high
        let isHigh = true;
        for (let j = i - lookback; j <= i + lookback; j++) {
            if (j === i)
                continue;
            const cmp = source === 'wick' ? bars[j].high : Math.max(bars[j].open, bars[j].close);
            if (cmp >= highVal) {
                isHigh = false;
                break;
            }
        }
        if (isHigh) {
            pivots.push({ index: i, time: bars[i].time, price: highVal, type: 'high', source });
        }
        // Check swing low
        let isLow = true;
        for (let j = i - lookback; j <= i + lookback; j++) {
            if (j === i)
                continue;
            const cmp = source === 'wick' ? bars[j].low : Math.min(bars[j].open, bars[j].close);
            if (cmp <= lowVal) {
                isLow = false;
                break;
            }
        }
        if (isLow) {
            pivots.push({ index: i, time: bars[i].time, price: lowVal, type: 'low', source });
        }
    }
    return pivots;
}
// ---------------------------------------------------------------------------
// Trendline Scoring
// ---------------------------------------------------------------------------
function priceAtBar(slope, intercept, index) {
    return slope * index + intercept;
}
function scoreTrendline(candidate, bars, allPivots, direction) {
    const { p1, p2, slope, intercept } = candidate;
    const tolerance = Math.abs(p2.price - p1.price) * 0.02 + (bars[0]?.close ?? 100) * 0.002;
    // Count touches (other pivots near the line)
    let touches = 0;
    for (const piv of allPivots) {
        if (piv.index === p1.index || piv.index === p2.index)
            continue;
        if (piv.index < p1.index || piv.index > p2.index)
            continue;
        const expected = priceAtBar(slope, intercept, piv.index);
        if (Math.abs(piv.price - expected) < tolerance)
            touches++;
    }
    // Time span (number of bars)
    const span = p2.index - p1.index;
    // Recency bonus (lines touching recent bars score higher)
    const recency = p2.index / bars.length;
    // Angle penalty (near-horizontal is better)
    const angleRad = Math.abs(Math.atan(slope / (bars[0]?.close ?? 100)));
    const anglePenalty = 1 - Math.min(1, angleRad / (Math.PI / 4));
    // Break count (how many bars violate the line)
    let breaks = 0;
    for (let i = p1.index; i <= p2.index; i++) {
        const expected = priceAtBar(slope, intercept, i);
        if (direction === 'support') {
            if (bars[i].low < expected - tolerance)
                breaks++;
        }
        else {
            if (bars[i].high > expected + tolerance)
                breaks++;
        }
    }
    const breakPenalty = Math.max(0, 1 - breaks * 0.15);
    const score = (2 + touches * 3) * (1 + span / bars.length) * recency * anglePenalty * breakPenalty;
    return {
        p1, p2, slope, intercept, touches, span, score,
        direction,
        source: p1.source,
    };
}
// ---------------------------------------------------------------------------
// Trendline Detection
// ---------------------------------------------------------------------------
function detectTrendlines(bars, lookback, source, maxCandidates = 200) {
    const pivots = detectPivots(bars, lookback, source);
    const highs = pivots.filter(p => p.type === 'high');
    const lows = pivots.filter(p => p.type === 'low');
    const candidates = [];
    // Support lines: connect pairs of lows
    for (let i = 0; i < lows.length && candidates.length < maxCandidates; i++) {
        for (let j = i + 1; j < lows.length && candidates.length < maxCandidates; j++) {
            const p1 = lows[i], p2 = lows[j];
            if (p2.index - p1.index < lookback)
                continue; // too close
            const slope = (p2.price - p1.price) / (p2.index - p1.index);
            const intercept = p1.price - slope * p1.index;
            candidates.push(scoreTrendline({ p1, p2, slope, intercept }, bars, pivots, 'support'));
        }
    }
    // Resistance lines: connect pairs of highs
    for (let i = 0; i < highs.length && candidates.length < maxCandidates * 2; i++) {
        for (let j = i + 1; j < highs.length && candidates.length < maxCandidates * 2; j++) {
            const p1 = highs[i], p2 = highs[j];
            if (p2.index - p1.index < lookback)
                continue;
            const slope = (p2.price - p1.price) / (p2.index - p1.index);
            const intercept = p1.price - slope * p1.index;
            candidates.push(scoreTrendline({ p1, p2, slope, intercept }, bars, pivots, 'resistance'));
        }
    }
    // Sort by score, keep top N
    candidates.sort((a, b) => b.score - a.score);
    return candidates.slice(0, 10);
}
// ---------------------------------------------------------------------------
// Channel Detection
// ---------------------------------------------------------------------------
function detectChannels(trendlines) {
    const supports = trendlines.filter(t => t.direction === 'support');
    const resistances = trendlines.filter(t => t.direction === 'resistance');
    const channels = [];
    for (const sup of supports) {
        for (const res of resistances) {
            // Check if roughly parallel (slope difference < 15%)
            const avgSlope = (Math.abs(sup.slope) + Math.abs(res.slope)) / 2;
            if (avgSlope === 0)
                continue;
            const slopeDiff = Math.abs(sup.slope - res.slope) / avgSlope;
            if (slopeDiff > 0.3)
                continue;
            // Check that resistance is above support
            const midIdx = Math.round((sup.p1.index + sup.p2.index) / 2);
            const supPrice = priceAtBar(sup.slope, sup.intercept, midIdx);
            const resPrice = priceAtBar(res.slope, res.intercept, midIdx);
            if (resPrice <= supPrice)
                continue;
            const parallelScore = (sup.score + res.score) * (1 - slopeDiff);
            channels.push({ support: sup, resistance: res, parallelScore });
        }
    }
    channels.sort((a, b) => b.parallelScore - a.parallelScore);
    return channels.slice(0, 3);
}
// ---------------------------------------------------------------------------
// Public API: Run Detection for a Symbol
// ---------------------------------------------------------------------------
const TIMEFRAME_CONFIGS = [
    { tf: '1h', label: '1H', lookbacks: [5, 10, 20] },
    { tf: '4h', label: '4H', lookbacks: [5, 10, 15] },
    { tf: '1d', label: '1D', lookbacks: [5, 10, 20] },
    { tf: '1wk', label: '1W', lookbacks: [3, 5, 10] },
];
const SOURCES = ['wick', 'body'];
const COLORS = {
    '1H-wick-support': '#2196f3',
    '1H-wick-resistance': '#f44336',
    '1H-body-support': '#64b5f6',
    '1H-body-resistance': '#ef9a9a',
    '4H-wick-support': '#4caf50',
    '4H-wick-resistance': '#ff9800',
    '4H-body-support': '#81c784',
    '4H-body-resistance': '#ffb74d',
    '1D-wick-support': '#9c27b0',
    '1D-wick-resistance': '#e91e63',
    '1D-body-support': '#ba68c8',
    '1D-body-resistance': '#f06292',
    '1W-wick-support': '#00bcd4',
    '1W-wick-resistance': '#ff5722',
    '1W-body-support': '#4dd0e1',
    '1W-body-resistance': '#ff8a65',
};
export async function runTrendlineDetection(symbol, barsMap) {
    // Clear old auto-trendlines for this symbol
    await query(`DELETE FROM annotations WHERE symbol = $1 AND source = 'auto-trend'`, [symbol]);
    await invalidate(symbol);
    const annotations = [];
    for (const config of TIMEFRAME_CONFIGS) {
        const bars = barsMap[config.tf];
        if (!bars || bars.length < 30)
            continue;
        for (const source of SOURCES) {
            for (const lookback of config.lookbacks) {
                const trendlines = detectTrendlines(bars, lookback, source);
                for (const tl of trendlines) {
                    if (tl.score < 3)
                        continue; // skip weak lines
                    const label = `${config.label} ${source}`;
                    const colorKey = `${config.label}-${source}-${tl.direction}`;
                    const color = COLORS[colorKey] ?? '#888';
                    const ann = {
                        id: uuid(),
                        symbol,
                        source: 'auto-trend',
                        type: 'trendline',
                        points: [
                            { time: tl.p1.time, price: tl.p1.price },
                            { time: tl.p2.time, price: tl.p2.price },
                        ],
                        style: {
                            color,
                            opacity: Math.min(1, 0.4 + tl.score * 0.05),
                            lineStyle: source === 'body' ? 'dashed' : 'solid',
                            thickness: Math.min(2, 0.5 + tl.touches * 0.3),
                        },
                        strength: Math.min(1, tl.score / 20),
                        group: 'auto-trendlines',
                        tags: [config.label, source, tl.direction, `lb${lookback}`],
                        visibility: ['*'],
                        timeframe: config.tf,
                        ttl: null,
                        metadata: {
                            label,
                            touches: tl.touches,
                            span: tl.span,
                            slope: tl.slope,
                            lookback,
                            direction: tl.direction,
                            source,
                            timeframeLabel: config.label,
                        },
                        created_at: new Date().toISOString(),
                        updated_at: new Date().toISOString(),
                    };
                    annotations.push(ann);
                }
                // Channel detection
                const allTrendlines = detectTrendlines(bars, lookback, source, 50);
                const channels = detectChannels(allTrendlines);
                for (const ch of channels) {
                    const colorKey = `${config.label}-${source}-support`;
                    const color = COLORS[colorKey] ?? '#888';
                    // Store channel as two linked trendlines with 'channel' tag
                    for (const tl of [ch.support, ch.resistance]) {
                        annotations.push({
                            id: uuid(),
                            symbol,
                            source: 'auto-trend',
                            type: 'trendline',
                            points: [
                                { time: tl.p1.time, price: tl.p1.price },
                                { time: tl.p2.time, price: tl.p2.price },
                            ],
                            style: {
                                color,
                                opacity: 0.6,
                                lineStyle: 'dotted',
                                thickness: 1,
                            },
                            strength: Math.min(1, ch.parallelScore / 30),
                            group: 'auto-channels',
                            tags: [config.label, source, 'channel', tl.direction],
                            visibility: ['*'],
                            timeframe: config.tf,
                            ttl: null,
                            metadata: {
                                label: `${config.label} ${source} channel`,
                                direction: tl.direction,
                                parallelScore: ch.parallelScore,
                            },
                            created_at: new Date().toISOString(),
                            updated_at: new Date().toISOString(),
                        });
                    }
                }
            }
        }
    }
    // Deduplicate: remove lines that are very similar (within 1% price tolerance)
    const deduped = deduplicateTrendlines(annotations);
    // Persist to DB
    for (const ann of deduped) {
        await query(`INSERT INTO annotations (id, symbol, source, type, points, style, strength, "group", tags, visibility, timeframe, metadata)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)`, [
            ann.id, ann.symbol, ann.source, ann.type,
            JSON.stringify(ann.points), JSON.stringify(ann.style),
            ann.strength, ann.group, ann.tags, ann.visibility,
            ann.timeframe, JSON.stringify(ann.metadata),
        ]);
        // Publish to real-time subscribers
        await publishSignal(symbol, ann);
    }
    await invalidate(symbol);
    console.info(`Trendline detection for ${symbol}: ${deduped.length} trendlines/channels`);
}
function deduplicateTrendlines(annotations) {
    const result = [];
    for (const ann of annotations) {
        const isDupe = result.some(existing => {
            if (existing.type !== ann.type)
                return false;
            if (existing.points.length !== ann.points.length)
                return false;
            const p1Diff = Math.abs(existing.points[0].price - ann.points[0].price) / ann.points[0].price;
            const p2Diff = Math.abs(existing.points[1].price - ann.points[1].price) / ann.points[1].price;
            return p1Diff < 0.01 && p2Diff < 0.01;
        });
        if (!isDupe)
            result.push(ann);
    }
    return result;
}
//# sourceMappingURL=trendlines.js.map