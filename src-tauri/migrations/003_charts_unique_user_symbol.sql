-- Adds the UNIQUE constraint on (user_id, symbol_canonical) that the
-- find_or_create_chart ON CONFLICT clause in drawing_db.rs relies on.
--
-- The original 001_chart_state.sql only created an index (non-unique).
-- This migration upgrades that index to a proper unique constraint so
-- that INSERT … ON CONFLICT (user_id, symbol_canonical) DO NOTHING works
-- and duplicate chart rows can no longer be created (TOCTOU fix).
--
-- Apply with:
--   psql "$APEX_PG_URL" -f migrations/003_charts_unique_user_symbol.sql
--
-- Safe to run on a database that already has 001 applied: the old non-unique
-- index is dropped first, then the unique constraint is added.  If duplicate
-- rows were somehow created before this migration, the ALTER will fail — in
-- that case deduplicate first:
--   DELETE FROM charts a USING charts b
--     WHERE a.user_id = b.user_id
--       AND a.symbol_canonical = b.symbol_canonical
--       AND a.created_at > b.created_at;

BEGIN;

-- Drop the non-unique index from 001_chart_state.sql, if it still exists.
DROP INDEX IF EXISTS charts_user_symbol;

-- Add the unique constraint (also creates an implicit unique index).
ALTER TABLE charts
    ADD CONSTRAINT charts_user_symbol_unique
    UNIQUE (user_id, symbol_canonical);

COMMIT;
