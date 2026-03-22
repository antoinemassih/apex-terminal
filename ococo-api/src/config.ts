export const config = {
  port: parseInt(process.env.PORT ?? '3000'),
  host: process.env.HOST ?? '0.0.0.0',

  postgres: {
    host: process.env.POSTGRES_HOST ?? '192.168.1.143',
    port: parseInt(process.env.POSTGRES_PORT ?? '5432'),
    database: process.env.POSTGRES_DB ?? 'ococo',
    user: process.env.POSTGRES_USER ?? 'postgres',
    password: process.env.POSTGRES_PASSWORD ?? 'monkeyxx',
    max: parseInt(process.env.POSTGRES_POOL_MAX ?? '20'),
  },

  redis: {
    host: process.env.REDIS_HOST ?? '192.168.1.89',
    port: parseInt(process.env.REDIS_PORT ?? '6379'),
    password: process.env.REDIS_PASSWORD ?? 'monkeyxx',
  },

  influx: {
    url: process.env.INFLUXDB_URL ?? 'http://192.168.1.67:8086',
    token: process.env.INFLUXDB_TOKEN ?? 'Fi1WqaQKgpdnWt9l00w6dvbY1x1-wajcxU_G77PvxbL1cD9B82d1a_OLzdtcNlW-zdyAd02GXGqBt5tkDG9cqw==',
    org: process.env.INFLUXDB_ORG ?? 'homelab',
  },

  /** TTL reaper interval in ms */
  reaperInterval: 60_000,
  /** Default annotation cache TTL in seconds */
  cacheTtl: 30,
}
