export declare const config: {
    port: number;
    host: string;
    postgres: {
        host: string;
        port: number;
        database: string;
        user: string;
        password: string;
        max: number;
    };
    redis: {
        host: string;
        port: number;
        password: string;
    };
    influx: {
        url: string;
        token: string;
        org: string;
    };
    /** TTL reaper interval in ms */
    reaperInterval: number;
    /** Default annotation cache TTL in seconds */
    cacheTtl: number;
};
//# sourceMappingURL=config.d.ts.map