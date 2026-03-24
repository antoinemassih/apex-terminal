import type { Annotation } from './types.js';
/** Get cached annotations for a symbol. Returns null on miss. */
export declare function getCached(symbol: string): Promise<Annotation[] | null>;
/** Cache annotations for a symbol */
export declare function setCache(symbol: string, annotations: Annotation[]): Promise<void>;
/** Invalidate cache for a symbol */
export declare function invalidate(symbol: string): Promise<void>;
export declare function cachedFetch(symbol: string, fetcher: () => Promise<Annotation[]>): Promise<Annotation[]>;
/** Invalidate all annotation caches */
export declare function invalidateAll(): Promise<void>;
//# sourceMappingURL=cache.d.ts.map