import type { FastifyInstance } from 'fastify'
import * as ann from './annotations.js'
import * as alerts from './alerts.js'
import { healthCheck } from './db.js'
import { getClientCount, getSubscriptionCount } from './signalBus.js'

export async function registerRoutes(app: FastifyInstance): Promise<void> {

  // Health
  app.get('/api/health', async () => {
    const dbOk = await healthCheck()
    return { status: dbOk ? 'ok' : 'degraded', db: dbOk, clients: getClientCount(), subscriptions: getSubscriptionCount() }
  })

  // ---- Annotations ----

  app.get<{ Querystring: { symbol?: string; source?: string; group?: string; tags?: string; type?: string } }>(
    '/api/annotations', async (req) => {
      const tags = req.query.tags ? req.query.tags.split(',') : undefined
      return ann.listAnnotations({
        symbol: req.query.symbol,
        source: req.query.source,
        group: req.query.group,
        tags,
        type: req.query.type,
      })
    }
  )

  app.get<{ Params: { id: string } }>('/api/annotations/:id', async (req, reply) => {
    const result = await ann.getAnnotation(req.params.id)
    if (!result) return reply.code(404).send({ error: 'not found' })
    return result
  })

  app.post('/api/annotations', async (req, reply) => {
    const body = req.body as any
    if (!body.symbol || !body.type) return reply.code(400).send({ error: 'symbol and type required' })
    if (body.id) return ann.upsertAnnotation(body)
    return ann.createAnnotation(body)
  })

  app.patch<{ Params: { id: string } }>('/api/annotations/:id', async (req, reply) => {
    const result = await ann.updateAnnotation(req.params.id, req.body as any)
    if (!result) return reply.code(404).send({ error: 'not found' })
    return result
  })

  app.patch<{ Params: { id: string } }>('/api/annotations/:id/points', async (req) => {
    await ann.updatePoints(req.params.id, (req.body as any).points)
    return { ok: true }
  })

  app.patch<{ Params: { id: string } }>('/api/annotations/:id/style', async (req) => {
    await ann.updateStyle(req.params.id, req.body as any)
    return { ok: true }
  })

  app.delete<{ Params: { id: string } }>('/api/annotations/:id', async (req) => {
    await ann.deleteAnnotation(req.params.id)
    return { ok: true }
  })

  app.delete<{ Querystring: { symbol?: string; source?: string; group?: string } }>(
    '/api/annotations', async (req, reply) => {
      if (!req.query.symbol && !req.query.source && !req.query.group) {
        return reply.code(400).send({ error: 'at least one filter required for bulk delete' })
      }
      const count = await ann.deleteByFilter({
        symbol: req.query.symbol,
        source: req.query.source,
        group: req.query.group,
      })
      return { deleted: count }
    }
  )

  // ---- Alerts ----

  app.get<{ Querystring: { symbol?: string } }>('/api/alerts', async (req) => {
    return alerts.listAlerts(req.query.symbol)
  })

  app.post('/api/alerts', async (req, reply) => {
    const body = req.body as any
    if (!body.symbol || !body.condition) return reply.code(400).send({ error: 'symbol and condition required' })
    return alerts.createAlert(body)
  })

  app.patch<{ Params: { id: string } }>('/api/alerts/:id', async (req, reply) => {
    const result = await alerts.updateAlert(req.params.id, req.body as any)
    if (!result) return reply.code(404).send({ error: 'not found' })
    return result
  })

  app.delete<{ Params: { id: string } }>('/api/alerts/:id', async (req) => {
    await alerts.deleteAlert(req.params.id)
    return { ok: true }
  })
}
