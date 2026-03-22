import Fastify from 'fastify'
import fastifyWebsocket from '@fastify/websocket'
import fastifyCors from '@fastify/cors'
import { config } from './config.js'
import { connectRedis } from './redis.js'
import { initSignalBus } from './signalBus.js'
import { registerRoutes } from './routes.js'
import { registerWebSocket } from './ws.js'
import { reapExpired } from './annotations.js'
import { healthCheck } from './db.js'

async function main() {
  const app = Fastify({ logger: true })

  // Plugins
  await app.register(fastifyCors, { origin: true })
  await app.register(fastifyWebsocket)

  // Connect to Redis
  await connectRedis()
  initSignalBus()

  // Verify PostgreSQL
  const dbOk = await healthCheck()
  if (!dbOk) {
    console.error('PostgreSQL is unreachable — aborting')
    process.exit(1)
  }
  console.info('PostgreSQL connected')

  // Register routes
  await registerRoutes(app)
  await registerWebSocket(app)

  // TTL reaper — cleans expired annotations periodically
  setInterval(async () => {
    try {
      const count = await reapExpired()
      if (count > 0) console.info(`Reaped ${count} expired annotations`)
    } catch (e) {
      console.error('Reaper error:', e)
    }
  }, config.reaperInterval)

  // Start
  await app.listen({ port: config.port, host: config.host })
  console.info(`OCOCO API listening on ${config.host}:${config.port}`)
}

main().catch(err => {
  console.error('Fatal:', err)
  process.exit(1)
})
