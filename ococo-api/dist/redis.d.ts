import Redis from 'ioredis';
/** Main Redis client for caching */
export declare const redis: Redis;
/** Dedicated subscriber client (Redis requires separate connection for pub/sub) */
export declare const redisSub: Redis;
/** Dedicated publisher client */
export declare const redisPub: Redis;
export declare function connectRedis(): Promise<void>;
//# sourceMappingURL=redis.d.ts.map