/**
 * Signal bus: bridges Redis pub/sub → WebSocket clients.
 *
 * Compute workers (GEX, pattern detection, etc.) publish signals to
 * Redis channels `signals:{symbol}`. This module subscribes to those
 * channels and forwards signals to connected WebSocket clients.
 *
 * Also handles publishing from within this service (e.g., alert triggers).
 */
import type { Annotation, WsServerMessage } from './types.js';
import type { WebSocket } from 'ws';
interface Client {
    ws: WebSocket;
    symbols: Set<string>;
}
/** Register a new WebSocket client */
export declare function addClient(ws: WebSocket): Client;
/** Remove a disconnected client */
export declare function removeClient(client: Client): void;
/** Subscribe a client to symbols */
export declare function subscribeClient(client: Client, symbols: string[]): Promise<void>;
/** Unsubscribe a client from symbols */
export declare function unsubscribeClient(client: Client, symbols: string[]): void;
/** Send a message to a specific client */
export declare function send(client: Client, msg: WsServerMessage): void;
/** Broadcast to all clients subscribed to a symbol */
export declare function broadcast(symbol: string, msg: WsServerMessage): void;
/** Publish a signal to Redis (for other services to consume or for broadcast) */
export declare function publishSignal(symbol: string, annotation: Annotation): Promise<void>;
/** Publish a signal removal */
export declare function publishSignalRemove(symbol: string, id: string): Promise<void>;
/** Initialize the Redis → WebSocket bridge */
export declare function initSignalBus(): void;
/** Get connected client count */
export declare function getClientCount(): number;
/** Get total subscriptions across all clients */
export declare function getSubscriptionCount(): number;
export {};
//# sourceMappingURL=signalBus.d.ts.map