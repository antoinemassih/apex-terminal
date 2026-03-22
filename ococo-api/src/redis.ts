import Redis from 'ioredis'
import { config } from './config.js'

/** Main Redis client for caching */
export const redis = new Redis({
  host: config.redis.host,
  port: config.redis.port,
  password: config.redis.password,
  maxRetriesPerRequest: 3,
  lazyConnect: true,
})

/** Dedicated subscriber client (Redis requires separate connection for pub/sub) */
export const redisSub = new Redis({
  host: config.redis.host,
  port: config.redis.port,
  password: config.redis.password,
  maxRetriesPerRequest: 3,
  lazyConnect: true,
})

/** Dedicated publisher client */
export const redisPub = new Redis({
  host: config.redis.host,
  port: config.redis.port,
  password: config.redis.password,
  maxRetriesPerRequest: 3,
  lazyConnect: true,
})

redis.on('error', (err) => console.error('Redis error:', err))
redisSub.on('error', (err) => console.error('Redis sub error:', err))
redisPub.on('error', (err) => console.error('Redis pub error:', err))

export async function connectRedis(): Promise<void> {
  await Promise.all([redis.connect(), redisSub.connect(), redisPub.connect()])
  console.info('Redis connected')
}
