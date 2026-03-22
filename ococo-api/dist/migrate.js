import { pool } from './db.js';
async function migrate() {
    console.info('Running migrations...');
    await pool.query(`
    CREATE TABLE IF NOT EXISTS annotations (
      id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
      symbol          VARCHAR(20) NOT NULL,
      source          VARCHAR(30) NOT NULL DEFAULT 'user',
      type            VARCHAR(30) NOT NULL,
      points          JSONB NOT NULL DEFAULT '[]',
      style           JSONB NOT NULL DEFAULT '{}',
      strength        REAL NOT NULL DEFAULT 0.5,
      "group"         VARCHAR(50),
      tags            TEXT[] NOT NULL DEFAULT '{}',
      visibility      TEXT[] NOT NULL DEFAULT '{*}',
      timeframe       VARCHAR(10),
      ttl             TIMESTAMPTZ,
      metadata        JSONB NOT NULL DEFAULT '{}',
      created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
      updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
    )
  `);
    await pool.query(`CREATE INDEX IF NOT EXISTS idx_annotations_symbol ON annotations (symbol)`);
    await pool.query(`CREATE INDEX IF NOT EXISTS idx_annotations_symbol_source ON annotations (symbol, source)`);
    await pool.query(`CREATE INDEX IF NOT EXISTS idx_annotations_group ON annotations ("group")`);
    await pool.query(`CREATE INDEX IF NOT EXISTS idx_annotations_tags ON annotations USING GIN (tags)`);
    await pool.query(`CREATE INDEX IF NOT EXISTS idx_annotations_ttl ON annotations (ttl) WHERE ttl IS NOT NULL`);
    await pool.query(`
    CREATE TABLE IF NOT EXISTS alert_rules (
      id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
      annotation_id   UUID REFERENCES annotations(id) ON DELETE CASCADE,
      symbol          VARCHAR(20) NOT NULL,
      condition       VARCHAR(30) NOT NULL,
      price           REAL,
      active          BOOLEAN NOT NULL DEFAULT true,
      last_triggered  TIMESTAMPTZ,
      cooldown_sec    INTEGER NOT NULL DEFAULT 60,
      notify          JSONB NOT NULL DEFAULT '{"websocket": true}',
      created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
    )
  `);
    await pool.query(`CREATE INDEX IF NOT EXISTS idx_alert_rules_symbol ON alert_rules (symbol) WHERE active = true`);
    await pool.query(`CREATE INDEX IF NOT EXISTS idx_alert_rules_annotation ON alert_rules (annotation_id)`);
    // Migrate old drawings table data if it exists
    const oldTable = await pool.query(`SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_name = 'drawings')`);
    if (oldTable.rows[0].exists) {
        const count = await pool.query(`SELECT COUNT(*) FROM drawings`);
        if (parseInt(count.rows[0].count) > 0) {
            await pool.query(`
        INSERT INTO annotations (id, symbol, source, type, points, style, timeframe, created_at, updated_at)
        SELECT
          id, symbol, 'user', type,
          points,
          jsonb_build_object('color', color, 'opacity', opacity, 'lineStyle', line_style, 'thickness', thickness),
          timeframe, created_at, updated_at
        FROM drawings
        ON CONFLICT (id) DO NOTHING
      `);
            console.info(`Migrated drawings to annotations table`);
        }
    }
    console.info('Migrations complete');
    await pool.end();
}
migrate().catch(err => {
    console.error('Migration failed:', err);
    process.exit(1);
});
//# sourceMappingURL=migrate.js.map