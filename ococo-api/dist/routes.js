import * as ann from './annotations.js';
import * as alerts from './alerts.js';
import * as sym from './symbols.js';
import { runTrendlineDetection } from './trendlines.js';
import { healthCheck } from './db.js';
import { getClientCount, getSubscriptionCount } from './signalBus.js';
function aggregate4h(bars1h) {
    const result = [];
    for (let i = 0; i < bars1h.length; i += 4) {
        const chunk = bars1h.slice(i, i + 4);
        if (chunk.length === 0)
            continue;
        result.push({
            time: chunk[0].time,
            open: chunk[0].open,
            high: Math.max(...chunk.map((b) => b.high)),
            low: Math.min(...chunk.map((b) => b.low)),
            close: chunk[chunk.length - 1].close,
            volume: chunk.reduce((s, b) => s + b.volume, 0),
        });
    }
    return result;
}
export async function registerRoutes(app) {
    // Health
    app.get('/api/health', async () => {
        const dbOk = await healthCheck();
        return { status: dbOk ? 'ok' : 'degraded', db: dbOk, clients: getClientCount(), subscriptions: getSubscriptionCount() };
    });
    // ---- Annotations ----
    app.get('/api/annotations', async (req) => {
        const tags = req.query.tags ? req.query.tags.split(',') : undefined;
        return ann.listAnnotations({
            symbol: req.query.symbol,
            source: req.query.source,
            group: req.query.group,
            tags,
            type: req.query.type,
        });
    });
    app.get('/api/annotations/:id', async (req, reply) => {
        const result = await ann.getAnnotation(req.params.id);
        if (!result)
            return reply.code(404).send({ error: 'not found' });
        return result;
    });
    app.post('/api/annotations', async (req, reply) => {
        const body = req.body;
        if (!body.symbol || !body.type)
            return reply.code(400).send({ error: 'symbol and type required' });
        if (body.id)
            return ann.upsertAnnotation(body);
        return ann.createAnnotation(body);
    });
    app.patch('/api/annotations/:id', async (req, reply) => {
        const result = await ann.updateAnnotation(req.params.id, req.body);
        if (!result)
            return reply.code(404).send({ error: 'not found' });
        return result;
    });
    app.patch('/api/annotations/:id/points', async (req) => {
        await ann.updatePoints(req.params.id, req.body.points);
        return { ok: true };
    });
    app.patch('/api/annotations/:id/style', async (req) => {
        await ann.updateStyle(req.params.id, req.body);
        return { ok: true };
    });
    app.delete('/api/annotations/:id', async (req) => {
        await ann.deleteAnnotation(req.params.id);
        return { ok: true };
    });
    app.delete('/api/annotations', async (req, reply) => {
        if (!req.query.symbol && !req.query.source && !req.query.group) {
            return reply.code(400).send({ error: 'at least one filter required for bulk delete' });
        }
        const count = await ann.deleteByFilter({
            symbol: req.query.symbol,
            source: req.query.source,
            group: req.query.group,
        });
        return { deleted: count };
    });
    // ---- Alerts ----
    app.get('/api/alerts', async (req) => {
        return alerts.listAlerts(req.query.symbol);
    });
    app.post('/api/alerts', async (req, reply) => {
        const body = req.body;
        if (!body.symbol || !body.condition)
            return reply.code(400).send({ error: 'symbol and condition required' });
        return alerts.createAlert(body);
    });
    app.patch('/api/alerts/:id', async (req, reply) => {
        const result = await alerts.updateAlert(req.params.id, req.body);
        if (!result)
            return reply.code(404).send({ error: 'not found' });
        return result;
    });
    app.delete('/api/alerts/:id', async (req) => {
        await alerts.deleteAlert(req.params.id);
        return { ok: true };
    });
    // ---- Symbols ----
    app.get('/api/symbols', async (req) => {
        if (req.query.q)
            return sym.searchSymbols(req.query.q);
        return sym.listSymbols(req.query.type);
    });
    app.post('/api/symbols', async (req) => {
        await sym.upsertSymbol(req.body);
        return { ok: true };
    });
    // ---- Trendline Detection ----
    app.post('/api/trendlines/detect', async (req, reply) => {
        const symbol = req.query.symbol;
        if (!symbol)
            return reply.code(400).send({ error: 'symbol required' });
        // Fetch bars from yfinance sidecar for multiple timeframes
        const timeframes = [
            { tf: '1h', interval: '1h', period: '1mo' },
            { tf: '4h', interval: '1h', period: '3mo' }, // 4h = aggregate from 1h
            { tf: '1d', interval: '1d', period: '1y' },
            { tf: '1wk', interval: '1wk', period: '5y' },
        ];
        const barsMap = {};
        for (const config of timeframes) {
            try {
                const url = `http://127.0.0.1:8777/bars?symbol=${symbol}&interval=${config.interval}&period=${config.period}`;
                const resp = await fetch(url);
                if (resp.ok) {
                    let bars = await resp.json();
                    // For 4h: aggregate 1h bars into 4h
                    if (config.tf === '4h' && bars.length > 0) {
                        bars = aggregate4h(bars);
                    }
                    barsMap[config.tf] = bars;
                }
            }
            catch (e) {
                console.warn(`Failed to fetch ${config.tf} bars for ${symbol}:`, e);
            }
        }
        await runTrendlineDetection(symbol, barsMap);
        return { ok: true, timeframes: Object.keys(barsMap).map(k => `${k}: ${barsMap[k].length} bars`) };
    });
    // ---- Recents ----
    app.get('/api/recents', async (req) => {
        return sym.getRecents(req.query.session ?? 'default');
    });
    app.post('/api/recents', async (req) => {
        const body = req.body;
        if (!body.symbol)
            return { error: 'symbol required' };
        await sym.touchRecent(body.symbol, req.query.session ?? 'default');
        return { ok: true };
    });
}
//# sourceMappingURL=routes.js.map