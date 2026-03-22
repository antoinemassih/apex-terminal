import type { FastifyInstance } from 'fastify'
import { listAnnotations } from './annotations.js'
import { checkAlerts } from './alerts.js'
import { addClient, removeClient, subscribeClient, unsubscribeClient, send, broadcast } from './signalBus.js'
import type { WsClientMessage } from './types.js'

export async function registerWebSocket(app: FastifyInstance): Promise<void> {
  app.get('/ws', { websocket: true }, (socket, _req) => {
    const client = addClient(socket)

    socket.on('message', async (raw: Buffer) => {
      try {
        const msg = JSON.parse(raw.toString()) as WsClientMessage

        switch (msg.type) {
          case 'subscribe': {
            await subscribeClient(client, msg.symbols)
            // Send snapshot for each newly subscribed symbol
            for (const symbol of msg.symbols) {
              const annotations = await listAnnotations({ symbol })
              send(client, { type: 'snapshot', symbol, annotations })
            }
            break
          }

          case 'unsubscribe': {
            unsubscribeClient(client, msg.symbols)
            break
          }

          case 'price': {
            // Price update from client — check alerts
            const triggered = await checkAlerts(msg.symbol, msg.price)
            for (const alert of triggered) {
              const alertMsg = {
                type: 'alert' as const,
                rule_id: alert.id,
                annotation_id: alert.annotation_id,
                symbol: alert.symbol,
                price: msg.price,
                condition: alert.condition,
              }
              // Send to the client that reported the price
              send(client, alertMsg)
              // Also broadcast to all subscribers of this symbol
              broadcast(msg.symbol, alertMsg)
            }
            break
          }
        }
      } catch (e) {
        send(client, { type: 'error', message: 'Invalid message' })
      }
    })

    socket.on('close', () => {
      removeClient(client)
    })

    socket.on('error', () => {
      removeClient(client)
    })
  })
}
