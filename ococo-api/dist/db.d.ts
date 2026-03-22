import pg from 'pg';
declare const pool: import("pg").Pool;
export { pool };
export declare function query<T extends pg.QueryResultRow = any>(text: string, params?: any[]): Promise<pg.QueryResult<T>>;
export declare function healthCheck(): Promise<boolean>;
//# sourceMappingURL=db.d.ts.map