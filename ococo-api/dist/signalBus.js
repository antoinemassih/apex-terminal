/**
 * Signal bus: bridges Redis pub/sub → WebSocket clients.
 *
 * Compute workers (GEX, pattern detection, etc.) publish signals to
 * Redis channels `signals:{symbol}`. This module subscribes to those
 * channels and forwards signals to connected WebSocket clients.
 *
 * Also handles publishing from within this service (e.g., alert triggers).
 */
import { redisSub, redisPub } from './redis.js';
const clients = new Set();
const subscribedChannels = new Set();
/** Register a new WebSocket client */
export function addClient(ws) {
    const client = { ws, symbols: new Set() };
    clients.add(client);
    return client;
}
/** Remove a disconnected client */
export function removeClient(client) {
    clients.delete(client);
    // Unsubscribe from channels no longer needed
    for (const symbol of client.symbols) {
        const channel = `signals:${symbol}`;
        const stillNeeded = Array.from(clients).some(c => c.symbols.has(symbol));
        if (!stillNeeded && subscribedChannels.has(channel)) {
            redisSub.unsubscribe(channel).catch(() => { });
            subscribedChannels.delete(channel);
        }
    }
}
/** Subscribe a client to symbols */
export async function subscribeClient(client, symbols) {
    for (const symbol of symbols) {
        client.symbols.add(symbol);
        const channel = `signals:${symbol}`;
        if (!subscribedChannels.has(channel)) {
            await redisSub.subscribe(channel);
            subscribedChannels.add(channel);
        }
    }
}
/** Unsubscribe a client from symbols */
export function unsubscribeClient(client, symbols) {
    for (const symbol of symbols) {
        client.symbols.delete(symbol);
        const channel = `signals:${symbol}`;
        const stillNeeded = Array.from(clients).some(c => c.symbols.has(symbol));
        if (!stillNeeded && subscribedChannels.has(channel)) {
            redisSub.unsubscribe(channel).catch(() => { });
            subscribedChannels.delete(channel);
        }
    }
}
/** Send a message to a specific client */
export function send(client, msg) {
    if (client.ws.readyState === 1) { // WebSocket.OPEN
        client.ws.send(JSON.stringify(msg));
    }
}
/** Broadcast to all clients subscribed to a symbol */
export function broadcast(symbol, msg) {
    const data = JSON.stringify(msg);
    for (const client of clients) {
        if (client.symbols.has(symbol) && client.ws.readyState === 1) {
            client.ws.send(data);
        }
    }
}
/** Publish a signal to Redis (for other services to consume or for broadcast) */
export async function publishSignal(symbol, annotation) {
    const msg = { type: 'signal', annotation };
    await redisPub.publish(`signals:${symbol}`, JSON.stringify(msg));
}
/** Publish a signal removal */
export async function publishSignalRemove(symbol, id) {
    const msg = { type: 'signal_remove', id, symbol };
    await redisPub.publish(`signals:${symbol}`, JSON.stringify(msg));
}
/** Initialize the Redis → WebSocket bridge */
export function initSignalBus() {
    redisSub.on('message', (channel, message) => {
        // channel = "signals:AAPL" → symbol = "AAPL"
        const symbol = channel.replace('signals:', '');
        try {
            const msg = JSON.parse(message);
            broadcast(symbol, msg);
        }
        catch (e) {
            console.warn('Invalid signal message:', e);
        }
    });
}
/** Get connected client count */
export function getClientCount() {
    return clients.size;
}
/** Get total subscriptions across all clients */
export function getSubscriptionCount() {
    return subscribedChannels.size;
}
//# sourceMappingURL=signalBus.js.map