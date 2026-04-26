/**
 * Runtime config — every credential MUST come from the environment.
 *
 * Hosts/ports default to homelab values for local dev convenience, but
 * passwords/tokens never have a hardcoded fallback. Missing creds at startup
 * should fail loud rather than silently use a known value committed to source.
 */
function requireEnv(key: string): string {
  const v = process.env[key]
  if (!v) {
    throw new Error(
      `Missing required env var: ${key}. Set it via .env, k8s secret, or shell.`,
    )
  }
  return v
}

export const config = {
  port: parseInt(process.env.PORT ?? '3000'),
  host: process.env.HOST ?? '0.0.0.0',

  postgres: {
    host: process.env.POSTGRES_HOST ?? '192.168.1.143',
    port: parseInt(process.env.POSTGRES_PORT ?? '5432'),
    database: process.env.POSTGRES_DB ?? 'ococo',
    user: process.env.POSTGRES_USER ?? 'postgres',
    password: requireEnv('POSTGRES_PASSWORD'),
    max: parseInt(process.env.POSTGRES_POOL_MAX ?? '20'),
  },

  redis: {
    host: process.env.REDIS_HOST ?? '192.168.1.89',
    port: parseInt(process.env.REDIS_PORT ?? '6379'),
    password: requireEnv('REDIS_PASSWORD'),
  },

  influx: {
    url: process.env.INFLUXDB_URL ?? 'http://192.168.1.67:8086',
    token: requireEnv('INFLUXDB_TOKEN'),
    org: process.env.INFLUXDB_ORG ?? 'homelab',
  },

  /** TTL reaper interval in ms */
  reaperInterval: 60_000,
  /** Default annotation cache TTL in seconds */
  cacheTtl: 30,
}
