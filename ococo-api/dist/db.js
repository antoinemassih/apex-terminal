import pg from 'pg';
import { config } from './config.js';
const pool = new pg.Pool({
    host: config.postgres.host,
    port: config.postgres.port,
    database: config.postgres.database,
    user: config.postgres.user,
    password: config.postgres.password,
    max: config.postgres.max,
});
pool.on('error', (err) => {
    console.error('Unexpected PG pool error:', err);
});
export { pool };
export async function query(text, params) {
    return pool.query(text, params);
}
export async function healthCheck() {
    try {
        await pool.query('SELECT 1');
        return true;
    }
    catch {
        return false;
    }
}
//# sourceMappingURL=db.js.map